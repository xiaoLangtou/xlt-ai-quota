use std::fs;
use std::path::Path;

use serde::Serialize;

use super::error::SkillsError;
use super::paths::safe_join;

/// 单个技能打包产物大小上限（与安装引擎下载上限一致）。
const MAX_PACKAGE_BYTES: usize = 50 * 1024 * 1024;

fn api_base() -> String {
    std::env::var("LSM_SKILLSSH_BASE").unwrap_or_else(|_| "https://skills.sh".to_owned())
}

fn client() -> Result<reqwest::Client, SkillsError> {
    reqwest::Client::builder()
        .user_agent("xlt-workbench")
        .build()
        .map_err(|error| SkillsError::internal(error.to_string()))
}

pub struct SkillsShRef {
    pub owner: String,
    pub repo: String,
    pub slug: String,
}

/// 解析 owner/repo/skill 或 skills.sh 详情页 URL。
pub fn parse_skills_sh_ref(reference: &str) -> Result<SkillsShRef, SkillsError> {
    let text = regex_strip(reference.trim());
    let parts: Vec<&str> = text.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 3 {
        return Err(SkillsError::bad_request("skills.sh 引用格式错误，需要 owner/repo/skill"));
    }
    Ok(SkillsShRef {
        owner: parts[0].to_owned(),
        repo: parts[1].to_owned(),
        slug: parts[2..].join("/"),
    })
}

fn regex_strip(text: &str) -> String {
    let lower = text.to_lowercase();
    for prefix in ["https://skills.sh/", "http://skills.sh/", "https://www.skills.sh/", "http://www.skills.sh/"] {
        if lower.starts_with(prefix) {
            return text[prefix.len()..].to_owned();
        }
    }
    text.to_owned()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsShSearchItem {
    pub id: String,
    pub name: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installs: Option<u64>,
    pub detail_url: String,
    pub repo_url: String,
}

/// 标准化 skills.sh 搜索接口的原始条目（过滤脏数据，按安装量降序）。
fn normalize_search_results(raw: &serde_json::Value) -> Vec<SkillsShSearchItem> {
    let Some(list) = raw.get("skills").and_then(|value| value.as_array()) else {
        return Vec::new();
    };
    let mut items: Vec<SkillsShSearchItem> = list
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.to_owned();
            let name = entry.get("name")?.as_str()?.to_owned();
            let source = entry.get("source")?.as_str()?.to_owned();
            if !is_repo_slug(&source) {
                return None;
            }
            Some(SkillsShSearchItem {
                id: id.clone(),
                name,
                source: source.clone(),
                installs: entry.get("installs").and_then(|value| value.as_u64()),
                detail_url: format!("https://skills.sh/{id}"),
                repo_url: format!("https://github.com/{source}"),
            })
        })
        .collect();
    items.sort_by(|a, b| b.installs.unwrap_or(0).cmp(&a.installs.unwrap_or(0)));
    items
}

fn is_repo_slug(value: &str) -> bool {
    let parts: Vec<&str> = value.split('/').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')))
}

pub async fn search_skills_sh(
    query: &str,
    limit: Option<u32>,
) -> Result<Vec<SkillsShSearchItem>, SkillsError> {
    let keyword = query.trim();
    if keyword.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let capped = limit.unwrap_or(20).clamp(1, 50);
    let http = client()?;
    let response = http
        .get(format!("{}/api/search", api_base()))
        .query(&[("q", keyword), ("limit", &capped.to_string())])
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| SkillsError::new(502, format!("skills.sh 搜索失败：{error}")))?;
    if !response.status().is_success() {
        return Err(SkillsError::new(
            502,
            format!("skills.sh 搜索失败：{}", response.status().as_u16()),
        ));
    }
    let raw = response.json::<serde_json::Value>().await.unwrap_or(serde_json::Value::Null);
    Ok(normalize_search_results(&raw))
}

#[derive(Debug, Clone, Serialize)]
pub struct SkillsShFile {
    pub path: String,
    pub contents: String,
}

/// 拉取 skills.sh 打包产物（详情预览与安装共用）。
pub async fn fetch_skills_sh_package(
    reference: &str,
) -> Result<(String, Vec<SkillsShFile>), SkillsError> {
    let parsed = parse_skills_sh_ref(reference)?;
    let http = client()?;
    let url = format!(
        "{}/api/download/{}/{}/{}",
        api_base(),
        percent_encode(&parsed.owner),
        percent_encode(&parsed.repo),
        percent_encode(&parsed.slug)
    );
    let response = http
        .get(url)
        .header("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| SkillsError::new(502, format!("skills.sh 下载失败：{error}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(SkillsError::not_found(format!(
            "skills.sh 上不存在该技能：{}/{}",
            parsed.owner, parsed.repo
        )));
    }
    if !response.status().is_success() {
        return Err(SkillsError::new(
            502,
            format!("skills.sh 下载失败：{}", response.status().as_u16()),
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|error| SkillsError::new(502, format!("skills.sh 下载失败：{error}")))?;
    if text.len() > MAX_PACKAGE_BYTES {
        return Err(SkillsError::new(
            413,
            format!("技能包过大，上限 {}MB", MAX_PACKAGE_BYTES / 1024 / 1024),
        ));
    }
    let data: serde_json::Value = serde_json::from_str(&text)
        .map_err(|_| SkillsError::new(502, "skills.sh 返回内容不是合法 JSON"))?;
    let Some(files) = data.get("files").and_then(|value| value.as_array()) else {
        return Err(SkillsError::new(502, "skills.sh 返回的技能包为空"));
    };
    let files: Vec<SkillsShFile> = files
        .iter()
        .filter_map(|file| {
            let path = file.get("path")?.as_str()?.to_owned();
            let contents = file.get("contents")?.as_str()?.to_owned();
            (!path.is_empty()).then_some(SkillsShFile { path, contents })
        })
        .collect();
    if files.is_empty() {
        return Err(SkillsError::new(502, "skills.sh 返回的技能包没有可用文件"));
    }
    Ok((parsed.slug, files))
}

fn percent_encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes()).collect()
}

/// 将打包产物文件清单写入 workDir/<dirName>（safeJoin 防路径穿越）。
pub fn write_package_files(
    files: &[SkillsShFile],
    work_dir: &Path,
    dir_name: &str,
) -> Result<usize, SkillsError> {
    let base = safe_join(work_dir, dir_name)?;
    let mut written = 0;
    let mut total = 0usize;
    for file in files {
        if file.path.is_empty() {
            continue;
        }
        total += file.contents.len();
        if total > MAX_PACKAGE_BYTES {
            return Err(SkillsError::new(
                413,
                format!("技能包过大，上限 {}MB", MAX_PACKAGE_BYTES / 1024 / 1024),
            ));
        }
        let out = safe_join(&base, &file.path)?;
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out, &file.contents)?;
        written += 1;
    }
    Ok(written)
}

/// 从 skills.sh 下载技能打包产物并落到 workDir，返回子目录名。
pub async fn download_skills_sh(reference: &str, work_dir: &Path) -> Result<String, SkillsError> {
    let (slug, files) = fetch_skills_sh_package(reference).await?;
    let dir_name = sanitize_dir_name(&slug);
    let written = write_package_files(&files, work_dir, &dir_name)?;
    if written == 0 {
        return Err(SkillsError::new(502, "skills.sh 返回的技能包没有可写入的文件"));
    }
    Ok(dir_name)
}

fn sanitize_dir_name(slug: &str) -> String {
    let cleaned: String = slug
        .chars()
        .map(|c| if c.is_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    let trimmed = cleaned.trim_start_matches(['.', '-']);
    if trimmed.is_empty() {
        "skill".to_owned()
    } else {
        trimmed.to_owned()
    }
}

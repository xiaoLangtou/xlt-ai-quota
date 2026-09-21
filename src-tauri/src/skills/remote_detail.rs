use serde::Serialize;

use super::error::SkillsError;
use super::install::github::resolve_github_location;
use super::skillssh::fetch_skills_sh_package;
use super::validation::parse_skill_md;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileEntry {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteSkillDetail {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub body: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<RemoteFileEntry>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFileContent {
    pub path: String,
    pub content: String,
}

fn client() -> Result<reqwest::Client, SkillsError> {
    reqwest::Client::builder()
        .user_agent("xlt-workbench")
        .build()
        .map_err(|error| SkillsError::internal(error.to_string()))
}

async fn fetch_text(url: &str, timeout_ms: u64) -> Option<String> {
    let http = client().ok()?;
    let response = http
        .get(url)
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

fn to_detail(raw: &str, fallback_name: &str) -> RemoteSkillDetail {
    let parsed = parse_skill_md(raw);
    let (frontmatter, body) = match parsed {
        Ok(parsed) => (
            parsed.frontmatter.as_object().cloned().unwrap_or_default(),
            parsed.body,
        ),
        Err(_) => (Default::default(), raw.to_owned()),
    };
    let get = |key: &str| {
        frontmatter
            .get(key)
            .and_then(|value| value.as_str())
            .map(ToOwned::to_owned)
    };
    RemoteSkillDetail {
        name: get("name").filter(|value| !value.is_empty()).unwrap_or_else(|| fallback_name.to_owned()),
        description: get("description"),
        version: get("version"),
        license: get("license"),
        body: body.trim().to_owned(),
        files: None,
    }
}

async fn list_github_dir(
    owner: &str,
    repo: &str,
    branch: &str,
    subpath: &str,
) -> Option<Vec<RemoteFileEntry>> {
    let encoded_path = if subpath.is_empty() {
        String::new()
    } else {
        format!(
            "/{}",
            subpath.split('/').map(percent_encode).collect::<Vec<_>>().join("/")
        )
    };
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/contents{encoded_path}?ref={}",
        percent_encode(branch)
    );
    let http = client().ok()?;
    let response = http
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let data: serde_json::Value = response.json().await.ok()?;
    let list = data.as_array()?;
    let mut entries: Vec<RemoteFileEntry> = list
        .iter()
        .filter_map(|entry| {
            let name = entry.get("name")?.as_str()?;
            let kind = entry.get("type").and_then(|value| value.as_str()).unwrap_or("file");
            if kind == "dir" {
                Some(RemoteFileEntry {
                    path: format!("{name}/"),
                    size_bytes: None,
                })
            } else {
                Some(RemoteFileEntry {
                    path: name.to_owned(),
                    size_bytes: entry.get("size").and_then(|value| value.as_u64()),
                })
            }
        })
        .collect();
    entries.sort_by(|a, b| {
        let a_dir = a.path.ends_with('/');
        let b_dir = b.path.ends_with('/');
        b_dir.cmp(&a_dir).then_with(|| a.path.cmp(&b.path))
    });
    Some(entries)
}

fn percent_encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes()).collect()
}

async fn github_detail(reference: &str) -> Result<RemoteSkillDetail, SkillsError> {
    let location = resolve_github_location(reference).await?;
    let branches: Vec<String> = location
        .revision
        .clone()
        .map(|rev| vec![rev])
        .unwrap_or_else(|| vec!["main".to_owned(), "master".to_owned()]);
    let md_path = match &location.subpath {
        Some(subpath) => format!("{subpath}/SKILL.md"),
        None => "SKILL.md".to_owned(),
    };
    for branch in branches {
        let encoded = md_path.split('/').map(percent_encode).collect::<Vec<_>>().join("/");
        let url = format!(
            "https://raw.githubusercontent.com/{}/{}/{branch}/{encoded}",
            location.owner, location.repo
        );
        if let Some(raw) = fetch_text(&url, 15_000).await {
            let fallback = location
                .subpath
                .as_deref()
                .and_then(|subpath| subpath.split('/').next_back())
                .unwrap_or(&location.repo);
            let mut detail = to_detail(&raw, fallback);
            detail.files = list_github_dir(
                &location.owner,
                &location.repo,
                &branch,
                location.subpath.as_deref().unwrap_or(""),
            )
            .await;
            return Ok(detail);
        }
    }
    Err(SkillsError::not_found(format!("未找到 SKILL.md：{reference}")))
}

async fn skills_sh_detail(reference: &str) -> Result<RemoteSkillDetail, SkillsError> {
    let (slug, files) = fetch_skills_sh_package(reference).await?;
    let skill_md = files
        .iter()
        .find(|file| file.path.to_lowercase() == "skill.md")
        .ok_or_else(|| SkillsError::not_found(format!("技能包中没有 SKILL.md：{reference}")))?;
    let mut detail = to_detail(&skill_md.contents, &slug);
    let mut entries: Vec<RemoteFileEntry> = files
        .iter()
        .map(|file| RemoteFileEntry {
            path: file.path.clone(),
            size_bytes: Some(file.contents.len() as u64),
        })
        .collect();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    detail.files = Some(entries);
    Ok(detail)
}

fn assert_safe_rel_path(path: &str) -> Result<String, SkillsError> {
    let trimmed = path.trim();
    if trimmed.is_empty()
        || trimmed.starts_with('/')
        || trimmed.contains('\\')
        || trimmed.split('/').any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(SkillsError::bad_request(format!("非法文件路径：{path}")));
    }
    Ok(trimmed.to_owned())
}

/// 读取远程技能包内单个文件的文本内容（详情页预览用）。
pub async fn get_remote_skill_file(
    source: &str,
    reference: &str,
    path: &str,
) -> Result<RemoteFileContent, SkillsError> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(SkillsError::bad_request("缺少 ref"));
    }
    let safe_path = assert_safe_rel_path(path)?;
    if source == "github" {
        let location = resolve_github_location(reference).await?;
        let full = match &location.subpath {
            Some(subpath) => format!("{subpath}/{safe_path}"),
            None => safe_path.clone(),
        };
        let branches: Vec<String> = location
            .revision
            .clone()
            .map(|rev| vec![rev])
            .unwrap_or_else(|| vec!["main".to_owned(), "master".to_owned()]);
        for branch in branches {
            let encoded = full.split('/').map(percent_encode).collect::<Vec<_>>().join("/");
            let url = format!(
                "https://raw.githubusercontent.com/{}/{}/{branch}/{encoded}",
                location.owner, location.repo
            );
            if let Some(content) = fetch_text(&url, 15_000).await {
                return Ok(RemoteFileContent {
                    path: safe_path,
                    content,
                });
            }
        }
        return Err(SkillsError::not_found(format!("文件不存在：{safe_path}")));
    }
    if source == "skills-sh" {
        let (_slug, files) = fetch_skills_sh_package(reference).await?;
        let hit = files
            .iter()
            .find(|file| file.path == safe_path)
            .ok_or_else(|| SkillsError::not_found(format!("文件不存在：{safe_path}")))?;
        return Ok(RemoteFileContent {
            path: safe_path,
            content: hit.contents.clone(),
        });
    }
    Err(SkillsError::bad_request(format!("该来源类型不支持文件预览：{source}")))
}

/// 按来源类型获取远程技能详情。
pub async fn get_remote_skill_detail(
    source: &str,
    reference: &str,
) -> Result<RemoteSkillDetail, SkillsError> {
    let reference = reference.trim();
    if reference.is_empty() {
        return Err(SkillsError::bad_request("缺少 ref"));
    }
    match source {
        "github" => github_detail(reference).await,
        "skills-sh" => skills_sh_detail(reference).await,
        _ => Err(SkillsError::bad_request(format!(
            "该来源类型不支持详情查看：{source}"
        ))),
    }
}

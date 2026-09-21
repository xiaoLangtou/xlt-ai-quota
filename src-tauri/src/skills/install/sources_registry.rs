use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use super::catalog::CatalogSkill;
use super::super::config::SkillsPaths;
use super::super::error::SkillsError;

/// 内置源（不可删除）。
const BUILTIN_SOURCES: &[(&str, &str)] = &[("anbeime/skill", "Skills 商店（anbeime）")];
/// 单个源最多索引的技能数（防御超大仓库）。
const MAX_SKILLS_PER_SOURCE: usize = 200;
/// frontmatter 拉取并发数。
const FETCH_CONCURRENCY: usize = 8;
/// 索引过期阈值：超过后访问目录时后台刷新。
const INDEX_TTL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddSourceRequest {
    pub repo: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillSource {
    pub id: String,
    pub repo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub label: String,
    pub builtin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub skill_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sync_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredSources {
    #[serde(default)]
    extra: Vec<AddSourceRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceIndexEntry {
    synced_at: String,
    #[serde(default)]
    skills: Vec<CatalogSkill>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn sources_file(paths: &SkillsPaths) -> std::path::PathBuf {
    paths.config_dir.join("sources.json")
}

fn index_file(paths: &SkillsPaths) -> std::path::PathBuf {
    paths.config_dir.join("sources-index.json")
}

fn read_json<T: for<'de> Deserialize<'de>>(file: &std::path::Path, fallback: T) -> T {
    fs::read_to_string(file)
        .ok()
        .and_then(|raw| serde_json::from_str::<T>(&raw).ok())
        .unwrap_or(fallback)
}

fn write_json<T: Serialize>(paths: &SkillsPaths, file: &std::path::Path, data: &T) -> Result<(), SkillsError> {
    fs::create_dir_all(&paths.config_dir)?;
    let text = serde_json::to_string_pretty(data)
        .map_err(|error| SkillsError::internal(error.to_string()))?;
    fs::write(file, text)?;
    Ok(())
}

/// 基于 repo 生成稳定 source id。
pub fn source_id(repo: &str) -> String {
    let digest = Sha1::digest(format!("source:{}", repo.to_lowercase()).as_bytes());
    format!("{digest:x}").chars().take(10).collect()
}

fn read_extras(paths: &SkillsPaths) -> Vec<AddSourceRequest> {
    read_json::<StoredSources>(&sources_file(paths), StoredSources::default()).extra
}

fn read_index(paths: &SkillsPaths) -> HashMap<String, SourceIndexEntry> {
    read_json::<HashMap<String, SourceIndexEntry>>(&index_file(paths), HashMap::new())
}

fn write_index_entry(paths: &SkillsPaths, id: &str, entry: &SourceIndexEntry) -> Result<(), SkillsError> {
    let mut index = read_index(paths);
    index.insert(id.to_owned(), entry.clone());
    write_json(paths, &index_file(paths), &index)
}

fn all_defs(paths: &SkillsPaths) -> Vec<(AddSourceRequest, bool)> {
    let mut defs: Vec<(AddSourceRequest, bool)> = BUILTIN_SOURCES
        .iter()
        .map(|(repo, label)| {
            (
                AddSourceRequest {
                    repo: (*repo).to_owned(),
                    branch: None,
                    label: Some((*label).to_owned()),
                },
                true,
            )
        })
        .collect();
    defs.extend(read_extras(paths).into_iter().map(|def| (def, false)));
    defs
}

fn to_skill_source(def: &AddSourceRequest, builtin: bool, index: &HashMap<String, SourceIndexEntry>) -> SkillSource {
    let id = source_id(&def.repo);
    let entry = index.get(&id);
    SkillSource {
        id,
        repo: def.repo.clone(),
        branch: def.branch.clone(),
        label: def.label.clone().unwrap_or_else(|| def.repo.clone()),
        builtin,
        kind: None,
        description: None,
        skill_count: entry.map(|entry| entry.skills.len()).unwrap_or(0),
        last_sync_at: entry.map(|entry| entry.synced_at.clone()),
        last_sync_error: entry.and_then(|entry| entry.error.clone()),
    }
}

fn search_source() -> SkillSource {
    SkillSource {
        id: "skills-sh".to_owned(),
        repo: "skills.sh".to_owned(),
        branch: None,
        label: "skills.sh 全网搜索".to_owned(),
        builtin: true,
        kind: Some("search".to_owned()),
        description: Some("实时检索 skills.sh 收录的开源技能，无需同步索引".to_owned()),
        skill_count: 0,
        last_sync_at: None,
        last_sync_error: None,
    }
}

/// 列出所有已接入源（内置仓库源 + 搜索引擎源 + 用户登记）。
pub fn list_sources(paths: &SkillsPaths) -> Vec<SkillSource> {
    let index = read_index(paths);
    let mut sources: Vec<SkillSource> = all_defs(paths)
        .iter()
        .map(|(def, builtin)| to_skill_source(def, *builtin, &index))
        .collect();
    sources.push(search_source());
    sources
}

/// 新增一个源（校验 repo 格式与重复）。
pub fn add_source(paths: &SkillsPaths, request: &AddSourceRequest) -> Result<SkillSource, SkillsError> {
    let repo = normalize_repo(&request.repo);
    if !is_repo(&repo) {
        return Err(SkillsError::bad_request("仓库格式错误，应为 owner/repo 或 GitHub 仓库 URL"));
    }
    if all_defs(paths)
        .iter()
        .any(|(def, _)| def.repo.to_lowercase() == repo.to_lowercase())
    {
        return Err(SkillsError::conflict(format!("源已存在：{repo}")));
    }
    let mut stored = StoredSources {
        extra: read_extras(paths),
    };
    let def = AddSourceRequest {
        repo,
        branch: request.branch.clone().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty()),
        label: request.label.clone().map(|value| value.trim().to_owned()).filter(|value| !value.is_empty()),
    };
    stored.extra.push(def.clone());
    write_json(paths, &sources_file(paths), &stored)?;
    Ok(to_skill_source(&def, false, &read_index(paths)))
}

/// 移除一个用户登记的源（内置源不可删除；同时清掉其索引）。
pub fn remove_source(paths: &SkillsPaths, id: &str) -> Result<bool, SkillsError> {
    if BUILTIN_SOURCES.iter().any(|(repo, _)| source_id(repo) == id) {
        return Err(SkillsError::bad_request("内置源不可删除"));
    }
    let extras = read_extras(paths);
    let next: Vec<AddSourceRequest> = extras
        .iter()
        .filter(|def| source_id(&def.repo) != id)
        .cloned()
        .collect();
    if next.len() == extras.len() {
        return Ok(false);
    }
    write_json(paths, &sources_file(paths), &StoredSources { extra: next })?;
    let mut index = read_index(paths);
    if index.remove(id).is_some() {
        write_json(paths, &index_file(paths), &index)?;
    }
    Ok(true)
}

fn normalize_repo(repo: &str) -> String {
    let mut text = repo.trim().to_owned();
    for prefix in ["https://github.com/", "http://github.com/"] {
        if text.to_lowercase().starts_with(prefix) {
            text = text[prefix.len()..].to_owned();
        }
    }
    text = text.strip_suffix(".git").unwrap_or(&text).to_owned();
    text.trim_end_matches('/').to_owned()
}

fn is_repo(repo: &str) -> bool {
    let parts: Vec<&str> = repo.split('/').collect();
    parts.len() == 2
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-')))
}

/// 从文件清单挑出 skill 目录（含 SKILL.md 的目录，目录嵌套时只保留最浅一层）。
pub fn pick_skill_dirs(paths: &[String]) -> Vec<String> {
    let dirs: Vec<String> = paths
        .iter()
        .filter(|path| *path == "SKILL.md" || path.ends_with("/SKILL.md"))
        .map(|path| {
            if path == "SKILL.md" {
                String::new()
            } else {
                path.trim_end_matches("/SKILL.md").to_owned()
            }
        })
        .collect();
    let mut sub_dirs: Vec<String> = dirs.iter().filter(|dir| !dir.is_empty()).cloned().collect();
    sub_dirs.sort_by_key(|dir| dir.len());
    if sub_dirs.is_empty() {
        return if dirs.iter().any(|dir| dir.is_empty()) {
            vec![String::new()]
        } else {
            Vec::new()
        };
    }
    let mut kept: Vec<String> = Vec::new();
    for dir in sub_dirs {
        if !kept.iter().any(|existing| dir.starts_with(&format!("{existing}/"))) {
            kept.push(dir);
        }
    }
    kept.truncate(MAX_SKILLS_PER_SOURCE);
    kept
}

fn has_scripts(files: &[String], dir: &str) -> bool {
    let prefix = if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    };
    files.iter().any(|path| {
        path.starts_with(&prefix)
            && path != &format!("{prefix}SKILL.md")
            && path
                .rsplit('.')
                .next()
                .map(|extension| {
                    matches!(
                        extension.to_lowercase().as_str(),
                        "py" | "sh" | "bash" | "zsh" | "js" | "mjs" | "cjs" | "ts" | "rb" | "ps1" | "bat" | "cmd"
                    )
                })
                .unwrap_or(false)
    })
}

async fn fetch_frontmatter(client: &reqwest::Client, repo: &str, branch: &str, dir: &str) -> (Option<String>, Option<String>) {
    let path = if dir.is_empty() {
        "SKILL.md".to_owned()
    } else {
        format!("{dir}/SKILL.md")
    };
    let encoded = path
        .split('/')
        .map(|segment| url::form_urlencoded::byte_serialize(segment.as_bytes()).collect::<String>())
        .collect::<Vec<_>>()
        .join("/");
    let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/{encoded}");
    let Ok(response) = client
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    else {
        return (None, None);
    };
    if !response.status().is_success() {
        return (None, None);
    }
    let Ok(text) = response.text().await else {
        return (None, None);
    };
    match super::super::validation::parse_skill_md(&text) {
        Ok(parsed) => {
            let map = parsed.frontmatter.as_object();
            let get = |key: &str| {
                map.and_then(|map| map.get(key))
                    .and_then(|value| value.as_str())
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
            };
            (get("name"), get("description"))
        }
        Err(_) => (None, None),
    }
}

async fn do_sync(paths: &SkillsPaths, def: &AddSourceRequest) -> SourceIndexEntry {
    let id = source_id(&def.repo);
    let client = match reqwest::Client::builder().user_agent("xlt-workbench").build() {
        Ok(client) => client,
        Err(error) => {
            let mut entry = read_index(paths).remove(&id).unwrap_or(SourceIndexEntry {
                synced_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                skills: Vec::new(),
                error: None,
            });
            entry.error = Some(error.to_string());
            let _ = write_index_entry(paths, &id, &entry);
            return entry;
        }
    };

    let result: Result<SourceIndexEntry, SkillsError> = async {
        let meta: serde_json::Value = client
            .get(format!("https://api.github.com/repos/{}", def.repo))
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|error| SkillsError::new(502, format!("GitHub API 请求失败：{error}")))?
            .json()
            .await
            .map_err(|error| SkillsError::new(502, format!("GitHub API 响应非法：{error}")))?;
        let branch = def
            .branch
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| meta.get("default_branch").and_then(|value| value.as_str()).map(ToOwned::to_owned))
            .unwrap_or_else(|| "main".to_owned());
        let tree: serde_json::Value = client
            .get(format!(
                "https://api.github.com/repos/{}/git/trees/{branch}?recursive=1",
                def.repo
            ))
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(20))
            .send()
            .await
            .map_err(|error| SkillsError::new(502, format!("GitHub API 请求失败：{error}")))?
            .json()
            .await
            .map_err(|error| SkillsError::new(502, format!("GitHub API 响应非法：{error}")))?;
        let tree_sha = tree
            .get("sha")
            .and_then(|value| value.as_str())
            .unwrap_or("")
            .chars()
            .take(7)
            .collect::<String>();
        let files: Vec<String> = tree
            .get("tree")
            .and_then(|value| value.as_array())
            .map(|entries| {
                entries
                    .iter()
                    .filter(|entry| entry.get("type").and_then(|value| value.as_str()) == Some("blob"))
                    .filter_map(|entry| entry.get("path").and_then(|value| value.as_str()).map(ToOwned::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        let dirs = pick_skill_dirs(&files);
        let label = def.label.clone().unwrap_or_else(|| def.repo.clone());
        let stars = meta.get("stargazers_count").and_then(|value| value.as_u64());
        let pushed_at = meta.get("pushed_at").and_then(|value| value.as_str()).map(ToOwned::to_owned);

        let metas: Vec<(Option<String>, Option<String>)> = futures_util::stream::iter(dirs.iter().cloned())
            .map(|dir| {
                let client = client.clone();
                let repo = def.repo.clone();
                let branch = branch.clone();
                async move { fetch_frontmatter(&client, &repo, &branch, &dir).await }
            })
            .buffer_unordered(FETCH_CONCURRENCY)
            .collect()
            .await;

        let skills: Vec<CatalogSkill> = dirs
            .iter()
            .zip(metas.iter())
            .map(|(dir, (name, description))| {
                let fallback = if dir.is_empty() {
                    def.repo.split('/').next_back().unwrap_or("skill").to_owned()
                } else {
                    dir.split('/').next_back().unwrap_or("skill").to_owned()
                };
                let install_ref = if dir.is_empty() {
                    format!("{}/tree/{branch}", def.repo)
                } else {
                    format!("{}/tree/{branch}/{dir}", def.repo)
                };
                CatalogSkill {
                    id: format!(
                        "{id}-{}",
                        {
                            let digest = Sha1::digest(dir.as_bytes());
                            format!("{digest:x}").chars().take(8).collect::<String>()
                        }
                    ),
                    name: name.clone().unwrap_or(fallback),
                    description: description.clone().unwrap_or_else(|| "（无描述）".to_owned()),
                    repo: def.repo.clone(),
                    install_ref: install_ref.clone(),
                    homepage: format!("https://github.com/{install_ref}"),
                    tags: vec![label.clone()],
                    stars,
                    source_type: Some("raw-scan".to_owned()),
                    source_id: Some(id.clone()),
                    path_in_repo: Some(dir.clone()),
                    has_scripts: Some(has_scripts(&files, dir)),
                    version: (!tree_sha.is_empty()).then_some(tree_sha.clone()),
                    updated_at: pushed_at.clone(),
                }
            })
            .collect();
        Ok(SourceIndexEntry {
            synced_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            skills,
            error: None,
        })
    }
    .await;

    match result {
        Ok(entry) => {
            let _ = write_index_entry(paths, &id, &entry);
            entry
        }
        Err(error) => {
            // 失败降级：保留旧索引内容，仅记录失败原因。
            let mut entry = read_index(paths).remove(&id).unwrap_or(SourceIndexEntry {
                synced_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                skills: Vec::new(),
                error: None,
            });
            entry.error = Some(error.message);
            let _ = write_index_entry(paths, &id, &entry);
            entry
        }
    }
}

/// 手动强制同步某个源，返回更新后的源状态。
pub async fn sync_source(paths: &SkillsPaths, id: &str) -> Result<SkillSource, SkillsError> {
    let def = all_defs(paths)
        .into_iter()
        .find(|(def, _)| source_id(&def.repo) == id)
        .ok_or_else(|| SkillsError::not_found("数据源不存在"))?;
    let entry = do_sync(paths, &def.0).await;
    if entry.error.is_some() && entry.skills.is_empty() {
        return Err(SkillsError::new(
            502,
            format!("同步失败：{}", entry.error.unwrap_or_default()),
        ));
    }
    Ok(to_skill_source(&def.0, def.1, &read_index(paths)))
}

/// 返回所有已接入源的技能索引。从未同步过的源阻塞完成首次同步；已过期的源后台刷新。
pub async fn get_source_skills(paths: &SkillsPaths) -> Result<Vec<CatalogSkill>, SkillsError> {
    let defs = all_defs(paths);
    let index = read_index(paths);
    let missing: Vec<(AddSourceRequest, bool)> = defs
        .iter()
        .filter(|(def, _)| !index.contains_key(&source_id(&def.repo)))
        .cloned()
        .collect();
    for (def, _) in &missing {
        do_sync(paths, def).await;
    }
    let fresh = read_index(paths);
    for (def, _) in &defs {
        if let Some(entry) = fresh.get(&source_id(&def.repo)) {
            if let Ok(synced) = chrono::DateTime::parse_from_rfc3339(&entry.synced_at) {
                let age = chrono::Utc::now() - synced.with_timezone(&chrono::Utc);
                if age.num_seconds() > INDEX_TTL.as_secs() as i64 {
                    let def_clone = def.clone();
                    let paths_clone = paths.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = do_sync(&paths_clone, &def_clone).await;
                    });
                }
            }
        }
    }
    Ok(defs
        .iter()
        .flat_map(|(def, _)| fresh.get(&source_id(&def.repo)).map(|entry| entry.skills.clone()).unwrap_or_default())
        .collect())
}

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::download::download;
use super::extract::extract_archive_buffer;
use super::security::MAX_PACKAGE_BYTES;
use super::super::error::SkillsError;
use super::super::paths::safe_join;

pub struct GitHubLocation {
    pub owner: String,
    pub repo: String,
    /// 分支 / tag / commit SHA；缺省用默认分支。
    pub revision: Option<String>,
    pub subpath: Option<String>,
}

pub struct GitHubFetchResult {
    /// destDir 内应作为候选搜索根的相对子路径。
    pub search_subpath: Option<String>,
    /// 实际拉取的 rev（分支/tag/SHA）。
    pub revision: Option<String>,
    /// 解析出的不可变 commit SHA（best-effort）。
    pub commit_sha: Option<String>,
}

fn client() -> Result<reqwest::Client, SkillsError> {
    reqwest::Client::builder()
        .user_agent("xlt-workbench")
        .build()
        .map_err(|error| SkillsError::internal(error.to_string()))
}

async fn gh_json<T: for<'de> Deserialize<'de>>(path: &str, timeout_ms: u64) -> Option<T> {
    let http = client().ok()?;
    let response = http
        .get(format!("https://api.github.com{path}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(std::time::Duration::from_millis(timeout_ms))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json::<T>().await.ok()
}

/// 静态解析（不访问网络）：拆出 owner/repo 与 tree 剩余段。
fn parse_static(reference: &str) -> Result<(String, String, Option<Vec<String>>, Option<String>), SkillsError> {
    let mut text = reference.trim().to_owned();
    text = regex_strip_prefix(&text);
    text = text.strip_suffix(".git").unwrap_or(&text).to_owned();
    let parts: Vec<&str> = text.split('/').filter(|part| !part.is_empty()).collect();
    if parts.len() < 2 {
        return Err(SkillsError::bad_request("GitHub 引用格式错误，至少需要 owner/repo"));
    }
    let owner = parts[0].to_owned();
    let repo = parts[1].to_owned();
    let rest = &parts[2..];
    if rest.first() == Some(&"tree") && rest.len() >= 2 {
        return Ok((owner, repo, Some(rest[1..].iter().map(|part| (*part).to_owned()).collect()), None));
    }
    let subpath = rest.join("/");
    Ok((owner, repo, None, (!subpath.is_empty()).then_some(subpath)))
}

fn regex_strip_prefix(text: &str) -> String {
    let lower = text.to_lowercase();
    for prefix in ["https://github.com/", "http://github.com/"] {
        if lower.starts_with(prefix) {
            return text[prefix.len()..].to_owned();
        }
    }
    text.to_owned()
}

async fn commit_sha_of(owner: &str, repo: &str, rev: &str) -> Option<String> {
    let data = gh_json::<serde_json::Value>(
        &format!("/repos/{owner}/{repo}/commits/{rev}"),
        10_000,
    )
    .await?;
    data.get("sha").and_then(|value| value.as_str()).map(ToOwned::to_owned)
}

pub async fn github_default_branch(owner: &str, repo: &str) -> Option<String> {
    let data = gh_json::<serde_json::Value>(&format!("/repos/{owner}/{repo}"), 5_000).await?;
    data.get("default_branch")
        .and_then(|value| value.as_str())
        .map(ToOwned::to_owned)
}

/// 解析 GitHub 引用为标准 Location（/tree/ 剩余段按「最长 rev 优先」逐段验证）。
pub async fn resolve_github_location(reference: &str) -> Result<GitHubLocation, SkillsError> {
    let (owner, repo, tree_parts, subpath) = parse_static(reference)?;
    let Some(tree_parts) = tree_parts else {
        return Ok(GitHubLocation {
            owner,
            repo,
            revision: None,
            subpath,
        });
    };
    for index in (1..=tree_parts.len()).rev() {
        let rev = tree_parts[..index].join("/");
        if commit_sha_of(&owner, &repo, &rev).await.is_some() {
            let rest = tree_parts[index..].join("/");
            return Ok(GitHubLocation {
                owner,
                repo,
                revision: Some(rev),
                subpath: (!rest.is_empty()).then_some(rest),
            });
        }
    }
    Ok(GitHubLocation {
        owner,
        repo,
        revision: tree_parts.first().cloned(),
        subpath: if tree_parts.len() > 1 {
            Some(tree_parts[1..].join("/"))
        } else {
            None
        },
    })
}

/// 解析 Location 的不可变 commit SHA（best-effort）。
pub async fn resolve_github_revision(location: &GitHubLocation) -> Option<String> {
    let rev = match &location.revision {
        Some(rev) => Some(rev.clone()),
        None => github_default_branch(&location.owner, &location.repo).await,
    }?;
    commit_sha_of(&location.owner, &location.repo, &rev).await
}

async fn try_subpath_fetch(
    owner: &str,
    repo: &str,
    rev: &str,
    subpath: &str,
    dest_dir: &Path,
) -> bool {
    const MAX_FILES: usize = 200;
    let Some(data) = gh_json::<serde_json::Value>(
        &format!("/repos/{owner}/{repo}/git/trees/{rev}?recursive=1"),
        20_000,
    )
    .await
    else {
        return false;
    };
    let Some(tree) = data.get("tree").and_then(|value| value.as_array()) else {
        return false;
    };
    let truncated = data.get("truncated").and_then(|value| value.as_bool()).unwrap_or(false);
    let prefix = format!("{subpath}/");
    let files: Vec<&serde_json::Value> = tree
        .iter()
        .filter(|entry| {
            entry.get("type").and_then(|value| value.as_str()) == Some("blob")
                && entry
                    .get("path")
                    .and_then(|value| value.as_str())
                    .map(|path| path == subpath || path.starts_with(&prefix))
                    .unwrap_or(false)
        })
        .collect();
    if files.is_empty() || truncated || files.len() > MAX_FILES {
        return false;
    }
    let total: u64 = files
        .iter()
        .map(|entry| entry.get("size").and_then(|value| value.as_u64()).unwrap_or(0))
        .sum();
    if total > MAX_PACKAGE_BYTES {
        return false;
    }
    for entry in files {
        let Some(path) = entry.get("path").and_then(|value| value.as_str()) else {
            return false;
        };
        let rel = path.strip_prefix(&format!("{subpath}/")).unwrap_or(path);
        let target = match safe_join(&dest_dir.join(subpath), rel) {
            Ok(path) => path,
            Err(_) => return false,
        };
        let url = format!(
            "https://raw.githubusercontent.com/{owner}/{repo}/{rev}/{}",
            path.split('/').map(percent_encode).collect::<Vec<_>>().join("/")
        );
        let Ok(result) = download(&url).await else {
            return false;
        };
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&target, &result.buffer).is_err() {
            return false;
        }
    }
    true
}

fn percent_encode(segment: &str) -> String {
    url::form_urlencoded::byte_serialize(segment.as_bytes()).collect()
}

/// 将 GitHub 内容拉取到 destDir。
pub async fn fetch_github_to_dir(
    location: &GitHubLocation,
    dest_dir: &Path,
) -> Result<GitHubFetchResult, SkillsError> {
    let explicit = location.revision.clone();
    let default_branch = if explicit.is_none() {
        github_default_branch(&location.owner, &location.repo).await
    } else {
        None
    };
    let candidates: Vec<String> = match explicit {
        Some(rev) => vec![rev],
        None => default_branch
            .map(|branch| vec![branch])
            .unwrap_or_else(|| vec!["main".to_owned(), "master".to_owned()]),
    };

    if let Some(subpath) = &location.subpath {
        for rev in &candidates {
            if try_subpath_fetch(&location.owner, &location.repo, rev, subpath, dest_dir).await {
                return Ok(GitHubFetchResult {
                    search_subpath: Some(subpath.clone()),
                    revision: Some(rev.clone()),
                    commit_sha: commit_sha_of(&location.owner, &location.repo, rev).await,
                });
            }
        }
    }

    let mut errors = Vec::new();
    for rev in &candidates {
        let url = format!(
            "https://codeload.github.com/{}/{}/tar.gz/{}",
            location.owner,
            location.repo,
            percent_encode(rev)
        );
        match download(&url).await {
            Ok(result) => {
                extract_archive_buffer(&result.buffer, "tar.gz", dest_dir)?;
                let top = fs::read_dir(dest_dir)
                    .ok()
                    .and_then(|entries| {
                        entries
                            .flatten()
                            .find(|entry| entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false))
                            .map(|entry| entry.file_name().to_string_lossy().to_string())
                    })
                    .unwrap_or_default();
                let search_subpath = [top.clone(), location.subpath.clone().unwrap_or_default()]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join("/");
                return Ok(GitHubFetchResult {
                    search_subpath: (!search_subpath.is_empty()).then_some(search_subpath),
                    revision: Some(rev.clone()),
                    commit_sha: commit_sha_of(&location.owner, &location.repo, rev).await,
                });
            }
            Err(error) => errors.push(format!("{rev}: {}", error.message)),
        }
    }
    Err(SkillsError::new(
        502,
        format!("GitHub 拉取失败（{}）", errors.join("；")),
    ))
}

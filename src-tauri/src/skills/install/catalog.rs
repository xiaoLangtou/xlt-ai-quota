use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::sources_registry::get_source_skills;
use super::super::config::SkillsPaths;

const STARS_TTL: Duration = Duration::from_secs(60 * 60);

/// 精选目录条目（统一 Schema，安装复用 GitHub 远程流程）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSkill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub repo: String,
    pub install_ref: String,
    #[serde(default)]
    pub homepage: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_in_repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub has_scripts: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

pub type StarsCache = Mutex<Option<(HashMap<String, u64>, Instant)>>;

fn curated() -> Vec<CatalogSkill> {
    serde_json::from_str(include_str!("catalog.json")).unwrap_or_default()
}

async fn fetch_repo_stars(repo: &str) -> Option<u64> {
    let client = reqwest::Client::builder()
        .user_agent("xlt-workbench")
        .build()
        .ok()?;
    let response = client
        .get(format!("https://api.github.com/repos/{repo}"))
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let data: serde_json::Value = response.json().await.ok()?;
    data.get("stargazers_count").and_then(|value| value.as_u64())
}

async fn load_stars(cache: &StarsCache) -> HashMap<String, u64> {
    if let Ok(guard) = cache.lock() {
        if let Some((stars, fetched_at)) = guard.as_ref() {
            if fetched_at.elapsed() < STARS_TTL {
                return stars.clone();
            }
        }
    }
    let mut repos: Vec<String> = curated().into_iter().map(|item| item.repo).collect();
    repos.sort();
    repos.dedup();
    let mut stars = HashMap::new();
    for repo in repos {
        if let Some(count) = fetch_repo_stars(&repo).await {
            stars.insert(repo, count);
        }
    }
    if let Ok(mut guard) = cache.lock() {
        *guard = Some((stars.clone(), Instant::now()));
    }
    stars
}

/// 返回完整目录：内置精选（附 star 缓存）+ 已接入数据源的扫描索引。
pub async fn get_catalog(paths: &SkillsPaths, stars_cache: &StarsCache) -> Vec<CatalogSkill> {
    let stars = load_stars(stars_cache).await;
    let mut curated: Vec<CatalogSkill> = curated()
        .into_iter()
        .map(|mut item| {
            item.source_type = Some("curated".to_owned());
            item.stars = stars.get(&item.repo).copied();
            item
        })
        .collect();
    // 源索引拉取失败不影响精选展示。
    let sourced = get_source_skills(paths).await.unwrap_or_default();
    curated.extend(sourced);
    curated
}

use serde::Deserialize;
use serde_json::Value;

use super::super::error::McpError;
use super::super::types::{CatalogSource, InstallInfo, LanguageInfo, McpPackage};

pub const ID: &str = "gitcode";
pub const DEFAULT_CHANNEL: &str = "6a55a5361944323916325288";
const BASE: &str = "https://web-api.gitcode.com/api/v1/agg/index";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36";
/// 单次操作最多连续补页数（过滤后仍不足时）。
const MAX_PAGES: u32 = 3;

#[derive(Debug, Clone, Copy)]
pub enum Sort {
    All,
    Star,
    Updated,
}

impl Sort {
    pub fn as_str(self) -> &'static str {
        match self {
            Sort::All => "all",
            Sort::Star => "star_desc",
            Sort::Updated => "updated_at_desc",
        }
    }

    pub fn parse(raw: Option<&str>) -> Self {
        match raw.unwrap_or("all") {
            "star_desc" => Sort::Star,
            "updated_at_desc" => Sort::Updated,
            _ => Sort::All,
        }
    }
}

#[derive(Debug, Deserialize)]
struct AggPage {
    #[serde(default)]
    content: Vec<AggItem>,
    #[serde(default)]
    page_count: u64,
}

#[derive(Debug, Deserialize)]
struct AggItem {
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    star_count: Option<u64>,
    #[serde(default)]
    fork_count: Option<u64>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    language: Vec<Value>,
    #[serde(default)]
    topic: Vec<Topic>,
    #[serde(default)]
    is_mirrors: Option<bool>,
    #[serde(default)]
    is_gh_mirrors: Option<bool>,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Topic {
    #[serde(default)]
    name: Option<String>,
}

pub fn source() -> CatalogSource {
    CatalogSource {
        id: ID.to_owned(),
        label: "GitCode（AtomGit）".to_owned(),
        kind: "gitcode".to_owned(),
        url: Some(BASE.to_owned()),
        channel_id: Some(DEFAULT_CHANNEL.to_owned()),
        sub_channel_id: None,
        enabled: true,
        builtin: true,
        last_sync_at: None,
        last_error: None,
    }
}

fn language_of(item: &AggItem) -> Option<LanguageInfo> {
    let first = item.language.first()?;
    match first {
        Value::String(name) => Some(LanguageInfo {
            name: name.clone(),
            color: item
                .language
                .get(1)
                .and_then(Value::as_str)
                .map(str::to_owned),
        }),
        Value::Object(map) => map
            .get("name")
            .and_then(Value::as_str)
            .map(|name| LanguageInfo {
                name: name.to_owned(),
                color: map
                    .get("color")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }),
        _ => None,
    }
}

const LICENSE_TAGS: &[&str] = &[
    "mit",
    "apache-2.0",
    "apache 2.0",
    "gpl-3.0",
    "gpl-2.0",
    "bsd-3-clause",
    "bsd-2-clause",
    "mpl-2.0",
    "agpl-3.0",
    "lgpl-3.0",
];

fn map_item(item: &AggItem) -> Option<McpPackage> {
    let namespace = item.namespace.as_ref()?.trim();
    if namespace.is_empty() {
        return None;
    }
    let repo = namespace.to_lowercase();
    let homepage = item
        .url
        .clone()
        .unwrap_or_else(|| format!("https://gitcode.com/{namespace}"));
    let name = item
        .name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| repo.split('/').next_back().map(str::to_owned))
        .unwrap_or_else(|| repo.clone());

    let mut tags: Vec<String> = Vec::new();
    let mut platforms: Vec<String> = Vec::new();
    let mut license: Option<String> = None;
    let mut is_server = false;
    let mut is_client = false;
    for topic in &item.topic {
        let Some(raw) = &topic.name else { continue };
        let text = raw.trim();
        let lower = text.to_lowercase();
        if lower.starts_with("频道-") {
            continue;
        }
        if lower.contains("mcp client") {
            is_client = true;
        } else if lower.contains("mcp") {
            is_server = true;
        } else if LICENSE_TAGS.contains(&lower.as_str()) {
            license = Some(text.to_owned());
        } else if matches!(lower.as_str(), "windows" | "macos" | "linux") {
            platforms.push(text.to_owned());
        } else {
            tags.push(text.to_owned());
        }
    }

    let kind = if is_server {
        "server"
    } else if is_client {
        "client"
    } else {
        // 兜底：topic 未标注时按名称 / 描述判断，避免整源为空。
        let haystack = format!(
            "{} {} {}",
            name,
            item.description.clone().unwrap_or_default(),
            repo
        )
        .to_lowercase();
        if haystack.contains("mcp") {
            "server"
        } else {
            return None;
        }
    };

    Some(McpPackage {
        id: format!("{ID}:{repo}"),
        name,
        description: item.description.clone().unwrap_or_default(),
        source: ID.to_owned(),
        homepage,
        repo: Some(repo),
        tags,
        language: language_of(item),
        license,
        platforms,
        stars: item.star_count,
        forks: item.fork_count,
        updated_at: item.updated_at.clone(),
        verified: false,
        mirror: item.is_mirrors.unwrap_or(false) || item.is_gh_mirrors.unwrap_or(false),
        kind: kind.to_owned(),
        install: InstallInfo::Manual,
        installed_agents: Vec::new(),
        favorite: false,
    })
}

async fn fetch_page(
    client: &reqwest::Client,
    cfg: &CatalogSource,
    sort: Sort,
    page: u32,
    keyword: Option<&str>,
) -> Result<AggPage, McpError> {
    let channel = cfg
        .channel_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_CHANNEL.to_owned());
    let sub = cfg.sub_channel_id.clone().unwrap_or_default();
    let page_text = page.to_string();
    let mut query: Vec<(String, String)> = vec![
        ("channel_id".into(), channel.clone()),
        ("sub_channel_id".into(), sub),
        ("sort".into(), sort.as_str().to_owned()),
        ("repo_type".into(), "-1".into()),
        ("m_code".into(), "recommendList".into()),
        ("d_code".into(), "projects".into()),
        ("c_id".into(), channel),
        ("page".into(), page_text),
        ("per_page".into(), "18".into()),
    ];
    if let Some(value) = keyword.filter(|value| !value.trim().is_empty()) {
        query.push(("keyword".into(), value.to_owned()));
    }

    let response = client
        .get(BASE)
        .query(&query)
        .header("Referer", "https://gitcode.com/")
        .header("User-Agent", UA)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| McpError::upstream(format!("GitCode 请求失败：{error}")))?;

    match response.status().as_u16() {
        200 => response
            .json::<AggPage>()
            .await
            .map_err(|error| McpError::upstream(format!("GitCode 响应解析失败：{error}"))),
        418 | 403 | 429 => Err(McpError::forbidden("GitCode 数据源暂时不可用（被拦截）")),
        status => Err(McpError::upstream(format!("GitCode 返回 HTTP {status}"))),
    }
}

/// 拉取多页并映射为库条目；过滤后不足 18 条时继续补页（最多 3 页）。
pub async fn fetch_packages(
    client: &reqwest::Client,
    cfg: &CatalogSource,
    sort: Sort,
    keyword: Option<&str>,
) -> Result<Vec<McpPackage>, McpError> {
    let mut packages: Vec<McpPackage> = Vec::new();
    let mut page = 1u32;
    loop {
        let agg = fetch_page(client, cfg, sort, page, keyword).await?;
        let empty = agg.content.is_empty();
        for item in &agg.content {
            if let Some(package) = map_item(item) {
                if !packages.iter().any(|existing| existing.id == package.id) {
                    packages.push(package);
                }
            }
        }
        let servers = packages
            .iter()
            .filter(|package| package.kind == "server")
            .count();
        if empty || servers >= 18 || page >= MAX_PAGES || page as u64 >= agg.page_count {
            break;
        }
        page += 1;
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    Ok(packages)
}

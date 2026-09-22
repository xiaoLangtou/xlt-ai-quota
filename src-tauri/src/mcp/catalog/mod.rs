use std::collections::HashMap;
use std::time::Duration;

use tauri::AppHandle;

use crate::skills::install::security;

use super::error::McpError;
use super::metadata;
use super::parse;
use super::types::{
    CatalogFilter, CatalogSource, McpPackage, McpService, ReadmeResult, SourceSaveRequest,
};

pub mod builtin;
pub mod custom;
pub mod gitcode;
pub mod registry;

const USER_AGENT: &str = "xlt-workbench/0.1.3";
const README_LIMIT: usize = 40_000;

/// 构建库同步用的 HTTP 客户端。
pub fn http_client() -> Result<reqwest::Client, McpError> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| McpError::internal(error.to_string()))
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

/// 随应用内置的源（精选 + 官方 Registry + GitCode）。
pub fn default_sources() -> Vec<CatalogSource> {
    vec![builtin::source(), registry::source(), gitcode::source()]
}

/// 列出全部源：内置源合并本地状态，另有用户自定义源。
pub fn all_sources(app: &AppHandle) -> Result<Vec<CatalogSource>, McpError> {
    let states = metadata::load_source_states(app)?;
    let mut list = default_sources();
    for source in list.iter_mut() {
        if let Some((saved, error)) = states.get(&source.id) {
            source.enabled = saved.enabled;
            if saved.channel_id.is_some() {
                source.channel_id = saved.channel_id.clone();
            }
            if saved.sub_channel_id.is_some() {
                source.sub_channel_id = saved.sub_channel_id.clone();
            }
            if saved.url.is_some() {
                source.url = saved.url.clone();
            }
            source.last_sync_at = saved.last_sync_at.clone();
            source.last_error = error.clone().or_else(|| saved.last_error.clone());
        }
    }
    for (id, (saved, error)) in states {
        if saved.kind == "custom" && !list.iter().any(|source| source.id == id) {
            let mut source = saved;
            source.last_error = error.or(source.last_error);
            list.push(source);
        }
    }
    Ok(list)
}

/// 拉取单个源（不落库）。
async fn fetch_source(
    client: &reqwest::Client,
    source: &CatalogSource,
    query: Option<&str>,
) -> Result<Vec<McpPackage>, McpError> {
    match source.kind.as_str() {
        "builtin" => Ok(builtin::packages()),
        "registry" => registry::fetch_packages(client).await,
        "gitcode" => {
            gitcode::fetch_packages(client, source, gitcode::Sort::All, query).await
        }
        "custom" => custom::fetch_packages(client, source).await,
        other => Err(McpError::bad_request(format!("未知数据源类型：{other}"))),
    }
}

/// 同步单个源并写入缓存；失败时保留旧缓存并记录错误。
pub async fn sync_source(
    app: &AppHandle,
    client: &reqwest::Client,
    source: &CatalogSource,
) -> Result<Vec<McpPackage>, McpError> {
    match fetch_source(client, source, None).await {
        Ok(packages) => {
            metadata::replace_catalog(app, &source.id, &packages)?;
            let mut updated = source.clone();
            updated.last_sync_at = Some(now());
            updated.last_error = None;
            metadata::save_source_state(app, &updated, None)?;
            Ok(packages)
        }
        Err(error) => {
            let mut updated = source.clone();
            updated.last_error = Some(error.message.clone());
            let _ = metadata::save_source_state(app, &updated, Some(&error.message));
            Err(error)
        }
    }
}

/// 同步全部启用的源；单源失败不影响其他源。返回更新后的源列表。
pub async fn sync_all(
    app: &AppHandle,
    client: &reqwest::Client,
    only: Option<&str>,
) -> Result<Vec<CatalogSource>, McpError> {
    let sources = all_sources(app)?;
    for source in &sources {
        if let Some(id) = only {
            if source.id != id {
                continue;
            }
        } else if !source.enabled {
            continue;
        }
        let _ = sync_source(app, client, source).await;
    }
    all_sources(app)
}

/// 跨源去重：以规范化 repo 为键合并镜像与原仓库，优先保留信息更完整的一条。
fn information_score(package: &McpPackage) -> i64 {
    let mut score = 0i64;
    if !package.description.is_empty() {
        score += 2;
    }
    if package.install_ready() {
        score += 3;
    }
    if package.verified {
        score += 2;
    }
    if package.license.is_some() {
        score += 1;
    }
    score + package.stars.unwrap_or(0).min(1000) as i64
}

trait InstallReady {
    fn install_ready(&self) -> bool;
}

impl InstallReady for McpPackage {
    fn install_ready(&self) -> bool {
        matches!(self.install, super::types::InstallInfo::Ready { .. })
    }
}

fn dedupe(packages: Vec<McpPackage>) -> Vec<McpPackage> {
    let mut best: HashMap<String, McpPackage> = HashMap::new();
    let mut no_repo: Vec<McpPackage> = Vec::new();
    for package in packages {
        match package.repo.clone() {
            Some(repo) if !repo.trim().is_empty() => {
                let key = repo.to_lowercase();
                match best.get(&key) {
                    Some(existing) if information_score(existing) >= information_score(&package) => {
                        // 保留已有的镜像标记信息。
                        if let Some(entry) = best.get_mut(&key) {
                            if package.mirror {
                                entry.mirror = true;
                            }
                        }
                    }
                    _ => {
                        best.insert(key, package);
                    }
                }
            }
            _ => no_repo.push(package),
        }
    }
    let mut out: Vec<McpPackage> = best.into_values().collect();
    out.extend(no_repo);
    out
}

fn matches_query(package: &McpPackage, query: &str) -> bool {
    let needle = query.to_lowercase();
    package.name.to_lowercase().contains(&needle)
        || package.description.to_lowercase().contains(&needle)
        || package
            .repo
            .as_deref()
            .map(|repo| repo.to_lowercase().contains(&needle))
            .unwrap_or(false)
        || package
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(&needle))
}

fn runtime_kinds(package: &McpPackage) -> Vec<String> {
    match &package.install {
        super::types::InstallInfo::Ready { runtimes, .. } => {
            runtimes.iter().map(|runtime| runtime.kind.clone()).collect()
        }
        super::types::InstallInfo::Manual => Vec::new(),
    }
}

/// 服务是否匹配库条目：优先来源 id，其次命令 / 名称。
pub fn package_matches_service(package: &McpPackage, service: &McpService) -> bool {
    if service.source_id.as_deref() == Some(package.id.as_str()) {
        return true;
    }
    if service.name.eq_ignore_ascii_case(&package.name) {
        return true;
    }
    if let Some(command) = &service.command {
        let mut haystack = command.to_lowercase();
        for arg in &service.args {
            haystack.push(' ');
            haystack.push_str(&arg.to_lowercase());
        }
        let mut candidates = vec![package.name.to_lowercase()];
        if let Some(repo) = &package.repo {
            if let Some(base) = repo.rsplit('/').next() {
                candidates.push(base.to_lowercase());
            }
        }
        if candidates
            .iter()
            .any(|candidate| candidate.len() > 2 && haystack.contains(candidate))
        {
            return true;
        }
    }
    false
}

fn installed_agents(package: &McpPackage, services: &[McpService]) -> Vec<String> {
    let mut agents: Vec<String> = Vec::new();
    for service in services {
        if !package_matches_service(package, service) {
            continue;
        }
        for instance in &service.instances {
            if !agents.contains(&instance.agent) {
                agents.push(instance.agent.clone());
            }
        }
    }
    agents
}

/// 读取缓存并按筛选 / 排序返回库列表。
pub fn list_packages(
    app: &AppHandle,
    filter: &CatalogFilter,
    services: &[McpService],
) -> Result<Vec<McpPackage>, McpError> {
    let sources = all_sources(app)?;
    let mut packages: Vec<McpPackage> = Vec::new();
    for source in &sources {
        if let Some(id) = &filter.source {
            if &source.id != id {
                continue;
            }
        } else if !source.enabled {
            continue;
        }
        packages.extend(metadata::catalog_by_source(app, &source.id)?);
    }

    let favorites = metadata::favorite_ids(app)?;
    let query = filter
        .query
        .as_ref()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let runtime = filter
        .runtime
        .as_ref()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());

    let mut packages = dedupe(packages);
    packages.retain(|package| {
        if let Some(query) = &query {
            if !matches_query(package, query) {
                return false;
            }
        }
        if filter.favorites_only && !favorites.contains(&package.id) {
            return false;
        }
        if let Some(kind) = &filter.kind {
            if !kind.is_empty() && &package.kind != kind {
                return false;
            }
        } else if package.kind == "client" && !filter.show_clients {
            return false;
        }
        if let Some(runtime) = &runtime {
            let kinds = runtime_kinds(package);
            if !kinds.iter().any(|kind| kind == runtime) {
                return false;
            }
        }
        true
    });

    for package in packages.iter_mut() {
        package.favorite = favorites.contains(&package.id);
        package.installed_agents = installed_agents(package, services);
    }

    match filter.sort.as_deref().unwrap_or("all") {
        "star" => packages.sort_by(|a, b| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0))),
        "updated" => packages.sort_by(|a, b| b.updated_at.cmp(&a.updated_at)),
        "favorites" => packages.sort_by_key(|package| !package.favorite),
        _ => {
            packages.sort_by(|a, b| {
                b.verified
                    .cmp(&a.verified)
                    .then_with(|| b.stars.unwrap_or(0).cmp(&a.stars.unwrap_or(0)))
            });
        }
    }

    let offset = filter.offset.unwrap_or(0);
    let limit = filter.limit.unwrap_or(usize::MAX);
    Ok(packages.into_iter().skip(offset).take(limit).collect())
}

/// 保存 / 更新自定义源或内置源配置。
pub fn save_source(
    app: &AppHandle,
    request: &SourceSaveRequest,
) -> Result<CatalogSource, McpError> {
    if request.label.trim().is_empty() {
        return Err(McpError::bad_request("数据源名称不能为空"));
    }
    let kind = request.kind.clone().unwrap_or_else(|| "custom".to_owned());
    if kind == "custom" {
        let url = request
            .url
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| McpError::bad_request("自定义源必须填写 JSON URL"))?;
        if !url.starts_with("https://") {
            return Err(McpError::bad_request("自定义源仅允许 https 地址"));
        }
    }
    let id = request.id.clone().unwrap_or_else(|| {
        format!(
            "custom:{}",
            &uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                request.url.clone().unwrap_or_default().as_bytes()
            )
            .to_string()[..8]
        )
    });
    let source = CatalogSource {
        id,
        label: request.label.trim().to_owned(),
        kind,
        url: request.url.clone(),
        channel_id: request.channel_id.clone(),
        sub_channel_id: request.sub_channel_id.clone(),
        enabled: request.enabled,
        builtin: false,
        last_sync_at: None,
        last_error: None,
    };
    metadata::save_source_state(app, &source, None)?;
    Ok(source)
}

/// 删除自定义源（内置源不可删除）及其缓存。
pub fn remove_source(app: &AppHandle, id: &str) -> Result<bool, McpError> {
    if default_sources().iter().any(|source| source.id == id) {
        return Err(McpError::bad_request("内置源不可删除"));
    }
    metadata::remove_source(app, id)
}

/// 拉取 README 并解析其中的安装片段（懒加载，失败降级）。
pub async fn readme(client: &reqwest::Client, package: &McpPackage) -> ReadmeResult {
    let Some(repo) = package.repo.clone().filter(|value| value.contains('/')) else {
        return ReadmeResult {
            available: false,
            text: String::new(),
            snippets: Vec::new(),
            source_url: Some(package.homepage.clone()),
            error: Some("该条目没有关联仓库，请在浏览器中打开主页".to_owned()),
        };
    };
    for branch in ["HEAD", "main", "master"] {
        let url = format!("https://raw.githubusercontent.com/{repo}/{branch}/README.md");
        if security::assert_public_host(&url).is_err() {
            break;
        }
        let Ok(response) = client.get(&url).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(text) = response.text().await else {
            continue;
        };
        let truncated: String = text.chars().take(README_LIMIT).collect();
        let snippets = parse::parse_text(&truncated);
        return ReadmeResult {
            available: true,
            text: truncated,
            snippets,
            source_url: Some(url),
            error: None,
        };
    }
    ReadmeResult {
        available: false,
        text: String::new(),
        snippets: Vec::new(),
        source_url: Some(package.homepage.clone()),
        error: Some("README 获取失败，请在浏览器中打开仓库".to_owned()),
    }
}

/// 计算库条目的来源等级（用于风险提示）。
pub fn source_level(package: &McpPackage) -> &'static str {
    if package.source == "builtin" {
        "内置精选"
    } else if package.verified {
        "官方 Registry 已验证"
    } else if package.source == "custom" {
        "自定义源"
    } else {
        "社区"
    }
}

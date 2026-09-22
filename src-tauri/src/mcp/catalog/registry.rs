use serde::Deserialize;
use serde_json::Value;

use super::super::error::McpError;
use super::super::types::{CatalogSource, InstallInfo, McpPackage, Runtime};

pub const ID: &str = "registry";
const BASE: &str = "https://registry.modelcontextprotocol.io/v0/servers";
/// 单次同步拉取条数（该注册表以游标分页）。
const PAGE_LIMIT: u32 = 100;

#[derive(Debug, Deserialize)]
struct RegistryPage {
    #[serde(default)]
    servers: Vec<RegistryEntry>,
}

#[derive(Debug, Deserialize)]
struct RegistryEntry {
    #[serde(default)]
    server: RegistryServer,
}

#[derive(Debug, Default, Deserialize)]
struct RegistryServer {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    repository: Option<RegistryRepo>,
    #[serde(default)]
    packages: Vec<Value>,
    #[serde(default)]
    remotes: Vec<RegistryRemote>,
}

#[derive(Debug, Deserialize)]
struct RegistryRepo {
    #[serde(default)]
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegistryRemote {
    #[serde(default)]
    url: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
}

pub fn source() -> CatalogSource {
    CatalogSource {
        id: ID.to_owned(),
        label: "官方 MCP Registry".to_owned(),
        kind: "registry".to_owned(),
        url: Some(BASE.to_owned()),
        channel_id: None,
        sub_channel_id: None,
        enabled: true,
        builtin: true,
        last_sync_at: None,
        last_error: None,
    }
}

fn normalize_repo(url: &str) -> Option<String> {
    let mut text = url.trim().trim_end_matches('/').to_owned();
    for prefix in ["https://github.com/", "http://github.com/", "git+https://github.com/"] {
        if let Some(rest) = text.strip_prefix(prefix) {
            text = rest.to_owned();
            break;
        }
    }
    text = text.strip_suffix(".git").unwrap_or(&text).to_owned();
    let parts: Vec<&str> = text.split('/').collect();
    if parts.len() >= 2 {
        Some(format!("{}/{}", parts[0], parts[1]))
    } else {
        None
    }
}

fn display_name(qualified: &str) -> String {
    qualified
        .rsplit('/')
        .next()
        .unwrap_or(qualified)
        .rsplit('.')
        .next()
        .unwrap_or(qualified)
        .to_owned()
}

fn runtime_from_package(package: &Value) -> Option<Runtime> {
    let registry_type = package
        .get("registryType")
        .or_else(|| package.get("registry_name"))
        .or_else(|| package.get("registryName"))
        .and_then(Value::as_str)
        .unwrap_or("npm")
        .to_lowercase();
    let identifier = package
        .get("identifier")
        .or_else(|| package.get("name"))
        .and_then(Value::as_str)?;
    let version = package
        .get("version")
        .and_then(Value::as_str)
        .map(str::to_owned);
    match registry_type.as_str() {
        "pypi" | "python" => Some(Runtime {
            kind: "uvx".to_owned(),
            command: Some("uvx".to_owned()),
            args: vec![identifier.to_owned()],
            url: None,
            version,
        }),
        "oci" | "docker" => Some(Runtime {
            kind: "docker".to_owned(),
            command: Some("docker".to_owned()),
            args: vec!["run".to_owned(), "-i".to_owned(), "--rm".to_owned(), identifier.to_owned()],
            url: None,
            version,
        }),
        _ => Some(Runtime {
            kind: "npx".to_owned(),
            command: Some("npx".to_owned()),
            args: vec!["-y".to_owned(), identifier.to_owned()],
            url: None,
            version,
        }),
    }
}

fn map_entry(entry: &RegistryEntry) -> Option<McpPackage> {
    let server = &entry.server;
    if server.name.trim().is_empty() {
        return None;
    }
    let qualified = server.name.clone();
    let repo = server
        .repository
        .as_ref()
        .and_then(|repository| repository.url.as_deref())
        .and_then(normalize_repo);
    let homepage = server
        .repository
        .as_ref()
        .and_then(|repository| repository.url.clone())
        .unwrap_or_else(|| format!("https://registry.modelcontextprotocol.io/servers/{qualified}"));

    let mut runtimes: Vec<Runtime> = server
        .packages
        .iter()
        .filter_map(runtime_from_package)
        .collect();
    for remote in &server.remotes {
        if let Some(url) = &remote.url {
            runtimes.push(Runtime {
                kind: "remote".to_owned(),
                command: None,
                args: Vec::new(),
                url: Some(url.clone()),
                version: None,
            });
        }
    }
    let install = if runtimes.is_empty() {
        InstallInfo::Manual
    } else {
        InstallInfo::Ready {
            runtimes,
            env_spec: Vec::new(),
        }
    };

    Some(McpPackage {
        id: format!("{ID}:{}", qualified.to_lowercase()),
        name: display_name(&qualified),
        description: server.description.clone().unwrap_or_default(),
        source: ID.to_owned(),
        homepage,
        repo,
        tags: Vec::new(),
        language: None,
        license: None,
        platforms: Vec::new(),
        stars: None,
        forks: None,
        updated_at: None,
        verified: true,
        mirror: false,
        kind: "server".to_owned(),
        install,
        installed_agents: Vec::new(),
        favorite: false,
    })
}

/// 拉取官方 Registry 首页条目。
pub async fn fetch_packages(client: &reqwest::Client) -> Result<Vec<McpPackage>, McpError> {
    let response = client
        .get(BASE)
        .query(&[("limit", PAGE_LIMIT.to_string())])
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| McpError::upstream(format!("官方 Registry 请求失败：{error}")))?;
    if !response.status().is_success() {
        return Err(McpError::upstream(format!(
            "官方 Registry 返回 HTTP {}",
            response.status().as_u16()
        )));
    }
    let page = response
        .json::<RegistryPage>()
        .await
        .map_err(|error| McpError::upstream(format!("官方 Registry 响应解析失败：{error}")))?;
    Ok(page.servers.iter().filter_map(map_entry).collect())
}

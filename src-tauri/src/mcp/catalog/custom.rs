use serde_json::Value;

use crate::skills::install::security;

use super::super::error::McpError;
use super::super::types::{CatalogSource, EnvVar, InstallInfo, LanguageInfo, McpPackage, Runtime};

pub const ID: &str = "custom";

fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn strings(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

fn runtime_from_value(value: &Value) -> Option<Runtime> {
    let kind = string(value.get("type"))
        .or_else(|| string(value.get("kind")))
        .unwrap_or_else(|| "npx".to_owned());
    Some(Runtime {
        kind,
        command: string(value.get("command")),
        args: strings(value.get("args")),
        url: string(value.get("url")),
        version: string(value.get("version")),
    })
}

fn install_from_value(value: Option<&Value>) -> InstallInfo {
    let Some(object) = value.and_then(Value::as_object) else {
        return InstallInfo::Manual;
    };
    let level = object
        .get("level")
        .and_then(Value::as_str)
        .unwrap_or("manual");
    if level != "ready" {
        return InstallInfo::Manual;
    }
    let runtimes = object
        .get("runtimes")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(runtime_from_value).collect())
        .unwrap_or_default();
    let env_spec = object
        .get("envSpec")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = string(item.get("name"))?;
                    Some(EnvVar {
                        name,
                        description: string(item.get("description")),
                        required: item
                            .get("required")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        secret: item.get("secret").and_then(Value::as_bool).unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    InstallInfo::Ready { runtimes, env_spec }
}

fn map_item(source_id: &str, value: &Value) -> Option<McpPackage> {
    let object = value.as_object()?;
    let name = string(object.get("name"))?;
    if name.trim().is_empty() {
        return None;
    }
    let id = string(object.get("id"))
        .unwrap_or_else(|| format!("{source_id}:{}", name.to_lowercase().replace(' ', "-")));
    Some(McpPackage {
        id,
        name,
        description: string(object.get("description")).unwrap_or_default(),
        source: source_id.to_owned(),
        homepage: string(object.get("homepage")).unwrap_or_default(),
        repo: string(object.get("repo")),
        tags: strings(object.get("tags")),
        language: object.get("language").and_then(|value| {
            if let Some(name) = value.as_str() {
                Some(LanguageInfo {
                    name: name.to_owned(),
                    color: None,
                })
            } else {
                value.as_object().and_then(|map| {
                    string(map.get("name")).map(|name| LanguageInfo {
                        name,
                        color: string(map.get("color")),
                    })
                })
            }
        }),
        license: string(object.get("license")),
        platforms: strings(object.get("platforms")),
        stars: object.get("stars").and_then(Value::as_u64),
        forks: object.get("forks").and_then(Value::as_u64),
        updated_at: string(object.get("updatedAt")),
        verified: object.get("verified").and_then(Value::as_bool).unwrap_or(false),
        mirror: object.get("mirror").and_then(Value::as_bool).unwrap_or(false),
        kind: string(object.get("kind")).unwrap_or_else(|| "server".to_owned()),
        install: install_from_value(object.get("install")),
        installed_agents: Vec::new(),
        favorite: false,
    })
}

/// 拉取用户自定义源（约定格式的 JSON URL）。
pub async fn fetch_packages(
    client: &reqwest::Client,
    source: &CatalogSource,
) -> Result<Vec<McpPackage>, McpError> {
    let url = source
        .url
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| McpError::bad_request("自定义源缺少 URL"))?;
    // 复用 Skills 的 SSRF 防护：拒绝私网 / 本地地址。
    security::assert_public_host_resolved(url)
        .await
        .map_err(|error| McpError::bad_request(error.message))?;
    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| McpError::upstream(format!("自定义源请求失败：{error}")))?;
    if !response.status().is_success() {
        return Err(McpError::upstream(format!(
            "自定义源返回 HTTP {}",
            response.status().as_u16()
        )));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|error| McpError::upstream(format!("自定义源响应解析失败：{error}")))?;
    let items = value
        .as_array()
        .cloned()
        .or_else(|| value.get("packages").and_then(Value::as_array).cloned())
        .or_else(|| value.get("servers").and_then(Value::as_array).cloned())
        .unwrap_or_default();
    Ok(items
        .iter()
        .filter_map(|item| map_item(&source.id, item))
        .collect())
}

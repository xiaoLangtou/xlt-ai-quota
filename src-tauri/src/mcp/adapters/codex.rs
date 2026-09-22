use std::collections::BTreeMap;
use std::path::PathBuf;

use toml_edit::{Array, DocumentMut, Item, Table, value};

use super::super::error::McpError;
use super::super::paths::McpPaths;
use super::super::types::{AdapterEntry, McpServer, McpTransport, ServerOp};
use super::McpAdapter;

/// Codex：`~/.codex/config.toml` 的 `[mcp_servers.<name>]`。
/// 用 `toml_edit` 保留注释与未知键；不原生支持停用字段（走暂存区）。
pub struct CodexAdapter;

pub const ADAPTER: CodexAdapter = CodexAdapter;

impl McpAdapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn label(&self) -> &'static str {
        "Codex"
    }

    fn global_path(&self, paths: &McpPaths) -> Option<PathBuf> {
        Some(paths.home.join(".codex").join("config.toml"))
    }

    fn supports_disable(&self) -> bool {
        false
    }

    fn supports_remote(&self) -> bool {
        true
    }

    fn parse(&self, raw: &str) -> Result<Vec<AdapterEntry>, McpError> {
        if raw.trim().is_empty() {
            return Ok(Vec::new());
        }
        let doc: DocumentMut = raw
            .parse()
            .map_err(|error| McpError::bad_request(format!("Codex TOML 解析失败：{error}")))?;
        let Some(servers) = doc.get("mcp_servers").and_then(Item::as_table_like) else {
            return Ok(Vec::new());
        };
        let mut entries = Vec::new();
        for (name, item) in servers.iter() {
            let Some(table) = item.as_table_like() else {
                continue;
            };
            let command = table
                .get("command")
                .and_then(Item::as_str)
                .map(str::to_owned);
            let args = table
                .get("args")
                .and_then(Item::as_array)
                .map(|array| {
                    array
                        .iter()
                        .filter_map(|value| value.as_str())
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default();
            let mut env = BTreeMap::new();
            if let Some(env_table) = table.get("env").and_then(Item::as_table_like) {
                for (key, value) in env_table.iter() {
                    if let Some(text) = value.as_str() {
                        env.insert(key.to_owned(), text.to_owned());
                    }
                }
            }
            let url = table.get("url").and_then(Item::as_str).map(str::to_owned);
            let transport = if url.is_some() {
                McpTransport::Http
            } else {
                McpTransport::Stdio
            };
            entries.push(AdapterEntry {
                enabled: true,
                server: McpServer {
                    name: name.to_owned(),
                    transport,
                    command,
                    args,
                    env,
                    url,
                    headers: BTreeMap::new(),
                    extra: serde_json::Map::new(),
                },
            });
        }
        Ok(entries)
    }

    fn apply(&self, raw: &str, ops: &[ServerOp]) -> Result<String, McpError> {
        let mut doc: DocumentMut = if raw.trim().is_empty() {
            DocumentMut::new()
        } else {
            raw.parse()
                .map_err(|error| McpError::bad_request(format!("Codex TOML 解析失败：{error}")))?
        };
        if !doc.contains_key("mcp_servers") {
            doc["mcp_servers"] = Item::Table(Table::new());
        }
        let root = doc["mcp_servers"]
            .as_table_mut()
            .ok_or_else(|| McpError::bad_request("Codex `mcp_servers` 不是表"))?;

        for op in ops {
            match op {
                ServerOp::Upsert { server } => {
                    let mut table = root
                        .get(&server.name)
                        .and_then(Item::as_table)
                        .cloned()
                        .unwrap_or_else(Table::new);
                    table.remove("command");
                    table.remove("args");
                    table.remove("env");
                    table.remove("url");
                    if let Some(command) = &server.command {
                        table.insert("command", value(command.as_str()));
                    }
                    if !server.args.is_empty() {
                        let mut array = Array::new();
                        for arg in &server.args {
                            array.push(arg.as_str());
                        }
                        table.insert("args", value(array));
                    }
                    if !server.env.is_empty() {
                        let mut env = Table::new();
                        for (key, item) in &server.env {
                            env.insert(key, value(item.as_str()));
                        }
                        table.insert("env", Item::Table(env));
                    }
                    if let Some(url) = &server.url {
                        table.insert("url", value(url.as_str()));
                    }
                    root.insert(&server.name, Item::Table(table));
                }
                ServerOp::Remove { name } => {
                    root.remove(name);
                }
                ServerOp::SetEnabled { .. } => {
                    // Codex 无原生停用字段，停用 / 恢复由暂存区处理。
                }
            }
        }
        Ok(doc.to_string())
    }
}

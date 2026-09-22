use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use super::super::error::McpError;
use super::super::paths::McpPaths;
use super::super::types::{AdapterEntry, McpServer, McpTransport, ServerOp};
use super::McpAdapter;

/// JSON 配置的两种字段形状。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonShape {
    /// 通用：command / args / env，或 url / headers。
    CommandUrl,
    /// OpenCode：type: local/remote，局部 command 为数组、环境字段为 environment、含 enabled。
    OpenCode,
}

/// 一个 JSON 适配器的静态规格。
#[derive(Debug, Clone, Copy)]
pub struct JsonSpec {
    pub id: &'static str,
    pub label: &'static str,
    /// 服务容器键（`mcpServers` / `mcp`）。
    pub container: &'static str,
    /// 原生停用字段名（无则不原生支持）。
    pub disable_key: Option<&'static str>,
    /// 全局配置相对 home 的路径段。
    pub global_rel: &'static [&'static str],
    /// 项目级配置相对项目根的路径。
    pub project_rel: Option<&'static str>,
    pub shape: JsonShape,
}

pub struct JsonAdapter(pub &'static JsonSpec);

pub fn join_rel(base: &Path, parts: &[&str]) -> PathBuf {
    let mut path = base.to_path_buf();
    for part in parts {
        path.push(part);
    }
    path
}

impl McpAdapter for JsonAdapter {
    fn id(&self) -> &'static str {
        self.0.id
    }

    fn label(&self) -> &'static str {
        self.0.label
    }

    fn global_path(&self, paths: &McpPaths) -> Option<PathBuf> {
        Some(join_rel(&paths.home, self.0.global_rel))
    }

    fn project_path(&self, project: &Path) -> Option<PathBuf> {
        self.0.project_rel.map(|rel| project.join(rel))
    }

    fn supports_disable(&self) -> bool {
        self.0.disable_key.is_some() || self.0.shape == JsonShape::OpenCode
    }

    fn supports_remote(&self) -> bool {
        true
    }

    fn parse(&self, raw: &str) -> Result<Vec<AdapterEntry>, McpError> {
        parse_servers(self.0, raw)
    }

    fn apply(&self, raw: &str, ops: &[ServerOp]) -> Result<String, McpError> {
        apply_ops(self.0, raw, ops)
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn string_map(value: Option<&Value>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    if let Some(object) = value.and_then(Value::as_object) {
        for (key, item) in object {
            if let Some(text) = item.as_str() {
                out.insert(key.clone(), text.to_owned());
            } else if item.is_number() || item.is_boolean() {
                out.insert(key.clone(), item.to_string());
            }
        }
    }
    out
}

fn from_command_url(spec: &JsonSpec, name: &str, obj: &Map<String, Value>) -> AdapterEntry {
    let command = obj.get("command").and_then(Value::as_str).map(str::to_owned);
    let args = string_array(obj.get("args"));
    let env = string_map(obj.get("env"));
    let headers = string_map(obj.get("headers"));
    let url = obj
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| obj.get("httpUrl").and_then(Value::as_str))
        .map(str::to_owned);
    let declared_type = obj.get("type").and_then(Value::as_str);
    let transport = if url.is_some() {
        if declared_type
            .map(|value| value.eq_ignore_ascii_case("sse"))
            .unwrap_or(false)
        {
            McpTransport::Sse
        } else {
            McpTransport::Http
        }
    } else {
        McpTransport::Stdio
    };
    let enabled = match spec.disable_key {
        Some(key) => !obj.get(key).and_then(Value::as_bool).unwrap_or(false),
        None => true,
    };

    let mut known: Vec<&str> = vec!["command", "args", "env", "headers", "url", "type"];
    if let Some(key) = spec.disable_key {
        known.push(key);
    }
    let mut extra = Map::new();
    for (key, value) in obj {
        if !known.contains(&key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }

    AdapterEntry {
        enabled,
        server: McpServer {
            name: name.to_owned(),
            transport,
            command,
            args,
            env,
            url,
            headers,
            extra,
        },
    }
}

fn from_opencode(name: &str, obj: &Map<String, Value>) -> AdapterEntry {
    let declared = obj
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("local");
    let enabled = obj.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let remote = declared.eq_ignore_ascii_case("remote");

    let (transport, command, args, env, url, headers) = if remote {
        (
            McpTransport::Http,
            None,
            Vec::new(),
            BTreeMap::new(),
            obj.get("url").and_then(Value::as_str).map(str::to_owned),
            string_map(obj.get("headers")),
        )
    } else {
        let mut command = None;
        let mut args = Vec::new();
        if let Some(items) = obj.get("command").and_then(Value::as_array) {
            let mut iter = items.iter().filter_map(Value::as_str);
            command = iter.next().map(str::to_owned);
            args = iter.map(str::to_owned).collect();
        }
        (
            McpTransport::Stdio,
            command,
            args,
            string_map(obj.get("environment")),
            None,
            BTreeMap::new(),
        )
    };

    let known = ["type", "enabled", "command", "environment", "url", "headers"];
    let mut extra = Map::new();
    for (key, value) in obj {
        if !known.contains(&key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }

    AdapterEntry {
        enabled,
        server: McpServer {
            name: name.to_owned(),
            transport,
            command,
            args,
            env,
            url,
            headers,
            extra,
        },
    }
}

fn parse_servers(spec: &JsonSpec, raw: &str) -> Result<Vec<AdapterEntry>, McpError> {
    if raw.trim().is_empty() {
        return Ok(Vec::new());
    }
    let root: Value = serde_json::from_str(raw)
        .map_err(|error| McpError::bad_request(format!("{} 配置解析失败：{error}", spec.label)))?;
    let Some(container) = root.get(spec.container).and_then(Value::as_object) else {
        return Ok(Vec::new());
    };
    let mut entries = Vec::new();
    for (name, value) in container {
        let Some(obj) = value.as_object() else {
            continue;
        };
        entries.push(match spec.shape {
            JsonShape::CommandUrl => from_command_url(spec, name, obj),
            JsonShape::OpenCode => from_opencode(name, obj),
        });
    }
    Ok(entries)
}

fn command_url_to_value(
    server: &McpServer,
    previous_disabled: bool,
    disable_key: Option<&str>,
) -> Value {
    let mut obj = server.extra.clone();
    for key in ["command", "args", "env", "headers", "url", "type"] {
        obj.remove(key);
    }
    if let Some(command) = &server.command {
        obj.insert("command".to_owned(), json!(command));
    }
    if !server.args.is_empty() {
        obj.insert("args".to_owned(), json!(server.args));
    }
    if !server.env.is_empty() {
        obj.insert("env".to_owned(), json!(server.env));
    }
    if !server.headers.is_empty() {
        obj.insert("headers".to_owned(), json!(server.headers));
    }
    if let Some(url) = &server.url {
        // Gemini 等使用 httpUrl 键，写入时沿用原有键名。
        if obj.contains_key("httpUrl") {
            obj.insert("httpUrl".to_owned(), json!(url));
        } else {
            obj.insert("url".to_owned(), json!(url));
        }
    } else {
        obj.remove("httpUrl");
    }
    if server.transport == McpTransport::Sse {
        obj.insert("type".to_owned(), json!("sse"));
    }
    if let Some(key) = disable_key {
        if previous_disabled {
            obj.insert(key.to_owned(), json!(true));
        } else {
            obj.remove(key);
        }
    }
    Value::Object(obj)
}

fn opencode_to_value(server: &McpServer, previous_enabled: bool) -> Value {
    let mut obj = server.extra.clone();
    for key in ["command", "environment", "url", "headers", "enabled", "type"] {
        obj.remove(key);
    }
    let remote = server.url.is_some() && matches!(server.transport, McpTransport::Http | McpTransport::Sse);
    if remote {
        obj.insert("type".to_owned(), json!("remote"));
        if let Some(url) = &server.url {
            obj.insert("url".to_owned(), json!(url));
        }
        if !server.headers.is_empty() {
            obj.insert("headers".to_owned(), json!(server.headers));
        }
    } else {
        obj.insert("type".to_owned(), json!("local"));
        let mut command: Vec<String> = Vec::new();
        if let Some(binary) = &server.command {
            command.push(binary.clone());
        }
        command.extend(server.args.iter().cloned());
        obj.insert("command".to_owned(), json!(command));
        if !server.env.is_empty() {
            obj.insert("environment".to_owned(), json!(server.env));
        }
    }
    obj.insert("enabled".to_owned(), json!(previous_enabled));
    Value::Object(obj)
}

fn apply_ops(spec: &JsonSpec, raw: &str, ops: &[ServerOp]) -> Result<String, McpError> {
    let mut root: Value = if raw.trim().is_empty() {
        Value::Object(Map::new())
    } else {
        serde_json::from_str(raw)
            .map_err(|error| McpError::bad_request(format!("{} 配置解析失败：{error}", spec.label)))?
    };
    if !root.is_object() {
        return Err(McpError::bad_request(format!(
            "{} 配置顶层不是对象",
            spec.label
        )));
    }

    let root_obj = root.as_object_mut().expect("已确认是对象");
    let container = root_obj
        .entry(spec.container.to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if !container.is_object() {
        *container = Value::Object(Map::new());
    }
    let map = container.as_object_mut().expect("已确认为对象");

    for op in ops {
        match op {
            ServerOp::Upsert { server } => {
                let previous = map.get(&server.name);
                let value = match spec.shape {
                    JsonShape::CommandUrl => {
                        let previous_disabled = spec
                            .disable_key
                            .and_then(|key| previous.and_then(|entry| entry.get(key)))
                            .and_then(Value::as_bool)
                            .unwrap_or(false);
                        command_url_to_value(server, previous_disabled, spec.disable_key)
                    }
                    JsonShape::OpenCode => {
                        let previous_enabled = previous
                            .and_then(|entry| entry.get("enabled"))
                            .and_then(Value::as_bool)
                            .unwrap_or(true);
                        opencode_to_value(server, previous_enabled)
                    }
                };
                map.insert(server.name.clone(), value);
            }
            ServerOp::Remove { name } => {
                map.remove(name);
            }
            ServerOp::SetEnabled { name, enabled } => {
                if let Some(entry) = map.get_mut(name).and_then(Value::as_object_mut) {
                    match spec.shape {
                        JsonShape::CommandUrl => {
                            if let Some(key) = spec.disable_key {
                                if *enabled {
                                    entry.remove(key);
                                } else {
                                    entry.insert(key.to_owned(), json!(true));
                                }
                            }
                        }
                        JsonShape::OpenCode => {
                            entry.insert("enabled".to_owned(), json!(enabled));
                        }
                    }
                }
            }
        }
    }

    let text = serde_json::to_string_pretty(&root).map_err(McpError::from)?;
    Ok(format!("{text}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SPEC: JsonSpec = JsonSpec {
        id: "test",
        label: "Test",
        container: "mcpServers",
        disable_key: Some("disabled"),
        global_rel: &[".test.json"],
        project_rel: None,
        shape: JsonShape::CommandUrl,
    };

    #[test]
    fn parses_and_round_trips_unknown_fields() {
        let raw = r#"{"mcpServers":{"demo":{"command":"npx","args":["-y","x"],"custom":123}}}"#;
        let entries = parse_servers(&SPEC, raw).expect("解析");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].server.command.as_deref(), Some("npx"));
        assert_eq!(entries[0].server.extra.get("custom"), Some(&json!(123)));
        let out = apply_ops(&SPEC, raw, &[ServerOp::Upsert {
            server: entries[0].server.clone(),
        }])
        .expect("写回");
        let reparsed = parse_servers(&SPEC, &out).expect("重解析");
        assert_eq!(reparsed[0].server.extra.get("custom"), Some(&json!(123)));
    }

    #[test]
    fn toggles_disable_flag() {
        let raw = r#"{"mcpServers":{"demo":{"command":"uvx","args":["m"]}}}"#;
        let out = apply_ops(
            &SPEC,
            raw,
            &[ServerOp::SetEnabled {
                name: "demo".to_owned(),
                enabled: false,
            }],
        )
        .expect("停用");
        let entries = parse_servers(&SPEC, &out).expect("解析");
        assert!(!entries[0].enabled);
    }
}

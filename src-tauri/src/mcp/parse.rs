use serde_json::{Map, Value};

use super::error::McpError;
use super::types::{McpServer, McpTransport, ParsedServer};

/// 极简 shell 词法拆分：支持单 / 双引号，反斜杠转义。
pub fn shell_split(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut started = false;
    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match character {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' if quote == Some(character) => quote = None,
            '\'' | '"' if quote.is_none() => {
                quote = Some(character);
                started = true;
            }
            value if value.is_whitespace() && quote.is_none() => {
                if started {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            value => {
                current.push(value);
                started = true;
            }
        }
    }
    if started {
        tokens.push(current);
    }
    tokens
}

fn string_map(value: Option<&Value>) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
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

fn string_args(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        Some(Value::String(single)) => vec![single.clone()],
        _ => Vec::new(),
    }
}

/// 从任意 JSON 对象构造服务（兼容常见字段命名）。
pub fn server_from_object(name: &str, obj: &Map<String, Value>) -> McpServer {
    let command = obj
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| {
            // OpenCode 风格：command 为数组，首元素是命令。
            obj.get("command")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let args = {
        let direct = string_args(obj.get("args"));
        if direct.is_empty() {
            obj.get("command")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .skip(1)
                        .filter_map(Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        } else {
            direct
        }
    };
    let env = {
        let primary = string_map(obj.get("env"));
        if primary.is_empty() {
            string_map(obj.get("environment"))
        } else {
            primary
        }
    };
    let headers = string_map(obj.get("headers"));
    let url = obj
        .get("url")
        .and_then(Value::as_str)
        .or_else(|| obj.get("httpUrl").and_then(Value::as_str))
        .map(str::to_owned);
    let declared = obj.get("type").and_then(Value::as_str).unwrap_or("");
    let transport = if url.is_some() {
        if declared.eq_ignore_ascii_case("sse") {
            McpTransport::Sse
        } else {
            McpTransport::Http
        }
    } else {
        McpTransport::Stdio
    };

    let known = [
        "command",
        "args",
        "env",
        "environment",
        "headers",
        "url",
        "httpUrl",
        "type",
        "enabled",
        "disabled",
    ];
    let mut extra = Map::new();
    for (key, value) in obj {
        if !known.contains(&key.as_str()) {
            extra.insert(key.clone(), value.clone());
        }
    }

    McpServer {
        name: name.to_owned(),
        transport,
        command,
        args,
        env,
        url,
        headers,
        extra,
    }
}

fn servers_from_value(value: &Value) -> Vec<McpServer> {
    let Some(root) = value.as_object() else {
        return Vec::new();
    };
    if let Some(container) = root.get("mcpServers").and_then(Value::as_object) {
        return container
            .iter()
            .filter_map(|(name, item)| item.as_object().map(|obj| server_from_object(name, obj)))
            .collect();
    }
    if let Some(container) = root.get("mcp").and_then(Value::as_object) {
        return container
            .iter()
            .filter_map(|(name, item)| item.as_object().map(|obj| server_from_object(name, obj)))
            .collect();
    }
    // 单个服务定义。
    if root.contains_key("command") || root.contains_key("url") || root.contains_key("httpUrl") {
        let name = root
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("pasted-server");
        return vec![server_from_object(name, root)];
    }
    // 名称 → 定义 的映射。
    let looks_like_map = !root.is_empty()
        && root.values().all(|item| {
            item.as_object()
                .map(|obj| {
                    obj.contains_key("command")
                        || obj.contains_key("url")
                        || obj.contains_key("httpUrl")
                        || obj.contains_key("type")
                })
                .unwrap_or(false)
        });
    if looks_like_map {
        return root
            .iter()
            .filter_map(|(name, item)| item.as_object().map(|obj| server_from_object(name, obj)))
            .collect();
    }
    Vec::new()
}

fn json_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let trimmed = text.trim();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        blocks.push(trimmed.to_owned());
    }
    let mut in_fence = false;
    let mut buffer = String::new();
    for line in text.lines() {
        let fence = line.trim_start().starts_with("```");
        if fence {
            if in_fence {
                blocks.push(std::mem::take(&mut buffer));
            }
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            buffer.push_str(line);
            buffer.push('\n');
        }
    }
    blocks
}

/// 从命令行文本识别 stdio / 远程服务。
fn server_from_command_line(line: &str) -> Option<McpServer> {
    let trimmed = line.trim().trim_start_matches(['$', '>', '#', ' ']).trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens = shell_split(trimmed);
    if tokens.is_empty() {
        return None;
    }

    // claude mcp add <name> [-e K=V] [--transport http] -- <command...>
    if let Some(add_index) = tokens.iter().position(|token| token == "add") {
        if tokens.iter().any(|token| token.ends_with("mcp")) || tokens.first().map(|t| t == "claude").unwrap_or(false) {
            let name = tokens.get(add_index + 1)?.clone();
            let mut env = std::collections::BTreeMap::new();
            let mut transport = McpTransport::Stdio;
            let mut index = add_index + 2;
            while index < tokens.len() {
                match tokens[index].as_str() {
                    "-e" | "--env" => {
                        if let Some(pair) = tokens.get(index + 1) {
                            if let Some((key, value)) = pair.split_once('=') {
                                env.insert(key.to_owned(), value.to_owned());
                            }
                        }
                        index += 2;
                    }
                    "-t" | "--transport" => {
                        if let Some(value) = tokens.get(index + 1) {
                            transport = McpTransport::parse(value);
                        }
                        index += 2;
                    }
                    "--" => {
                        index += 1;
                        break;
                    }
                    _ => index += 1,
                }
            }
            let rest = &tokens[index.min(tokens.len())..];
            if rest.is_empty() {
                return None;
            }
            if rest[0].starts_with("http://") || rest[0].starts_with("https://") {
                return Some(McpServer {
                    name,
                    transport: if transport == McpTransport::Stdio {
                        McpTransport::Http
                    } else {
                        transport
                    },
                    command: None,
                    args: Vec::new(),
                    env,
                    url: Some(rest[0].clone()),
                    headers: Default::default(),
                    extra: Map::new(),
                });
            }
            return Some(McpServer {
                name,
                transport,
                command: Some(rest[0].clone()),
                args: rest[1..].to_vec(),
                env,
                url: None,
                headers: Default::default(),
                extra: Map::new(),
            });
        }
    }

    // npx / uvx / docker run 命令。
    let runners = ["npx", "uvx", "bunx", "pnpm", "npm", "docker"];
    if !runners.contains(&tokens[0].as_str()) {
        return None;
    }
    let mut args = tokens[1..].to_vec();
    let name = if tokens[0] == "docker" {
        // docker run -i --rm <image> ...
        let image = args
            .iter()
            .skip_while(|token| token.starts_with('-') || token == &"run")
            .find(|token| !token.starts_with('-'))
            .cloned()
            .unwrap_or_else(|| "docker-server".to_owned());
        image
            .rsplit('/')
            .next()
            .unwrap_or("docker-server")
            .to_owned()
    } else {
        args.iter()
            .skip_while(|token| token.starts_with('-') && *token != "-y")
            .find(|token| !token.starts_with('-'))
            .map(|package| {
                package
                    .rsplit('/')
                    .next()
                    .unwrap_or(package)
                    .split('@')
                    .next()
                    .unwrap_or(package)
                    .to_owned()
            })
            .unwrap_or_else(|| "mcp-server".to_owned())
    };
    // 去掉仅用于执行的 -y 标志，保留其余参数。
    args.retain(|token| token != "-y" || tokens[0] != "npx");
    Some(McpServer {
        name,
        transport: McpTransport::Stdio,
        command: Some(tokens[0].clone()),
        args,
        env: Default::default(),
        url: None,
        headers: Default::default(),
        extra: Map::new(),
    })
}

/// 解析粘贴内容：优先 JSON，其次命令行。永不执行任何东西。
pub fn parse_text(text: &str) -> Vec<ParsedServer> {
    let mut out: Vec<ParsedServer> = Vec::new();
    for block in json_blocks(text) {
        if let Ok(value) = serde_json::from_str::<Value>(&block) {
            for server in servers_from_value(&value) {
                out.push(ParsedServer {
                    server,
                    source: "json".to_owned(),
                });
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    for line in text.lines() {
        if let Some(server) = server_from_command_line(line) {
            out.push(ParsedServer {
                server,
                source: "command".to_owned(),
            });
        }
    }
    out
}

/// 命令行的安全校验：必须非空。
pub fn validate_server(server: &McpServer) -> Result<(), McpError> {
    if server.name.trim().is_empty() {
        return Err(McpError::bad_request("服务名不能为空"));
    }
    if server.transport == McpTransport::Stdio {
        match &server.command {
            Some(command) if !command.trim().is_empty() => {}
            _ => return Err(McpError::bad_request("stdio 服务必须填写 command")),
        }
    } else if server.url.as_deref().unwrap_or("").trim().is_empty() {
        return Err(McpError::bad_request("远程服务必须填写 url"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mcp_servers_json() {
        let parsed = parse_text(r#"{"mcpServers":{"demo":{"command":"npx","args":["-y","x"]}}}"#);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].server.name, "demo");
        assert_eq!(parsed[0].server.command.as_deref(), Some("npx"));
    }

    #[test]
    fn parses_claude_add_command() {
        let parsed = parse_text("claude mcp add github -e TOKEN=abc -- npx -y @modelcontextprotocol/server-github");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].server.name, "github");
        assert_eq!(parsed[0].server.env.get("TOKEN").map(String::as_str), Some("abc"));
    }

    #[test]
    fn parses_uvx_line() {
        let parsed = parse_text("uvx mcp-server-time");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].server.command.as_deref(), Some("uvx"));
    }
}

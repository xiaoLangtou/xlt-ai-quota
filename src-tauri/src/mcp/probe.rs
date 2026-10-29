use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::error::McpError;
use super::types::{McpServer, McpTransport, ProbeResult, ProbeTool};

const TIMEOUT: Duration = Duration::from_secs(10);
const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_STDIO_WORKERS: usize = 2;
static ACTIVE_STDIO_WORKERS: AtomicUsize = AtomicUsize::new(0);

struct ProbePermit;

impl ProbePermit {
    fn acquire() -> Result<Self, McpError> {
        ACTIVE_STDIO_WORKERS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < MAX_STDIO_WORKERS).then_some(count + 1)
            })
            .map_err(|_| McpError::upstream("连接测试已达到并发上限"))?;
        Ok(Self)
    }
}

impl Drop for ProbePermit {
    fn drop(&mut self) {
        ACTIVE_STDIO_WORKERS.fetch_sub(1, Ordering::AcqRel);
    }
}

fn initialize_request() -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "xlt-workbench", "version": "0.1.3" }
        }
    })
}

fn tools_list_request() -> Value {
    json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {} })
}

fn parse_tools(response: &Value) -> Vec<ProbeTool> {
    response
        .pointer("/result/tools")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let name = item.get("name").and_then(Value::as_str)?.to_owned();
                    let description = item
                        .get("description")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    Some(ProbeTool { name, description })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_server_info(response: &Value) -> Option<String> {
    response
        .pointer("/result/serverInfo/name")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn read_response(reader: &mut impl BufRead, id: i64) -> Result<Value, McpError> {
    loop {
        let mut line = String::new();
        let read = reader.take(MAX_RESPONSE_BYTES + 1).read_line(&mut line)?;
        if read as u64 > MAX_RESPONSE_BYTES || (read > 0 && !line.ends_with('\n')) {
            return Err(McpError::upstream("服务响应超过 1 MB"));
        }
        if read == 0 {
            return Err(McpError::upstream("进程提前退出，未返回响应"));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if value.get("id").and_then(Value::as_i64) == Some(id) {
            return Ok(value);
        }
    }
}

fn stdio_handshake(
    stdin: &mut ChildStdin,
    stdout: ChildStdout,
) -> Result<(Vec<ProbeTool>, Option<String>), McpError> {
    writeln!(stdin, "{}", initialize_request())?;
    stdin.flush()?;
    let mut reader = BufReader::new(stdout);
    let init = read_response(&mut reader, 1)?;
    if let Some(error) = init.get("error") {
        return Err(McpError::upstream(format!("initialize 失败：{}", error)));
    }
    let server_info = parse_server_info(&init);

    writeln!(
        stdin,
        "{}",
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })
    )?;
    writeln!(stdin, "{}", tools_list_request())?;
    stdin.flush()?;
    let tools_response = read_response(&mut reader, 2)?;
    Ok((parse_tools(&tools_response), server_info))
}

fn probe_stdio(server: &McpServer) -> Result<(Vec<ProbeTool>, Option<String>), McpError> {
    let permit = ProbePermit::acquire()?;
    let command = server
        .command
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| McpError::bad_request("stdio 服务缺少 command"))?;
    let mut process = Command::new(command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        process.process_group(0);
    }
    process.args(&server.args);
    for (key, value) in &server.env {
        process.env(key, value);
    }
    let child = process
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| McpError::bad_request(format!("启动进程失败：{error}")))?;
    let child = Arc::new(Mutex::new(child));
    let (mut stdin, stdout) = {
        let mut guard = child
            .lock()
            .map_err(|_| McpError::internal("进程锁定失败"))?;
        let stdin = guard
            .stdin
            .take()
            .ok_or_else(|| McpError::internal("无法写入子进程"))?;
        let stdout = guard
            .stdout
            .take()
            .ok_or_else(|| McpError::internal("无法读取子进程"))?;
        (stdin, stdout)
    };

    let (sender, receiver) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let _permit = permit;
        let _ = sender.send(stdio_handshake(&mut stdin, stdout));
    });

    let result = match receiver.recv_timeout(TIMEOUT) {
        Ok(result) => result,
        Err(_) => Err(McpError::upstream("测试连接超时（10 秒）")),
    };
    #[cfg(unix)]
    if let Ok(guard) = child.lock() {
        unsafe { libc::killpg(guard.id() as i32, libc::SIGKILL) };
    }
    if let Ok(mut guard) = child.lock() {
        let _ = guard.kill();
        let _ = guard.wait();
    }
    if worker.is_finished() || receiver.recv_timeout(Duration::from_millis(200)).is_ok() {
        let _ = worker.join();
    }
    result
}

/// 从可能的 SSE 流中提取第一个 JSON 对象。
fn extract_json(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(text.trim()) {
        return Some(value);
    }
    for line in text.lines() {
        let line = line.trim();
        let payload = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
        if payload.starts_with('{') {
            if let Ok(value) = serde_json::from_str::<Value>(payload) {
                if value.get("id").is_some() {
                    return Some(value);
                }
            }
        }
    }
    None
}

fn read_http_body(response: reqwest::blocking::Response) -> Result<String, McpError> {
    let mut bytes = Vec::new();
    response
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| McpError::upstream(format!("读取响应失败：{error}")))?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(McpError::upstream("服务响应超过 1 MB"));
    }
    String::from_utf8(bytes).map_err(|error| McpError::upstream(format!("响应不是 UTF-8：{error}")))
}

fn probe_http(server: &McpServer) -> Result<(Vec<ProbeTool>, Option<String>), McpError> {
    let url = server
        .url
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| McpError::bad_request("远程服务缺少 url"))?;
    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|error| McpError::internal(error.to_string()))?;

    let mut init_request = client
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json");
    for (key, value) in &server.headers {
        init_request = init_request.header(key, value);
    }
    let response = init_request
        .json(&initialize_request())
        .send()
        .map_err(|error| McpError::upstream(format!("连接失败：{error}")))?;
    let status = response.status();
    let session = response
        .headers()
        .get("mcp-session-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let body = read_http_body(response)?;
    if !status.is_success() {
        return Err(McpError::upstream(format!(
            "HTTP {}：{}",
            status.as_u16(),
            body.chars().take(200).collect::<String>()
        )));
    }
    let init = extract_json(&body).ok_or_else(|| McpError::upstream("响应不是合法 JSON-RPC"))?;
    if let Some(error) = init.get("error") {
        return Err(McpError::upstream(format!("initialize 失败：{error}")));
    }
    let server_info = parse_server_info(&init);

    let mut tools_request = client
        .post(url)
        .header("Accept", "application/json, text/event-stream")
        .header("Content-Type", "application/json");
    if let Some(session) = &session {
        tools_request = tools_request.header("mcp-session-id", session);
    }
    for (key, value) in &server.headers {
        tools_request = tools_request.header(key, value);
    }
    let tools_response = tools_request
        .json(&tools_list_request())
        .send()
        .map_err(|error| McpError::upstream(format!("tools/list 失败：{error}")))?;
    let body = read_http_body(tools_response)?;
    let tools_value =
        extract_json(&body).ok_or_else(|| McpError::upstream("tools/list 响应非法"))?;
    Ok((parse_tools(&tools_value), server_info))
}

/// 运行层检测：启动进程或连接 URL，发送 initialize + tools/list，10 秒超时。
pub fn probe(server: &McpServer) -> ProbeResult {
    let started = Instant::now();
    let transport = server.transport.as_str().to_owned();
    let outcome = if server.transport == McpTransport::Stdio {
        probe_stdio(server)
    } else {
        probe_http(server)
    };
    let duration_ms = started.elapsed().as_millis() as u64;
    match outcome {
        Ok((tools, server_info)) => ProbeResult {
            ok: true,
            transport,
            tools,
            server_info,
            error: None,
            duration_ms,
        },
        Err(error) => ProbeResult {
            ok: false,
            transport,
            tools: Vec::new(),
            server_info: None,
            error: Some(error.message),
            duration_ms,
        },
    }
}

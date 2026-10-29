use regex::Regex;
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};

/// 共享 AI Provider 调用层：日报（Git 报告）与文档拆解各自维护 Prompt 与响应校验，
/// 但供应商地址、认证头、请求发送、响应读取和 `<think>` 清理集中在这里，避免重复实现。
pub struct ChatRequest<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub api_key: &'a str,
    pub system: &'a str,
    pub user: &'a str,
    pub temperature: f64,
    pub max_tokens: u32,
    /// 要求模型只输出 JSON 对象（仅对支持 response_format 的服务商开启）。
    pub json_mode: bool,
}

/// 返回 (host, path)；未知 provider 直接报错。
fn endpoint(provider: &str) -> Result<(&'static str, &'static str), String> {
    match provider {
        "minimax" => Ok(("api.minimax.chat", "/v1/text/chatcompletion_v2")),
        "kimi" => Ok(("api.moonshot.cn", "/v1/chat/completions")),
        "deepseek" => Ok(("api.deepseek.com", "/chat/completions")),
        _ => Err("不支持的 AI Provider".to_owned()),
    }
}

/// 校验 provider 是否受支持（不发起网络请求）。
pub fn validate_provider(provider: &str) -> Result<(), String> {
    endpoint(provider).map(|_| ())
}

/// 调用 chat completions 接口，返回已清理思考内容后的正文。
pub fn chat(request: ChatRequest<'_>) -> Result<String, String> {
    chat_with_images(request, &[])
}

pub fn chat_with_images(request: ChatRequest<'_>, images: &[String]) -> Result<String, String> {
    let api_key = request.api_key.trim();
    if api_key.is_empty() {
        return Err("请输入 AI Provider 的 API Key".to_owned());
    }
    if request.model.trim().is_empty() {
        return Err("请选择 AI 模型".to_owned());
    }
    let (host, path) = endpoint(request.provider)?;
    let mut body = serde_json::json!({
        "model": request.model,
        "messages": [
            { "role": "system", "content": request.system },
            { "role": "user", "content": request.user },
        ],
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
    });
    if !images.is_empty() {
        let mut parts = vec![serde_json::json!({"type":"text","text":request.user})];
        for image in images {
            parts.push(serde_json::json!({"type":"image_url","image_url":{"url":image}}));
        }
        body["messages"][1]["content"] = Value::Array(parts);
    }
    if request.json_mode {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }
    let output = Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--max-time")
        .arg("120")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("--data-binary")
        .arg("@-")
        .arg(format!("https://{host}{path}"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法调用 AI Provider：{error}"))?;
    let mut child = output;
    let write_result = child.stdin.take().ok_or("无法写入 AI 请求")?.write_all(body.to_string().as_bytes());
    if let Err(error) = write_result {
        let _ = child.kill(); let _ = child.wait();
        return Err(format!("AI 请求发送失败：{error}"));
    }
    let output = child.wait_with_output().map_err(|error| format!("无法读取 AI 响应：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "AI Provider 请求失败：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let response: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("AI Provider 返回的不是 JSON：{error}"))?;
    let content = response
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            let message = response
                .pointer("/error/message")
                .and_then(Value::as_str)
                .or_else(|| response.get("message").and_then(Value::as_str))
                .unwrap_or("AI Provider 未返回内容");
            format!("AI Provider 返回错误：{message}")
        })?;
    strip_thinking(content)
}

/// 流式增量类型：模型思考内容或正式回答内容。
#[derive(Clone, Copy)]
pub enum ChatChunk {
    Reasoning,
    Content,
}

/// 以 SSE 流式方式调用 chat completions，边接收边回调增量，返回累积后的完整正文。
/// 支持 DeepSeek 等模型的 `reasoning_content` 思考流，便于前端展示调用过程。
pub fn chat_stream(
    request: ChatRequest<'_>,
    mut on_delta: impl FnMut(ChatChunk, &str),
) -> Result<String, String> {
    let api_key = request.api_key.trim();
    if api_key.is_empty() {
        return Err("请输入 AI Provider 的 API Key".to_owned());
    }
    if request.model.trim().is_empty() {
        return Err("请选择 AI 模型".to_owned());
    }
    let (host, path) = endpoint(request.provider)?;
    let mut body = serde_json::json!({
        "model": request.model,
        "messages": [
            { "role": "system", "content": request.system },
            { "role": "user", "content": request.user },
        ],
        "temperature": request.temperature,
        "max_tokens": request.max_tokens,
        "stream": true,
    });
    if request.json_mode {
        body["response_format"] = serde_json::json!({ "type": "json_object" });
    }
    let mut child = Command::new("curl")
        .arg("-sS")
        .arg("-N")
        .arg("--no-buffer")
        .arg("--connect-timeout")
        .arg("15")
        .arg("--max-time")
        .arg("120")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg("Accept: text/event-stream")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("--data-raw")
        .arg(body.to_string())
        .arg(format!("https://{host}{path}"))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("无法调用 AI Provider：{error}"))?;

    // stderr 单独读取，避免管道写满阻塞；stdout 逐行解析 SSE。
    let mut stderr = child.stderr.take();
    let stderr_reader = stderr.take().map(|mut pipe| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    });

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "无法读取 AI Provider 响应".to_owned())?;
    let reader = BufReader::new(stdout);
    let mut content = String::new();
    let mut raw = String::new();
    for line in reader.lines() {
        let line = line.map_err(|error| format!("读取 AI Provider 响应失败：{error}"))?;
        raw.push_str(&line);
        raw.push('\n');
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data.is_empty() || data == "[DONE]" {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if let Some(message) = value
            .pointer("/error/message")
            .and_then(Value::as_str)
            .or_else(|| value.get("message").and_then(Value::as_str))
        {
            if value.get("choices").is_none() {
                return Err(format!("AI Provider 返回错误：{message}"));
            }
        }
        if let Some(reasoning) = value
            .pointer("/choices/0/delta/reasoning_content")
            .and_then(Value::as_str)
        {
            if !reasoning.is_empty() {
                on_delta(ChatChunk::Reasoning, reasoning);
            }
        }
        if let Some(delta) = value
            .pointer("/choices/0/delta/content")
            .and_then(Value::as_str)
        {
            if !delta.is_empty() {
                content.push_str(delta);
                on_delta(ChatChunk::Content, delta);
            }
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("AI Provider 请求失败：{error}"))?;
    if !status.success() {
        let stderr = stderr_reader
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();
        return Err(format!("AI Provider 请求失败：{}", stderr.trim()));
    }

    if content.trim().is_empty() {
        // 非流式兜底或错误响应：尝试从原始输出中取错误信息。
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            let message = value
                .pointer("/error/message")
                .and_then(Value::as_str)
                .or_else(|| value.get("message").and_then(Value::as_str));
            if let Some(message) = message {
                return Err(format!("AI Provider 返回错误：{message}"));
            }
        }
        return Err("AI Provider 未返回内容".to_owned());
    }

    strip_thinking(&content)
}

/// 去掉模型可能输出的 `<think>...</think>` 推理块并 trim。
pub fn strip_thinking(content: &str) -> Result<String, String> {
    let thinking = Regex::new(r"(?s)<think>.*?</think>")
        .map_err(|error| format!("无法清理模型思考内容：{error}"))?;
    Ok(thinking.replace_all(content, "").trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::{strip_thinking, validate_provider};

    #[test]
    fn strips_thinking_block_and_trims() {
        let text = "  <think>内部推理</think>\n最终答案  ";
        assert_eq!(strip_thinking(text).unwrap(), "最终答案");
    }

    #[test]
    fn accepts_known_providers_and_rejects_unknown() {
        assert!(validate_provider("minimax").is_ok());
        assert!(validate_provider("kimi").is_ok());
        assert!(validate_provider("deepseek").is_ok());
        assert!(validate_provider("openai").is_err());
    }
}

//! DeepSeek（OpenAI 兼容 chat completions）翻译。
//!
//! 与有道的差别：一次请求翻一整段，提示词可自定义，术语与格式约束靠 system prompt。

use serde_json::{json, Value};

use super::{ChunkError, DeepSeekConfig};

pub(super) async fn translate_chunk(
    client: &reqwest::Client,
    config: &DeepSeekConfig,
    text: &str,
    from: &str,
    to: &str,
) -> Result<(String, String), ChunkError> {
    let url = chat_completions_url(&config.base_url);
    let mut body = json!({
        "model": config.model,
        "messages": [
            { "role": "system", "content": render_prompt(&config.system_prompt, from, to, text) },
            { "role": "user", "content": render_prompt(&config.user_prompt, from, to, text) },
        ],
        "temperature": config.temperature,
        "stream": false,
    });
    // 关闭思考模式时省略字段，避免个别兼容服务因未知参数报错。
    if config.thinking == "enabled" {
        body["thinking"] = json!({ "type": "enabled" });
    }

    let response = client
        .post(&url)
        .bearer_auth(&config.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|error| ChunkError::Fatal(format!("网络请求失败: {error}")))?;
    let status = response.status();
    let raw = response
        .text()
        .await
        .map_err(|error| ChunkError::Fatal(format!("读取响应失败: {error}")))?;
    if !status.is_success() {
        let message = format!("DeepSeek 返回 HTTP {status}{}", error_detail(&raw));
        return Err(if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            ChunkError::RateLimited(message)
        } else {
            ChunkError::Fatal(message)
        });
    }
    let translated = parse_response(&raw)?;
    Ok((translated, format!("{from}2{to}")))
}

/// 基础 URL 允许填到域名、`/v1` 或完整 `/chat/completions`。
fn chat_completions_url(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.ends_with("/chat/completions") {
        trimmed.to_owned()
    } else {
        format!("{trimmed}/chat/completions")
    }
}

fn render_prompt(template: &str, from: &str, to: &str, text: &str) -> String {
    let detected = if from == "auto" { "" } else { from };
    template
        .replace("{source_language}", language_name(from))
        .replace("{detected_source_language}", language_name(detected))
        .replace("{target_language}", language_name(to))
        .replace("{text}", text)
}

fn language_name(code: &str) -> &str {
    match code {
        "" => "",
        "auto" => "the detected source language",
        "zh-CHS" => "Simplified Chinese",
        "zh-CHT" => "Traditional Chinese",
        "en" => "English",
        "ja" => "Japanese",
        "ko" => "Korean",
        "fr" => "French",
        "de" => "German",
        "es" => "Spanish",
        "ru" => "Russian",
        other => other,
    }
}

fn parse_response(body: &str) -> Result<String, ChunkError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| ChunkError::Fatal("DeepSeek 返回了无法解析的内容".to_owned()))?;
    let content = value
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| ChunkError::Fatal("DeepSeek 未返回译文".to_owned()))?;
    Ok(content.to_owned())
}

fn error_detail(body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .map(|message| format!("：{message}"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{chat_completions_url, error_detail, parse_response, render_prompt};

    #[test]
    fn builds_chat_completions_url() {
        assert_eq!(
            chat_completions_url("https://api.deepseek.com"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/"),
            "https://api.deepseek.com/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/v1"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/chat/completions"),
            "https://api.deepseek.com/chat/completions"
        );
    }

    #[test]
    fn renders_prompt_variables() {
        let rendered = render_prompt(
            "translate to {target_language} from {source_language}: {text}",
            "en",
            "zh-CHS",
            "hello",
        );
        assert_eq!(rendered, "translate to Simplified Chinese from English: hello");
    }

    #[test]
    fn auto_source_renders_detected_variable_empty() {
        let rendered = render_prompt("{detected_source_language}|{text}", "auto", "en", "hi");
        assert_eq!(rendered, "|hi");
    }

    #[test]
    fn parses_message_content() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":"  你好  "}}]}"#;
        assert_eq!(parse_response(body).expect("应当解析成功"), "你好");
    }

    #[test]
    fn reports_missing_content() {
        let body = r#"{"choices":[]}"#;
        assert!(parse_response(body).is_err());
    }

    #[test]
    fn extracts_error_message() {
        let body = r#"{"error":{"message":"invalid api key"}}"#;
        assert_eq!(error_detail(body), "：invalid api key");
        assert_eq!(error_detail("not json"), "");
    }
}

//! 有道智云文本翻译 API v3。
//!
//! 签名与 q 截断规则见官方文档；一次请求只翻一段，不返回句级偏移。

use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{unix_timestamp, ChunkError, YoudaoConfig};

pub(super) async fn translate_chunk(
    client: &reqwest::Client,
    config: &YoudaoConfig,
    text: &str,
    from: &str,
    to: &str,
) -> Result<(String, String), ChunkError> {
    let salt = uuid::Uuid::new_v4().to_string();
    let curtime = unix_timestamp().map_err(ChunkError::Fatal)?;
    let sign = sign(&config.app_key, text, &salt, &curtime, &config.app_secret);

    let mut form: Vec<(&str, String)> = vec![
        ("q", text.to_owned()),
        ("from", from.to_owned()),
        ("to", to.to_owned()),
        ("appKey", config.app_key.clone()),
        ("salt", salt),
        ("sign", sign),
        ("signType", "v3".to_owned()),
        ("curtime", curtime),
    ];
    if config.strict {
        form.push(("strict", "true".to_owned()));
    }
    if !config.vocab_id.is_empty() {
        form.push(("vocabId", config.vocab_id.clone()));
    }
    if config.domain != "general" {
        form.push(("domain", config.domain.clone()));
    }
    if config.reject_fallback {
        form.push(("rejectFallback", "true".to_owned()));
    }

    let response = client
        .post(&config.base_url)
        .form(&form)
        .send()
        .await
        .map_err(|error| ChunkError::Fatal(format!("网络请求失败: {error}")))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| ChunkError::Fatal(format!("读取响应失败: {error}")))?;
    if !status.is_success() {
        let message = format!("有道接口返回 HTTP {status}");
        return Err(if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            ChunkError::RateLimited(message)
        } else {
            ChunkError::Fatal(message)
        });
    }
    parse_response(&body)
}

/// 签名原文：q 不超过 20 字符时取自身，否则取「前 10 + 长度 + 后 10」。
fn sign_input(q: &str) -> String {
    let chars: Vec<char> = q.chars().collect();
    if chars.len() <= 20 {
        return q.to_owned();
    }
    let head: String = chars[..10].iter().collect();
    let tail: String = chars[chars.len() - 10..].iter().collect();
    format!("{head}{}{tail}", chars.len())
}

fn sign(app_key: &str, q: &str, salt: &str, curtime: &str, app_secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{app_key}{}{salt}{curtime}{app_secret}", sign_input(q)).as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn parse_response(body: &str) -> Result<(String, String), ChunkError> {
    let value: Value = serde_json::from_str(body)
        .map_err(|_| ChunkError::Fatal("有道接口返回了无法解析的内容".to_owned()))?;
    let code = match value.get("errorCode") {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => return Err(ChunkError::Fatal("有道接口未返回错误码".to_owned())),
    };
    if code != "0" {
        let message = error_message(&code);
        return Err(match code.as_str() {
            // 411 访问频率受限 / 412 长请求过于频繁：退避后可以重试
            "411" | "412" => ChunkError::RateLimited(message),
            _ => ChunkError::Fatal(message),
        });
    }
    let translated = value
        .get("translation")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .filter(|text| !text.trim().is_empty())
        .ok_or_else(|| ChunkError::Fatal("有道接口未返回译文".to_owned()))?;
    let lang = value
        .get("l")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok((translated, lang))
}

fn error_message(code: &str) -> String {
    let hint = match code {
        "101" => "缺少必填参数",
        "102" => "不支持的语言类型",
        "103" => "文本过长，请分段后再翻译",
        "104" => "不支持的 API 类型",
        "105" => "不支持的签名类型",
        "106" => "不支持的响应类型",
        "107" => "不支持的传输加密类型",
        "108" => "应用 ID 无效，请在控制台确认应用 ID",
        "110" => "应用未绑定文本翻译服务",
        "111" => "开发者账号无效",
        "112" => "请求服务无效",
        "113" => "待翻译文本不能为空",
        "116" => "严格翻译取值无效",
        "202" => "签名校验失败，请检查应用 ID 与应用密钥",
        "203" => "访问 IP 不在白名单",
        "205" => "接口与应用平台类型不一致，请把应用创建为「API」类型",
        "206" => "时间戳无效，请检查本机时间",
        "207" => "请求重复（salt 已被使用）",
        "301" => "词典查询失败",
        "302" => "翻译查询失败",
        "303" => "服务端异常",
        "304" => "翻译失败，请稍后重试",
        "308" => "拒绝回退参数无效",
        "309" => "领域参数无效",
        "310" => "未开通领域翻译服务",
        "401" => "账户已欠费，请充值后再用",
        "411" => "访问频率受限，请稍后重试",
        "412" => "长请求过于频繁，请稍后重试",
        _ => "有道接口返回未知错误",
    };
    format!("{hint}（错误码 {code}）")
}

#[cfg(test)]
mod tests {
    use super::{error_message, parse_response, sign, sign_input};
    use crate::translate::ChunkError;

    #[test]
    fn sign_input_keeps_short_text() {
        assert_eq!(sign_input("representation"), "representation");
        assert_eq!(sign_input("12345678901234567890"), "12345678901234567890");
    }

    #[test]
    fn sign_input_truncates_long_text() {
        // 30 个字符：前 10 + 长度 + 后 10。
        let text = "1234567890abcdefghijABCDEFGHIJ";
        assert_eq!(sign_input(text), "123456789030ABCDEFGHIJ");
    }

    #[test]
    fn sign_is_sha256_hex() {
        let value = sign("app", "hello", "salt", "1700000000", "secret");
        assert_eq!(value.len(), 64);
        assert!(value.chars().all(|item| item.is_ascii_hexdigit()));
        assert_eq!(
            value,
            sign("app", "hello", "salt", "1700000000", "secret"),
            "相同输入必须得到相同签名"
        );
    }

    #[test]
    fn parses_success_response() {
        let body = r#"{"errorCode":"0","translation":["你好"],"l":"EN2zh-CHS"}"#;
        let (translated, lang) = parse_response(body).expect("应当解析成功");
        assert_eq!(translated, "你好");
        assert_eq!(lang, "EN2zh-CHS");
    }

    #[test]
    fn maps_error_code_to_hint() {
        let error = parse_response(r#"{"errorCode":"202"}"#).expect_err("签名错误必须报错");
        assert!(error.message().contains("签名校验失败"));
        assert!(error.message().contains("202"));
        assert!(matches!(error, ChunkError::Fatal(_)));
        assert!(error_message("411").contains("频率"));
    }

    #[test]
    fn treats_rate_limit_as_retryable() {
        let error = parse_response(r#"{"errorCode":"411"}"#).expect_err("限流必须报错");
        assert!(matches!(error, ChunkError::RateLimited(_)), "411 应当可重试");
        let error = parse_response(r#"{"errorCode":"412"}"#).expect_err("长请求限流必须报错");
        assert!(matches!(error, ChunkError::RateLimited(_)), "412 应当可重试");
    }
}

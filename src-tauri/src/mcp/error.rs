use serde::Serialize;

/// MCP 领域错误。实现 `Serialize` 以便作为 Tauri command 的错误类型，
/// 前端可读取 `code` / `message` / `status`（与 SkillsError 语义一致）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct McpError {
    pub code: String,
    pub message: String,
    pub status: u16,
}

impl McpError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        Self {
            code: "request_failed".to_owned(),
            message: message.into(),
            status,
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(403, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(404, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(409, message)
    }

    /// 数据源被拦截 / 上游不可用。
    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(502, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal_error".to_owned(),
            message: message.into(),
            status: 500,
        }
    }
}

impl std::fmt::Display for McpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for McpError {}

impl From<std::io::Error> for McpError {
    fn from(error: std::io::Error) -> Self {
        McpError::internal(error.to_string())
    }
}

impl From<serde_json::Error> for McpError {
    fn from(error: serde_json::Error) -> Self {
        McpError::bad_request(format!("配置解析失败：{error}"))
    }
}

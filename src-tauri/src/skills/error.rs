use serde::Serialize;

/// Skills 领域错误。实现 `Serialize` 以便作为 Tauri command 的错误类型，
/// 前端可读取 `code` / `message` / `status`（沿用原 Fastify 的语义）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillsError {
    pub code: String,
    pub message: String,
    pub status: u16,
}

impl SkillsError {
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

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: "internal_error".to_owned(),
            message: message.into(),
            status: 500,
        }
    }
}

impl std::fmt::Display for SkillsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl std::error::Error for SkillsError {}

impl From<std::io::Error> for SkillsError {
    fn from(error: std::io::Error) -> Self {
        SkillsError::internal(error.to_string())
    }
}

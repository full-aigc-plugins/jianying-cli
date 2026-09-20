use serde::Serialize;
use serde_json::Value;

/// 机器可解析的 CLI 错误体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResponseError {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
    details: Value,
    recovery: Vec<String>,
}

impl ResponseError {
    pub(crate) fn new(
        error_type: impl Into<String>,
        message: impl Into<String>,
        details: Value,
        recovery: Vec<String>,
    ) -> Self {
        Self {
            error_type: error_type.into(),
            message: message.into(),
            details,
            recovery,
        }
    }
}

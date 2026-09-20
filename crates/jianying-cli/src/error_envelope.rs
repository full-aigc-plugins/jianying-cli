use crate::ResponseError;
use serde::Serialize;
use serde_json::{Map, Value};

/// CLI JSON 模式失败响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope {
    ok: bool,
    error: ResponseError,
}

impl ErrorEnvelope {
    /// 包装稳定错误类别和用户可读消息。
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error: ResponseError::new(kind, message, Value::Object(Map::new()), Vec::new()),
        }
    }

    /// 包装含机器可读详情与恢复建议的失败结果。
    pub fn with_details(
        error_type: impl Into<String>,
        message: impl Into<String>,
        details: Value,
        recovery: Vec<String>,
    ) -> Self {
        Self {
            ok: false,
            error: ResponseError::new(error_type, message, details, recovery),
        }
    }
}

use serde::Serialize;

/// CLI JSON 模式成功响应。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SuccessEnvelope<T> {
    ok: bool,
    data: T,
}

impl<T> SuccessEnvelope<T> {
    /// 包装成功结果。
    pub fn new(data: T) -> Self {
        Self { ok: true, data }
    }
}

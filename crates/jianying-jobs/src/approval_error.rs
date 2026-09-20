use thiserror::Error;

/// 审批记录的校验、匹配与持久化错误。
#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("approval field {0} must not be empty")]
    EmptyField(&'static str),
    #[error("approval path {field} must be absolute: {value}")]
    RelativePath { field: &'static str, value: String },
    #[error("approval ttl must be greater than zero")]
    InvalidTtl,
    #[error("approval ttl overflows epoch seconds")]
    TtlOverflow,
    #[error("invalid approval id {0}")]
    InvalidId(String),
    #[error("approval {0} was not found")]
    NotFound(String),
    #[error("approval binding does not match the granted command")]
    BindingMismatch,
    #[error("approval expired at {expires_at}; current time is {now}")]
    Expired { expires_at: u64, now: u64 },
    #[error("approval was already consumed at {0}")]
    AlreadyConsumed(u64),
    #[error("approval storage I/O failed: {0}")]
    Io(String),
    #[error("approval storage JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl ApprovalError {
    /// 将底层 I/O 错误转换为不泄漏内部类型的稳定错误。
    pub fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

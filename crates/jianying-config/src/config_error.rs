use thiserror::Error;

/// 配置 profile、键路径、校验和持久化错误。
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid config profile {0}")]
    InvalidProfile(String),
    #[error("invalid config key path {0}")]
    InvalidKey(String),
    #[error("config key {0} was not found")]
    KeyNotFound(String),
    #[error("config path {0} crosses a non-object value")]
    TypeConflict(String),
    #[error("config patch must be a JSON object")]
    PatchMustBeObject,
    #[error("configuration is managed by a read-only host")]
    ReadOnly,
    #[error("config schema mismatch: expected jianying-config/v1, got {0}")]
    InvalidSchema(String),
    #[error("config profile mismatch: expected {expected}, got {actual}")]
    ProfileMismatch { expected: String, actual: String },
    #[error("config I/O failed: {0}")]
    Io(String),
    #[error("config JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl ConfigError {
    /// 将底层 I/O 错误转换为稳定错误。
    pub fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

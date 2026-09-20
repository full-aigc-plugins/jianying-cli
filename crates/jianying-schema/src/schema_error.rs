use thiserror::Error;

/// Job v2 解析或形状校验失败。
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("invalid schema: expected {expected}, got {actual}")]
    InvalidSchema {
        expected: &'static str,
        actual: String,
    },
    #[error("unsupported compatibility schema: {actual}")]
    InvalidCompatibilitySchema { actual: String },
    #[error("invalid domain model: {0}")]
    InvalidDomain(String),
    #[error("invalid field: {0}")]
    InvalidField(&'static str),
    #[error("invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),
}

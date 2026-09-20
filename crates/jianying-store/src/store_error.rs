use thiserror::Error;

/// 草稿存储布局参数错误。
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("store root must not be empty")]
    EmptyRoot,
    #[error("draft name must be a safe single path component")]
    UnsafeDraftName,
    #[error("source draft directory does not exist: {0}")]
    InvalidSource(String),
    #[error("mutation phase does not allow this operation: {0}")]
    InvalidPhase(String),
    #[error("work-copy validation failed: {0}")]
    Validation(String),
    #[error("atomic commit failed: {0}")]
    Commit(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

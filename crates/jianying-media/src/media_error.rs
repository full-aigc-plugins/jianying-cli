use thiserror::Error;

/// 媒体元数据校验错误。
#[derive(Debug, Error)]
pub enum MediaError {
    #[error("media path must not be empty")]
    EmptyPath,
    #[error("media duration must be positive")]
    InvalidDuration,
    #[error("media dimensions must be positive")]
    InvalidDimensions,
}

use crate::{AsrOutputFormat, AsrTimestampGranularity};
use std::path::PathBuf;
use thiserror::Error;

/// ASR 请求、能力校验与产物错误。
#[derive(Debug, Error)]
pub enum AsrError {
    #[error("ASR source SHA-256 must be a 64 character hexadecimal digest")]
    InvalidSourceDigest,
    #[error("ASR provider id must not be empty")]
    EmptyProviderId,
    #[error("ASR executor identity must not be empty")]
    EmptyExecutorIdentity,
    #[error("ASR provider must declare at least one output format")]
    EmptyOutputFormats,
    #[error("ASR output format is unsupported: {0:?}")]
    UnsupportedOutputFormat(AsrOutputFormat),
    #[error("ASR timestamps require verbose_json output")]
    TimestampsRequireVerboseJson,
    #[error("ASR timestamp granularity is unsupported: {0:?}")]
    UnsupportedTimestampGranularity(AsrTimestampGranularity),
    #[error("ASR source is missing: {0}")]
    SourceMissing(PathBuf),
    #[error("ASR source SHA-256 does not match the request")]
    SourceDigestMismatch,
    #[error("ASR requested model {requested} does not match configured model {configured}")]
    ModelMismatch {
        requested: String,
        configured: String,
    },
    #[error("invalid local ASR process: {0}")]
    InvalidLocalProcess(String),
    #[error("local ASR process exited with code {code:?}: {stderr}")]
    ProcessFailed { code: Option<i32>, stderr: String },
    #[error("ASR artifact is missing or empty: {0}")]
    EmptyArtifact(PathBuf),
    #[error("ASR I/O failed: {0}")]
    Io(String),
}

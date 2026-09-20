use std::path::PathBuf;
use thiserror::Error;

/// 运行时档案字段错误。
#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime field {0} must not be empty")]
    EmptyField(&'static str),
    #[error("runtime capability must not be empty")]
    EmptyCapability,
    #[error("runtime file does not exist: {0}")]
    FileMissing(PathBuf),
    #[error("runtime path is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("runtime destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("runtime destination must not be inside source: {0}")]
    DestinationInsideSource(PathBuf),
    #[error("runtime symbolic links are not accepted: {0}")]
    SymbolicLink(PathBuf),
    #[error("runtime I/O failed for {path}: {message}")]
    Io { path: PathBuf, message: String },
    #[error("unsupported runtime product: expected {expected}, observed {observed}")]
    UnsupportedProduct { expected: String, observed: String },
    #[error("unsupported runtime version: expected {expected}, observed {observed}")]
    UnsupportedVersion { expected: String, observed: String },
    #[error("unsupported runtime platform")]
    UnsupportedPlatform,
    #[error("runtime file identity mismatch: {0}")]
    FileIdentityMismatch(PathBuf),
    #[error("runtime profile has no file identity")]
    MissingFileIdentity,
    #[error("runtime draft root is not approved: {0}")]
    UnapprovedDraftRoot(PathBuf),
    #[error("runtime draft root is not writable: {0}")]
    DraftRootNotWritable(PathBuf),
    #[error("material unavailable: {0}")]
    MaterialUnavailable(PathBuf),
    #[error("unsupported runtime capability: {0}")]
    UnsupportedCapability(String),
    #[error("runtime executable path must be absolute: {0}")]
    ExecutableNotAbsolute(PathBuf),
    #[error("runtime process is already running")]
    AlreadyRunning,
    #[error("runtime process is not running")]
    NotRunning,
    #[error("runtime process state lock is poisoned")]
    ProcessLockPoisoned,
    #[error("runtime process operation failed: {0}")]
    Process(String),
    #[error("runtime profile id cannot be used for persistent ownership: {0}")]
    InvalidProfileId(String),
    #[error("runtime ownership record is invalid: {0}")]
    InvalidOwnershipRecord(String),
    #[error(
        "runtime ownership check failed for pid {0}; refusing to control an unverified process"
    )]
    OwnershipMismatch(u32),
}

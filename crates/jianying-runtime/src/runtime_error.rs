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
    #[error("invalid entitlement evidence: {0}")]
    InvalidEntitlement(String),
    #[error("runtime entitlement is not active: {0}")]
    EntitlementNotActive(String),
    #[error("runtime entitlement expired at {0}")]
    EntitlementExpired(u64),
    #[error("runtime edition {observed} does not satisfy required edition {required}")]
    InsufficientEdition { required: String, observed: String },
    #[error("runtime entitlement does not provide capability: {0}")]
    EntitlementCapabilityMissing(String),
    #[error("official asset identity mismatch: {0}")]
    AssetIdentityMismatch(PathBuf),
    #[error("official draft resource is missing or ambiguous: {0}")]
    DraftResourceUnavailable(String),
    #[error("official resource identity kind does not match the verification input")]
    ResourceIdentityKindMismatch,
    #[error("official asset is preview-only and cannot be delivered")]
    PreviewOnlyAsset,
    #[error("official asset usage is not allowed: {0}")]
    AssetUsageNotAllowed(String),
    #[error("invalid official asset receipt: {0}")]
    InvalidAssetReceipt(String),
    #[error(
        "runtime ownership check failed for pid {0}; refusing to control an unverified process"
    )]
    OwnershipMismatch(u32),
    #[error("unknown runtime control: {0}")]
    UnknownControl(String),
    #[error(
        "runtime control profile mismatch for {field}: expected {expected}, observed {observed}"
    )]
    ControlProfileMismatch {
        field: String,
        expected: String,
        observed: String,
    },
    #[error("runtime control {control} requires {expected}, not {requested}")]
    ControlOperationMismatch {
        control: String,
        expected: String,
        requested: String,
    },
    #[error("runtime control {control} is unavailable: {reason}")]
    ControlUnavailable { control: String, reason: String },
}

use crate::NativeExportState;
use thiserror::Error;

/// 原生导出授权、状态机和制品验证错误。
#[derive(Debug, Error)]
pub enum NativeExportError {
    #[error("export request is not native")]
    NotNativeExport,
    #[error("native export submission is invalid: {0}")]
    InvalidSubmission(String),
    #[error("native export runtime profile lacks render.native")]
    MissingCapability,
    #[error("native export runtime evidence does not match the profile")]
    RuntimeEvidenceMismatch,
    #[error("native export output already exists and overwrite is false")]
    OutputExists,
    #[error("native export approval failed: {0}")]
    Approval(#[from] crate::ApprovalError),
    #[error("native export task was not found: {0}")]
    NotFound(String),
    #[error("invalid native export transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: NativeExportState,
        to: NativeExportState,
    },
    #[error("native export progress regressed from {current} to {next}")]
    ProgressRegression { current: u8, next: u8 },
    #[error("native export progress must be between 0 and 99 before verification")]
    InvalidProgress,
    #[error("native export artifact is missing or empty for task {task_id}")]
    ArtifactMissing { task_id: String },
    #[error("native export artifact digest drifted for task {task_id}")]
    ArtifactDrift { task_id: String },
    #[error("native export output did not change for task {task_id}")]
    ArtifactUnchanged { task_id: String },
    #[error("native export I/O failed: {0}")]
    Io(String),
    #[error("native export JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl NativeExportError {
    pub(crate) fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

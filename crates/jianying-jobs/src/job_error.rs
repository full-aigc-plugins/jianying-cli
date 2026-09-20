use crate::JobState;
use thiserror::Error;

/// 作业检查点字段或状态迁移错误。
#[derive(Debug, Error)]
pub enum JobError {
    #[error("task id must not be empty")]
    EmptyTaskId,
    #[error("invalid job transition from {from:?} to {to:?}")]
    InvalidTransition { from: JobState, to: JobState },
    #[error("task {0} was not found")]
    NotFound(String),
    #[error("job storage I/O failed: {0}")]
    Io(String),
    #[error("job storage JSON failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("task {task_id} revision conflict: expected previous {expected:?}, actual {actual:?}")]
    RevisionConflict {
        task_id: String,
        expected: Option<u64>,
        actual: Option<u64>,
    },
    #[error("job storage SQLite failed: {0}")]
    Sqlite(String),
}

impl JobError {
    pub fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }

    /// 将 SQLite 错误转换为稳定存储错误。
    pub fn sqlite(error: rusqlite::Error) -> Self {
        Self::Sqlite(error.to_string())
    }
}

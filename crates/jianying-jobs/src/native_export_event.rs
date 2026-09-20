use crate::NativeExportState;
use serde::{Deserialize, Serialize};

/// 原生导出任务的状态与进度检查点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeExportEvent {
    state: NativeExportState,
    progress_percent: u8,
    reason: String,
    at: u64,
}

impl NativeExportEvent {
    pub(crate) fn new(
        state: NativeExportState,
        progress_percent: u8,
        reason: impl Into<String>,
        at: u64,
    ) -> Self {
        Self {
            state,
            progress_percent,
            reason: reason.into(),
            at,
        }
    }
}

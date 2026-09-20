use serde::{Deserialize, Serialize};

/// 显式授权原生导出任务的持久状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeExportState {
    Queued,
    Running,
    Verifying,
    Succeeded,
    Failed,
    Interrupted,
}

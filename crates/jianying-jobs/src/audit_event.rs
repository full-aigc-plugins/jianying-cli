use crate::JobState;
use serde::{Deserialize, Serialize};

/// 作业状态变化的追加式审计事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEvent {
    pub sequence: u64,
    pub state: JobState,
    pub reason: String,
    pub epoch_seconds: u64,
}

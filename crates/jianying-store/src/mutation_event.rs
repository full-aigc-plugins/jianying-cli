use crate::MutationPhase;
use serde::{Deserialize, Serialize};

/// 写事务的追加式审计事件，始终包含可执行恢复提示。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationEvent {
    pub sequence: u64,
    pub phase: MutationPhase,
    pub action: String,
    pub message: String,
    pub recovery_command: String,
}

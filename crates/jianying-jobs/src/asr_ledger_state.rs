use serde::{Deserialize, Serialize};

/// ASR 请求在幂等账本中的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrLedgerState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Ambiguous,
}

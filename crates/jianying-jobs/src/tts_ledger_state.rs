use serde::{Deserialize, Serialize};

/// 云端 TTS 请求在幂等账本中的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsLedgerState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Ambiguous,
}

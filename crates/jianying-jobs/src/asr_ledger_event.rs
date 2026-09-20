use crate::AsrLedgerState;
use serde::{Deserialize, Serialize};

/// ASR 账本中的不可省略状态事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrLedgerEvent {
    state: AsrLedgerState,
    reason: String,
    at: u64,
}

impl AsrLedgerEvent {
    pub(crate) fn new(state: AsrLedgerState, reason: impl Into<String>, at: u64) -> Self {
        Self {
            state,
            reason: reason.into(),
            at,
        }
    }
}

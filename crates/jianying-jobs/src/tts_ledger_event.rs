use crate::TtsLedgerState;
use serde::{Deserialize, Serialize};

/// 单次 TTS 账本状态变化的审计事件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsLedgerEvent {
    state: TtsLedgerState,
    reason: String,
    epoch_seconds: u64,
}

impl TtsLedgerEvent {
    pub(crate) fn new(
        state: TtsLedgerState,
        reason: impl Into<String>,
        epoch_seconds: u64,
    ) -> Self {
        Self {
            state,
            reason: reason.into(),
            epoch_seconds,
        }
    }

    /// 返回事件状态。
    pub fn state(&self) -> TtsLedgerState {
        self.state
    }

    /// 返回不包含原始文本和凭据的审计原因。
    pub fn reason(&self) -> &str {
        &self.reason
    }

    /// 返回事件纪元秒。
    pub fn epoch_seconds(&self) -> u64 {
        self.epoch_seconds
    }
}

use crate::TtsLedgerState;
use thiserror::Error;

/// TTS 幂等账本和费用审批门禁错误。
#[derive(Debug, Error)]
pub enum TtsLedgerError {
    #[error("invalid TTS submission: {0}")]
    InvalidSubmission(String),
    #[error("TTS approval failed: {0}")]
    Approval(#[from] crate::ApprovalError),
    #[error("TTS ledger record {0} was not found")]
    NotFound(String),
    #[error("TTS request {idempotency_key} is already {state:?}")]
    DuplicateInFlight {
        idempotency_key: String,
        state: TtsLedgerState,
    },
    #[error("TTS request {idempotency_key} is ambiguous and requires reconciliation")]
    AmbiguousRequiresReconciliation { idempotency_key: String },
    #[error("TTS request {idempotency_key} failed and requires explicit retry with new approval")]
    FailedRequiresExplicitRetry { idempotency_key: String },
    #[error("invalid TTS ledger transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: TtsLedgerState,
        to: TtsLedgerState,
    },
    #[error("TTS artifact is missing for {idempotency_key}")]
    ArtifactMissing { idempotency_key: String },
    #[error("TTS artifact digest drifted for {idempotency_key}")]
    ArtifactDrift { idempotency_key: String },
    #[error("TTS ledger I/O failed: {0}")]
    Io(String),
    #[error("TTS ledger JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl TtsLedgerError {
    pub(crate) fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

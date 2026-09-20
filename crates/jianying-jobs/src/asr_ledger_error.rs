use crate::AsrLedgerState;
use thiserror::Error;

/// ASR 幂等账本和费用审批门禁错误。
#[derive(Debug, Error)]
pub enum AsrLedgerError {
    #[error("invalid ASR submission: {0}")]
    InvalidSubmission(String),
    #[error("ASR approval failed: {0}")]
    Approval(#[from] crate::ApprovalError),
    #[error("ASR ledger record {0} was not found")]
    NotFound(String),
    #[error("ASR request {idempotency_key} is already {state:?}")]
    DuplicateInFlight {
        idempotency_key: String,
        state: AsrLedgerState,
    },
    #[error("ASR request {idempotency_key} is ambiguous and requires reconciliation")]
    AmbiguousRequiresReconciliation { idempotency_key: String },
    #[error("ASR request {idempotency_key} failed and requires explicit retry")]
    FailedRequiresExplicitRetry { idempotency_key: String },
    #[error("invalid ASR ledger transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: AsrLedgerState,
        to: AsrLedgerState,
    },
    #[error("ASR artifact is missing for {idempotency_key}")]
    ArtifactMissing { idempotency_key: String },
    #[error("ASR artifact digest drifted for {idempotency_key}")]
    ArtifactDrift { idempotency_key: String },
    #[error("ASR ledger I/O failed: {0}")]
    Io(String),
    #[error("ASR ledger JSON failed: {0}")]
    Json(#[from] serde_json::Error),
}

impl AsrLedgerError {
    pub(crate) fn io(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

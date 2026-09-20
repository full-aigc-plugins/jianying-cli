use crate::{AsrLedgerError, AsrLedgerEvent, AsrLedgerState, AsrSubmission};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 不保存音频正文或凭据的持久化 ASR 账本记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrLedgerRecord {
    idempotency_key: String,
    provider_id: String,
    executor_identity: String,
    source_sha256: String,
    request_sha256: String,
    target: PathBuf,
    paid: bool,
    max_cost_microunits: u64,
    state: AsrLedgerState,
    attempts: u64,
    approval_id: Option<String>,
    external_request_id: Option<String>,
    artifact_path: Option<PathBuf>,
    artifact_sha256: Option<String>,
    created_at: u64,
    updated_at: u64,
    history: Vec<AsrLedgerEvent>,
}

impl AsrLedgerRecord {
    pub(crate) fn queued(submission: &AsrSubmission, approval_id: Option<&str>, now: u64) -> Self {
        Self {
            idempotency_key: submission.idempotency_key().to_owned(),
            provider_id: submission.provider_id().to_owned(),
            executor_identity: submission.executor_identity().to_owned(),
            source_sha256: submission.source_sha256().to_owned(),
            request_sha256: submission.request_sha256().to_owned(),
            target: submission.target().to_path_buf(),
            paid: submission.paid(),
            max_cost_microunits: submission.max_cost_microunits(),
            state: AsrLedgerState::Queued,
            attempts: 0,
            approval_id: approval_id.map(str::to_owned),
            external_request_id: None,
            artifact_path: None,
            artifact_sha256: None,
            created_at: now,
            updated_at: now,
            history: vec![AsrLedgerEvent::new(
                AsrLedgerState::Queued,
                "validated and queued",
                now,
            )],
        }
    }

    pub(crate) fn transition(
        &mut self,
        next: AsrLedgerState,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<(), AsrLedgerError> {
        let allowed = matches!(
            (self.state, next),
            (AsrLedgerState::Queued, AsrLedgerState::Running)
                | (AsrLedgerState::Running, AsrLedgerState::Succeeded)
                | (AsrLedgerState::Running, AsrLedgerState::Failed)
                | (AsrLedgerState::Running, AsrLedgerState::Ambiguous)
                | (AsrLedgerState::Ambiguous, AsrLedgerState::Failed)
                | (AsrLedgerState::Failed, AsrLedgerState::Queued)
        );
        if !allowed {
            return Err(AsrLedgerError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        if next == AsrLedgerState::Running {
            self.attempts += 1;
        }
        self.updated_at = now;
        self.history.push(AsrLedgerEvent::new(next, reason, now));
        Ok(())
    }

    pub(crate) fn replace_approval(&mut self, approval_id: Option<&str>) {
        self.approval_id = approval_id.map(str::to_owned);
    }

    pub(crate) fn attach_artifact(
        &mut self,
        artifact_path: &Path,
        artifact_sha256: String,
        external_request_id: Option<&str>,
    ) {
        self.artifact_path = Some(artifact_path.to_path_buf());
        self.artifact_sha256 = Some(artifact_sha256);
        self.external_request_id = external_request_id.map(str::to_owned);
    }

    /// 返回当前状态。
    pub fn state(&self) -> AsrLedgerState {
        self.state
    }

    /// 返回真正启动执行器的次数。
    pub fn attempts(&self) -> u64 {
        self.attempts
    }

    /// 返回最近一次付费审批 ID；本地执行为 None。
    pub fn approval_id(&self) -> Option<&str> {
        self.approval_id.as_deref()
    }

    /// 返回幂等键。
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// 返回成功转录制品路径。
    pub fn artifact_path(&self) -> Option<&Path> {
        self.artifact_path.as_deref()
    }

    /// 返回成功转录制品 SHA-256。
    pub fn artifact_sha256(&self) -> Option<&str> {
        self.artifact_sha256.as_deref()
    }
}

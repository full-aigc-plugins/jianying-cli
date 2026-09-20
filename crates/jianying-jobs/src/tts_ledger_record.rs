use crate::{TtsExecutionMode, TtsLedgerError, TtsLedgerEvent, TtsLedgerState, TtsSubmission};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 不保存原始文本或凭据的持久化 TTS 账本记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsLedgerRecord {
    idempotency_key: String,
    provider_id: String,
    model: Option<String>,
    voice: Option<String>,
    text_sha256: String,
    request_sha256: String,
    target: PathBuf,
    max_cost_microunits: u64,
    execution_mode: TtsExecutionMode,
    state: TtsLedgerState,
    attempts: u64,
    approval_id: String,
    external_request_id: Option<String>,
    artifact_path: Option<PathBuf>,
    artifact_sha256: Option<String>,
    created_at: u64,
    updated_at: u64,
    history: Vec<TtsLedgerEvent>,
}

impl TtsLedgerRecord {
    pub(crate) fn queued(
        submission: &TtsSubmission,
        approval_id: impl Into<String>,
        now: u64,
    ) -> Self {
        Self {
            idempotency_key: submission.idempotency_key().to_owned(),
            provider_id: submission.provider_id().to_owned(),
            model: submission.model().map(str::to_owned),
            voice: submission.voice().map(str::to_owned),
            text_sha256: submission.text_sha256().to_owned(),
            request_sha256: submission.request_sha256().to_owned(),
            target: submission.target().to_path_buf(),
            max_cost_microunits: submission.max_cost_microunits(),
            execution_mode: submission.execution_mode(),
            state: TtsLedgerState::Queued,
            attempts: 0,
            approval_id: approval_id.into(),
            external_request_id: None,
            artifact_path: None,
            artifact_sha256: None,
            created_at: now,
            updated_at: now,
            history: vec![TtsLedgerEvent::new(
                TtsLedgerState::Queued,
                "approved and queued",
                now,
            )],
        }
    }

    pub(crate) fn transition(
        &mut self,
        next: TtsLedgerState,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<(), TtsLedgerError> {
        let allowed = matches!(
            (self.state, next),
            (TtsLedgerState::Queued, TtsLedgerState::Running)
                | (TtsLedgerState::Running, TtsLedgerState::Succeeded)
                | (TtsLedgerState::Running, TtsLedgerState::Failed)
                | (TtsLedgerState::Running, TtsLedgerState::Ambiguous)
                | (TtsLedgerState::Ambiguous, TtsLedgerState::Failed)
                | (TtsLedgerState::Failed, TtsLedgerState::Queued)
        );
        if !allowed {
            return Err(TtsLedgerError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        self.state = next;
        if next == TtsLedgerState::Running {
            self.attempts += 1;
        }
        self.updated_at = now;
        self.history.push(TtsLedgerEvent::new(next, reason, now));
        Ok(())
    }

    pub(crate) fn replace_approval(&mut self, approval_id: impl Into<String>) {
        self.approval_id = approval_id.into();
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
    pub fn state(&self) -> TtsLedgerState {
        self.state
    }

    /// 返回实际启动远端提交的次数。
    pub fn attempts(&self) -> u64 {
        self.attempts
    }

    /// 返回最近一次显式审批 ID。
    pub fn approval_id(&self) -> &str {
        &self.approval_id
    }

    /// 返回幂等键。
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    /// 返回已验证的本地音频制品路径。
    pub fn artifact_path(&self) -> Option<&Path> {
        self.artifact_path.as_deref()
    }

    /// 返回已验证音频制品的 SHA-256。
    pub fn artifact_sha256(&self) -> Option<&str> {
        self.artifact_sha256.as_deref()
    }

    /// 返回厂商请求 ID，供审计与对账使用。
    pub fn external_request_id(&self) -> Option<&str> {
        self.external_request_id.as_deref()
    }
}

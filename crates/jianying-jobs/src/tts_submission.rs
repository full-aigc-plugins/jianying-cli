use crate::{ApprovalBinding, ApprovalError, TtsExecutionMode, TtsLedgerError};
use jianying_media::TtsRequest;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// 一次云端 TTS 提交的无秘密、可哈希上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TtsSubmission {
    provider_id: String,
    model: Option<String>,
    voice: Option<String>,
    text_sha256: String,
    request_sha256: String,
    idempotency_key: String,
    cwd: PathBuf,
    target: PathBuf,
    task_id: String,
    max_cost_microunits: u64,
    execution_mode: TtsExecutionMode,
}

impl TtsSubmission {
    /// 从统一请求生成内容哈希、请求哈希和目标级幂等键。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: impl Into<String>,
        request: &TtsRequest,
        cwd: PathBuf,
        target: PathBuf,
        task_id: impl Into<String>,
        max_cost_microunits: u64,
        execution_mode: TtsExecutionMode,
    ) -> Result<Self, TtsLedgerError> {
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(TtsLedgerError::InvalidSubmission(
                "empty provider id".to_owned(),
            ));
        }
        if !cwd.is_absolute() || !target.is_absolute() {
            return Err(TtsLedgerError::InvalidSubmission(
                "cwd and target must be absolute".to_owned(),
            ));
        }
        let task_id = task_id.into();
        if task_id.trim().is_empty() {
            return Err(TtsLedgerError::InvalidSubmission(
                "empty task id".to_owned(),
            ));
        }
        if max_cost_microunits == 0 {
            return Err(TtsLedgerError::InvalidSubmission(
                "maximum cost must be positive".to_owned(),
            ));
        }
        let request_bytes = serde_json::to_vec(request)?;
        let text_sha256 = sha256(request.text().as_bytes());
        let request_sha256 = sha256(&request_bytes);
        let mut key_digest = Sha256::new();
        update_framed(&mut key_digest, provider_id.as_bytes());
        update_framed(&mut key_digest, execution_mode.key_name().as_bytes());
        update_framed(&mut key_digest, &request_bytes);
        update_framed(&mut key_digest, target.as_os_str().as_encoded_bytes());
        let idempotency_key = format!("{:x}", key_digest.finalize());
        Ok(Self {
            provider_id,
            model: request.model().map(str::to_owned),
            voice: request.voice().map(str::to_owned),
            text_sha256,
            request_sha256,
            idempotency_key,
            cwd,
            target,
            task_id,
            max_cost_microunits,
            execution_mode,
        })
    }

    /// 生成精确绑定 provider、模型、音色、内容哈希、目标和预算的审批上下文。
    pub fn approval_binding(&self) -> Result<ApprovalBinding, ApprovalError> {
        ApprovalBinding::new(
            self.execution_mode.approval_command().to_owned(),
            vec![
                "--provider".to_owned(),
                self.provider_id.clone(),
                "--model".to_owned(),
                self.model.clone().unwrap_or_default(),
                "--voice".to_owned(),
                self.voice.clone().unwrap_or_default(),
                "--text-sha256".to_owned(),
                self.text_sha256.clone(),
                "--request-sha256".to_owned(),
                self.request_sha256.clone(),
                "--max-cost-microunits".to_owned(),
                self.max_cost_microunits.to_string(),
            ],
            self.cwd.clone(),
            self.target.clone(),
            self.task_id.clone(),
        )
    }

    /// 返回稳定幂等键。
    pub fn idempotency_key(&self) -> &str {
        &self.idempotency_key
    }

    pub(crate) fn provider_id(&self) -> &str {
        &self.provider_id
    }

    pub(crate) fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn voice(&self) -> Option<&str> {
        self.voice.as_deref()
    }

    pub(crate) fn text_sha256(&self) -> &str {
        &self.text_sha256
    }

    pub(crate) fn request_sha256(&self) -> &str {
        &self.request_sha256
    }

    pub(crate) fn target(&self) -> &std::path::Path {
        &self.target
    }

    pub(crate) fn max_cost_microunits(&self) -> u64 {
        self.max_cost_microunits
    }

    /// 返回本次提交的生产或独立 live-canary 模式。
    pub fn execution_mode(&self) -> TtsExecutionMode {
        self.execution_mode
    }
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn update_framed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

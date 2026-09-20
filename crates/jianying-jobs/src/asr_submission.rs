use crate::{ApprovalBinding, ApprovalError, AsrLedgerError};
use jianying_media::AsrRequest;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// 一次 ASR 执行的无音频、无凭据、可哈希上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsrSubmission {
    provider_id: String,
    executor_identity: String,
    source_sha256: String,
    request_sha256: String,
    idempotency_key: String,
    cwd: PathBuf,
    target: PathBuf,
    task_id: String,
    paid: bool,
    max_cost_microunits: u64,
}

impl AsrSubmission {
    /// 用源内容哈希、完整请求和执行器身份生成稳定幂等键。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: impl Into<String>,
        executor_identity: impl Into<String>,
        request: &AsrRequest,
        cwd: PathBuf,
        target: PathBuf,
        task_id: impl Into<String>,
        paid: bool,
        max_cost_microunits: u64,
    ) -> Result<Self, AsrLedgerError> {
        let provider_id = provider_id.into();
        let executor_identity = executor_identity.into();
        let task_id = task_id.into();
        if provider_id.trim().is_empty()
            || executor_identity.trim().is_empty()
            || task_id.trim().is_empty()
        {
            return Err(AsrLedgerError::InvalidSubmission(
                "provider, executor identity and task id must not be empty".to_owned(),
            ));
        }
        if !cwd.is_absolute() || !target.is_absolute() {
            return Err(AsrLedgerError::InvalidSubmission(
                "cwd and target must be absolute".to_owned(),
            ));
        }
        if paid && max_cost_microunits == 0 {
            return Err(AsrLedgerError::InvalidSubmission(
                "paid ASR maximum cost must be positive".to_owned(),
            ));
        }
        if !paid && max_cost_microunits != 0 {
            return Err(AsrLedgerError::InvalidSubmission(
                "local ASR maximum cost must be zero".to_owned(),
            ));
        }
        let request_bytes = serde_json::to_vec(request)?;
        let request_sha256 = sha256(&request_bytes);
        let mut digest = Sha256::new();
        update_framed(&mut digest, provider_id.as_bytes());
        update_framed(&mut digest, executor_identity.as_bytes());
        update_framed(&mut digest, request.source_sha256().as_bytes());
        update_framed(&mut digest, &request_bytes);
        let idempotency_key = format!("{:x}", digest.finalize());
        Ok(Self {
            provider_id,
            executor_identity,
            source_sha256: request.source_sha256().to_owned(),
            request_sha256,
            idempotency_key,
            cwd,
            target,
            task_id,
            paid,
            max_cost_microunits,
        })
    }

    /// 为付费 ASR 生成绑定执行器、源哈希、请求哈希、目标和预算的审批上下文。
    pub fn approval_binding(&self) -> Result<ApprovalBinding, ApprovalError> {
        ApprovalBinding::new(
            "asr.cloud.submit".to_owned(),
            vec![
                "--provider".to_owned(),
                self.provider_id.clone(),
                "--executor".to_owned(),
                self.executor_identity.clone(),
                "--source-sha256".to_owned(),
                self.source_sha256.clone(),
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

    pub(crate) fn executor_identity(&self) -> &str {
        &self.executor_identity
    }

    pub(crate) fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    pub(crate) fn request_sha256(&self) -> &str {
        &self.request_sha256
    }

    pub(crate) fn target(&self) -> &std::path::Path {
        &self.target
    }

    pub(crate) fn paid(&self) -> bool {
        self.paid
    }

    pub(crate) fn max_cost_microunits(&self) -> u64 {
        self.max_cost_microunits
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn update_framed(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_be_bytes());
    digest.update(bytes);
}

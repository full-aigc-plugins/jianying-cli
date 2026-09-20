use crate::{
    ApprovalStore, AsrLedgerDecision, AsrLedgerError, AsrLedgerRecord, AsrLedgerState,
    AsrSubmission,
};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

/// 以内容哈希和执行器身份寻址的 ASR 持久账本。
pub struct AsrLedgerStore {
    root: PathBuf,
}

impl AsrLedgerStore {
    /// 打开 ASR 账本根目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 准备无需费用审批的本地执行；付费请求会 fail closed。
    pub fn prepare_local(
        &self,
        submission: &AsrSubmission,
        now: u64,
    ) -> Result<AsrLedgerDecision, AsrLedgerError> {
        if submission.paid() {
            return Err(AsrLedgerError::InvalidSubmission(
                "paid ASR requires prepare_paid".to_owned(),
            ));
        }
        self.prepare_claimed(submission, None, now)
    }

    /// 在远端提交前消费精确费用审批并建立原子 claim。
    pub fn prepare_paid(
        &self,
        submission: &AsrSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<AsrLedgerDecision, AsrLedgerError> {
        if !submission.paid() {
            return Err(AsrLedgerError::InvalidSubmission(
                "local ASR must use prepare_local".to_owned(),
            ));
        }
        self.with_claim(submission, |store| {
            let binding = submission.approval_binding()?;
            let original = approvals.load(approval_id)?;
            approvals.consume(approval_id, &binding, now)?;
            let record = AsrLedgerRecord::queued(submission, Some(approval_id), now);
            if let Err(error) = store.save(&record) {
                let _ = approvals.save(&original);
                return Err(error);
            }
            Ok(AsrLedgerDecision::Submit(record))
        })
    }

    /// 对已明确失败的本地请求执行显式重试。
    pub fn retry_local(
        &self,
        submission: &AsrSubmission,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        if submission.paid() {
            return Err(AsrLedgerError::InvalidSubmission(
                "paid ASR requires retry_paid".to_owned(),
            ));
        }
        self.retry_claimed(submission, None, None, now)
    }

    /// 对已明确失败的付费请求消费新审批后显式重试。
    pub fn retry_paid(
        &self,
        submission: &AsrSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        if !submission.paid() {
            return Err(AsrLedgerError::InvalidSubmission(
                "local ASR must use retry_local".to_owned(),
            ));
        }
        self.retry_claimed(submission, Some(approvals), Some(approval_id), now)
    }

    /// 标记执行器已真正开始；attempts 只在此递增。
    pub fn mark_running(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        self.transition(
            idempotency_key,
            AsrLedgerState::Running,
            "executor started",
            now,
        )
    }

    /// 标记执行器返回明确失败。
    pub fn mark_failed(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        self.transition(
            idempotency_key,
            AsrLedgerState::Failed,
            "definite failure",
            now,
        )
    }

    /// 标记外部执行结果未知，禁止自动重提。
    pub fn mark_ambiguous(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        self.transition(
            idempotency_key,
            AsrLedgerState::Ambiguous,
            "executor outcome is unknown",
            now,
        )
    }

    /// 在确认外部未接受请求后把 ambiguous 对账为 failed。
    pub fn reconcile_ambiguous_as_failed(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        self.transition(
            idempotency_key,
            AsrLedgerState::Failed,
            "reconciliation confirmed no accepted request",
            now,
        )
    }

    /// 保存非空转录产物哈希，供重复请求复用前复核。
    pub fn mark_succeeded(
        &self,
        idempotency_key: &str,
        artifact_path: &Path,
        external_request_id: Option<&str>,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        let bytes = std::fs::read(artifact_path).map_err(AsrLedgerError::io)?;
        if bytes.is_empty() {
            return Err(AsrLedgerError::ArtifactMissing {
                idempotency_key: idempotency_key.to_owned(),
            });
        }
        let mut record = self.load(idempotency_key)?;
        record.transition(AsrLedgerState::Succeeded, "artifact verified", now)?;
        record.attach_artifact(artifact_path, sha256(&bytes), external_request_id);
        self.save(&record)?;
        Ok(record)
    }

    /// 加载一条 ASR 记录。
    pub fn load(&self, idempotency_key: &str) -> Result<AsrLedgerRecord, AsrLedgerError> {
        validate_key(idempotency_key)?;
        let path = self.path(idempotency_key);
        if !path.is_file() {
            return Err(AsrLedgerError::NotFound(idempotency_key.to_owned()));
        }
        Ok(serde_json::from_slice(
            &std::fs::read(path).map_err(AsrLedgerError::io)?,
        )?)
    }

    /// 返回指定幂等键的记录文件路径。
    pub fn path(&self, idempotency_key: &str) -> PathBuf {
        self.root.join(format!("{idempotency_key}.json"))
    }

    fn prepare_claimed(
        &self,
        submission: &AsrSubmission,
        approval_id: Option<&str>,
        now: u64,
    ) -> Result<AsrLedgerDecision, AsrLedgerError> {
        self.with_claim(submission, |store| {
            let record = AsrLedgerRecord::queued(submission, approval_id, now);
            store.save(&record)?;
            Ok(AsrLedgerDecision::Submit(record))
        })
    }

    fn with_claim<T>(
        &self,
        submission: &AsrSubmission,
        action: impl FnOnce(&Self) -> Result<T, AsrLedgerError>,
    ) -> Result<T, AsrLedgerError>
    where
        T: From<AsrLedgerDecision>,
    {
        if self.path(submission.idempotency_key()).is_file() {
            let decision = self.existing_decision(self.load(submission.idempotency_key())?)?;
            return Ok(T::from(decision));
        }
        std::fs::create_dir_all(&self.root).map_err(AsrLedgerError::io)?;
        let claim = self.claim_path(submission.idempotency_key());
        match OpenOptions::new().write(true).create_new(true).open(&claim) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if self.path(submission.idempotency_key()).is_file() {
                    let decision =
                        self.existing_decision(self.load(submission.idempotency_key())?)?;
                    return Ok(T::from(decision));
                }
                return Err(AsrLedgerError::AmbiguousRequiresReconciliation {
                    idempotency_key: submission.idempotency_key().to_owned(),
                });
            }
            Err(error) => return Err(AsrLedgerError::io(error)),
        }
        let result = if self.path(submission.idempotency_key()).is_file() {
            self.existing_decision(self.load(submission.idempotency_key())?)
                .map(T::from)
        } else {
            action(self)
        };
        let _ = std::fs::remove_file(claim);
        result
    }

    fn retry_claimed(
        &self,
        submission: &AsrSubmission,
        approvals: Option<&ApprovalStore>,
        approval_id: Option<&str>,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        std::fs::create_dir_all(&self.root).map_err(AsrLedgerError::io)?;
        let claim = self.claim_path(submission.idempotency_key());
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim)
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    AsrLedgerError::AmbiguousRequiresReconciliation {
                        idempotency_key: submission.idempotency_key().to_owned(),
                    }
                } else {
                    AsrLedgerError::io(error)
                }
            })?;
        let result = (|| {
            let mut record = self.load(submission.idempotency_key())?;
            match record.state() {
                AsrLedgerState::Ambiguous => {
                    return Err(AsrLedgerError::AmbiguousRequiresReconciliation {
                        idempotency_key: record.idempotency_key().to_owned(),
                    });
                }
                AsrLedgerState::Failed => {}
                state => {
                    return Err(AsrLedgerError::DuplicateInFlight {
                        idempotency_key: record.idempotency_key().to_owned(),
                        state,
                    });
                }
            }
            let mut original_approval = None;
            if submission.paid() {
                let approvals = approvals.ok_or_else(|| {
                    AsrLedgerError::InvalidSubmission(
                        "paid retry requires approval store".to_owned(),
                    )
                })?;
                let approval_id = approval_id.ok_or_else(|| {
                    AsrLedgerError::InvalidSubmission("paid retry requires approval id".to_owned())
                })?;
                let binding = submission.approval_binding()?;
                original_approval = Some(approvals.load(approval_id)?);
                approvals.consume(approval_id, &binding, now)?;
            }
            record.transition(AsrLedgerState::Queued, "explicit retry", now)?;
            record.replace_approval(approval_id);
            if let Err(error) = self.save(&record) {
                if let (Some(approvals), Some(original)) = (approvals, original_approval.as_ref()) {
                    let _ = approvals.save(original);
                }
                return Err(error);
            }
            Ok(record)
        })();
        let _ = std::fs::remove_file(claim);
        result
    }

    fn transition(
        &self,
        idempotency_key: &str,
        next: AsrLedgerState,
        reason: &str,
        now: u64,
    ) -> Result<AsrLedgerRecord, AsrLedgerError> {
        let mut record = self.load(idempotency_key)?;
        record.transition(next, reason, now)?;
        self.save(&record)?;
        Ok(record)
    }

    fn save(&self, record: &AsrLedgerRecord) -> Result<(), AsrLedgerError> {
        validate_key(record.idempotency_key())?;
        std::fs::create_dir_all(&self.root).map_err(AsrLedgerError::io)?;
        let destination = self.path(record.idempotency_key());
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(record)?)
            .map_err(AsrLedgerError::io)?;
        std::fs::rename(&temporary, destination).map_err(AsrLedgerError::io)
    }

    fn existing_decision(
        &self,
        record: AsrLedgerRecord,
    ) -> Result<AsrLedgerDecision, AsrLedgerError> {
        match record.state() {
            AsrLedgerState::Succeeded => {
                self.verify_artifact(&record)?;
                Ok(AsrLedgerDecision::Reuse(record))
            }
            AsrLedgerState::Ambiguous => Err(AsrLedgerError::AmbiguousRequiresReconciliation {
                idempotency_key: record.idempotency_key().to_owned(),
            }),
            AsrLedgerState::Failed => Err(AsrLedgerError::FailedRequiresExplicitRetry {
                idempotency_key: record.idempotency_key().to_owned(),
            }),
            state => Err(AsrLedgerError::DuplicateInFlight {
                idempotency_key: record.idempotency_key().to_owned(),
                state,
            }),
        }
    }

    fn verify_artifact(&self, record: &AsrLedgerRecord) -> Result<(), AsrLedgerError> {
        let path = record
            .artifact_path()
            .ok_or_else(|| AsrLedgerError::ArtifactMissing {
                idempotency_key: record.idempotency_key().to_owned(),
            })?;
        let expected = record
            .artifact_sha256()
            .ok_or_else(|| AsrLedgerError::ArtifactMissing {
                idempotency_key: record.idempotency_key().to_owned(),
            })?;
        let bytes = std::fs::read(path).map_err(|_| AsrLedgerError::ArtifactMissing {
            idempotency_key: record.idempotency_key().to_owned(),
        })?;
        if sha256(&bytes) != expected {
            return Err(AsrLedgerError::ArtifactDrift {
                idempotency_key: record.idempotency_key().to_owned(),
            });
        }
        Ok(())
    }

    fn claim_path(&self, idempotency_key: &str) -> PathBuf {
        self.root.join(format!("{idempotency_key}.claim"))
    }
}

fn validate_key(value: &str) -> Result<(), AsrLedgerError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(AsrLedgerError::InvalidSubmission(
            "idempotency key must be a SHA-256 hex digest".to_owned(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

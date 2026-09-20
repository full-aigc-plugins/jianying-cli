use crate::{
    ApprovalStore, TtsLedgerDecision, TtsLedgerError, TtsLedgerRecord, TtsLedgerState,
    TtsSubmission,
};
use sha2::{Digest, Sha256};
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

/// 以每个幂等键一个 JSON 文件实现的云端 TTS 持久账本。
pub struct TtsLedgerStore {
    root: PathBuf,
}

impl TtsLedgerStore {
    /// 打开账本根目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 在任何网络提交前检查幂等状态并消费精确费用审批。
    pub fn prepare(
        &self,
        submission: &TtsSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<TtsLedgerDecision, TtsLedgerError> {
        if self.path(submission.idempotency_key()).is_file() {
            let record = self.load(submission.idempotency_key())?;
            return self.existing_decision(record);
        }
        std::fs::create_dir_all(&self.root).map_err(TtsLedgerError::io)?;
        let claim_path = self.claim_path(submission.idempotency_key());
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim_path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                if self.path(submission.idempotency_key()).is_file() {
                    return self.existing_decision(self.load(submission.idempotency_key())?);
                }
                return Err(TtsLedgerError::AmbiguousRequiresReconciliation {
                    idempotency_key: submission.idempotency_key().to_owned(),
                });
            }
            Err(error) => return Err(TtsLedgerError::io(error)),
        }
        let result = (|| {
            if self.path(submission.idempotency_key()).is_file() {
                return self.existing_decision(self.load(submission.idempotency_key())?);
            }
            let binding = submission.approval_binding()?;
            let original_approval = approvals.load(approval_id)?;
            approvals.consume(approval_id, &binding, now)?;
            let record = TtsLedgerRecord::queued(submission, approval_id, now);
            if let Err(error) = self.save(&record) {
                let _ = approvals.save(&original_approval);
                return Err(error);
            }
            Ok(TtsLedgerDecision::Submit(record))
        })();
        let _ = std::fs::remove_file(claim_path);
        result
    }

    /// 将已失败请求显式重新排队；必须消费一条新的精确审批。
    pub fn retry_failed(
        &self,
        submission: &TtsSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        std::fs::create_dir_all(&self.root).map_err(TtsLedgerError::io)?;
        let claim_path = self.claim_path(submission.idempotency_key());
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim_path)
        {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                return Err(TtsLedgerError::AmbiguousRequiresReconciliation {
                    idempotency_key: submission.idempotency_key().to_owned(),
                });
            }
            Err(error) => return Err(TtsLedgerError::io(error)),
        }
        let result = (|| {
            let mut record = self.load(submission.idempotency_key())?;
            match record.state() {
                TtsLedgerState::Ambiguous => {
                    return Err(TtsLedgerError::AmbiguousRequiresReconciliation {
                        idempotency_key: record.idempotency_key().to_owned(),
                    });
                }
                TtsLedgerState::Failed => {}
                state => {
                    return Err(TtsLedgerError::DuplicateInFlight {
                        idempotency_key: record.idempotency_key().to_owned(),
                        state,
                    });
                }
            }
            let binding = submission.approval_binding()?;
            let original_approval = approvals.load(approval_id)?;
            approvals.consume(approval_id, &binding, now)?;
            record.transition(TtsLedgerState::Queued, "explicit retry approved", now)?;
            record.replace_approval(approval_id);
            if let Err(error) = self.save(&record) {
                let _ = approvals.save(&original_approval);
                return Err(error);
            }
            Ok(record)
        })();
        let _ = std::fs::remove_file(claim_path);
        result
    }

    /// 标记开始向远端提交；这是 attempts 唯一递增点。
    pub fn mark_running(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        self.transition(
            idempotency_key,
            TtsLedgerState::Running,
            "remote submission started",
            now,
        )
    }

    /// 标记明确失败；普通 prepare 仍不会自动重提。
    pub fn mark_failed(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        self.transition(
            idempotency_key,
            TtsLedgerState::Failed,
            "provider returned a definite failure",
            now,
        )
    }

    /// 将超时或连接中断后的未知结果标记为 ambiguous。
    pub fn mark_ambiguous(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        self.transition(
            idempotency_key,
            TtsLedgerState::Ambiguous,
            "transport outcome is unknown",
            now,
        )
    }

    /// 在远端确认未接受请求后，把 ambiguous 显式对账为 failed。
    pub fn reconcile_ambiguous_as_failed(
        &self,
        idempotency_key: &str,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        self.transition(
            idempotency_key,
            TtsLedgerState::Failed,
            "provider reconciliation confirmed no accepted request",
            now,
        )
    }

    /// 保存成功制品路径、哈希和远端 request-id，供后续安全复用。
    pub fn mark_succeeded(
        &self,
        idempotency_key: &str,
        artifact_path: &Path,
        external_request_id: Option<&str>,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        let bytes = std::fs::read(artifact_path).map_err(TtsLedgerError::io)?;
        if bytes.is_empty() {
            return Err(TtsLedgerError::ArtifactMissing {
                idempotency_key: idempotency_key.to_owned(),
            });
        }
        let artifact_sha256 = sha256(&bytes);
        let mut record = self.load(idempotency_key)?;
        record.transition(TtsLedgerState::Succeeded, "artifact verified", now)?;
        record.attach_artifact(artifact_path, artifact_sha256, external_request_id);
        self.save(&record)?;
        Ok(record)
    }

    /// 加载一条账本记录。
    pub fn load(&self, idempotency_key: &str) -> Result<TtsLedgerRecord, TtsLedgerError> {
        validate_key(idempotency_key)?;
        let path = self.path(idempotency_key);
        if !path.is_file() {
            return Err(TtsLedgerError::NotFound(idempotency_key.to_owned()));
        }
        Ok(serde_json::from_slice(
            &std::fs::read(path).map_err(TtsLedgerError::io)?,
        )?)
    }

    /// 返回指定幂等键的记录路径。
    pub fn path(&self, idempotency_key: &str) -> PathBuf {
        self.root.join(format!("{idempotency_key}.json"))
    }

    fn transition(
        &self,
        idempotency_key: &str,
        next: TtsLedgerState,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<TtsLedgerRecord, TtsLedgerError> {
        let mut record = self.load(idempotency_key)?;
        record.transition(next, reason, now)?;
        self.save(&record)?;
        Ok(record)
    }

    fn save(&self, record: &TtsLedgerRecord) -> Result<(), TtsLedgerError> {
        validate_key(record.idempotency_key())?;
        std::fs::create_dir_all(&self.root).map_err(TtsLedgerError::io)?;
        let destination = self.path(record.idempotency_key());
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(record)?)
            .map_err(TtsLedgerError::io)?;
        std::fs::rename(&temporary, destination).map_err(TtsLedgerError::io)
    }

    fn verify_artifact(&self, record: &TtsLedgerRecord) -> Result<(), TtsLedgerError> {
        let path = record
            .artifact_path()
            .ok_or_else(|| TtsLedgerError::ArtifactMissing {
                idempotency_key: record.idempotency_key().to_owned(),
            })?;
        let expected = record
            .artifact_sha256()
            .ok_or_else(|| TtsLedgerError::ArtifactMissing {
                idempotency_key: record.idempotency_key().to_owned(),
            })?;
        let bytes = std::fs::read(path).map_err(|_| TtsLedgerError::ArtifactMissing {
            idempotency_key: record.idempotency_key().to_owned(),
        })?;
        if sha256(&bytes) != expected {
            return Err(TtsLedgerError::ArtifactDrift {
                idempotency_key: record.idempotency_key().to_owned(),
            });
        }
        Ok(())
    }

    fn existing_decision(
        &self,
        record: TtsLedgerRecord,
    ) -> Result<TtsLedgerDecision, TtsLedgerError> {
        match record.state() {
            TtsLedgerState::Succeeded => {
                self.verify_artifact(&record)?;
                Ok(TtsLedgerDecision::Reuse(record))
            }
            TtsLedgerState::Ambiguous => Err(TtsLedgerError::AmbiguousRequiresReconciliation {
                idempotency_key: record.idempotency_key().to_owned(),
            }),
            TtsLedgerState::Failed => Err(TtsLedgerError::FailedRequiresExplicitRetry {
                idempotency_key: record.idempotency_key().to_owned(),
            }),
            state => Err(TtsLedgerError::DuplicateInFlight {
                idempotency_key: record.idempotency_key().to_owned(),
                state,
            }),
        }
    }

    fn claim_path(&self, idempotency_key: &str) -> PathBuf {
        self.root.join(format!("{idempotency_key}.claim"))
    }
}

fn validate_key(value: &str) -> Result<(), TtsLedgerError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(TtsLedgerError::InvalidSubmission(
            "idempotency key must be a SHA-256 hex digest".to_owned(),
        ));
    }
    Ok(())
}

fn sha256(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

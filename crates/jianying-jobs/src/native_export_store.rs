use crate::{
    ApprovalStore, NativeExportArtifact, NativeExportError, NativeExportRecord, NativeExportState,
    NativeExportSubmission,
};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// 持久保存原生导出授权、进度、终态和制品证据的仓库。
pub struct NativeExportStore {
    root: PathBuf,
}

impl NativeExportStore {
    /// 打开原生导出任务根目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 消费精确审批并创建 queued 任务；这是启动 adapter 前的必经门禁。
    pub fn prepare(
        &self,
        submission: &NativeExportSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        if submission.output().exists() && !submission.overwrite() {
            return Err(NativeExportError::OutputExists);
        }
        if self.path(submission.task_id()).is_file() {
            return Err(NativeExportError::InvalidSubmission(
                "task id already exists".to_owned(),
            ));
        }
        let binding = submission.approval_binding()?;
        let original = approvals.load(approval_id)?;
        approvals.consume(approval_id, &binding, now)?;
        let record = NativeExportRecord::queued(submission, approval_id, now);
        if let Err(error) = self.save(&record) {
            let _ = approvals.save(&original);
            return Err(error);
        }
        Ok(record)
    }

    /// 标记 adapter 已开始真实原生导出。
    pub fn mark_running(
        &self,
        task_id: &str,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        self.transition(
            task_id,
            NativeExportState::Running,
            "native adapter started",
            now,
        )
    }

    /// 持久化 0..99 的单调进度检查点。
    pub fn update_progress(
        &self,
        task_id: &str,
        progress_percent: u8,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        let mut record = self.load(task_id)?;
        record.update_progress(progress_percent, now)?;
        self.save(&record)?;
        Ok(record)
    }

    /// 在 adapter 报告完成后进入结果验证阶段。
    pub fn begin_verification(
        &self,
        task_id: &str,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        self.transition(
            task_id,
            NativeExportState::Verifying,
            "adapter completed; verifying native artifact",
            now,
        )
    }

    /// 保存中断终态并保留最后进度和恢复命令。
    pub fn mark_interrupted(
        &self,
        task_id: &str,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        self.transition(task_id, NativeExportState::Interrupted, reason, now)
    }

    /// 保存明确失败终态。
    pub fn mark_failed(
        &self,
        task_id: &str,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        self.transition(task_id, NativeExportState::Failed, reason, now)
    }

    /// 验证非空输出、计算 SHA-256，并只生成 Native 类型制品。
    pub fn mark_succeeded(
        &self,
        task_id: &str,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        let mut record = self.load(task_id)?;
        if record.state() != NativeExportState::Verifying {
            return Err(NativeExportError::InvalidTransition {
                from: record.state(),
                to: NativeExportState::Succeeded,
            });
        }
        let bytes =
            std::fs::read(record.output()).map_err(|_| NativeExportError::ArtifactMissing {
                task_id: task_id.to_owned(),
            })?;
        if bytes.is_empty() {
            return Err(NativeExportError::ArtifactMissing {
                task_id: task_id.to_owned(),
            });
        }
        let artifact_sha256 = sha256(&bytes);
        if record.output_before_sha256() == Some(artifact_sha256.as_str()) {
            return Err(NativeExportError::ArtifactUnchanged {
                task_id: task_id.to_owned(),
            });
        }
        let artifact = NativeExportArtifact::new(
            record.output().to_path_buf(),
            bytes.len() as u64,
            artifact_sha256,
        );
        record.attach_artifact(artifact);
        record.transition(
            NativeExportState::Succeeded,
            "native artifact verified",
            now,
        )?;
        self.save(&record)?;
        Ok(record)
    }

    /// 重新计算已成功制品哈希，防止代理或篡改文件冒充结果。
    pub fn verify_result(&self, task_id: &str) -> Result<NativeExportArtifact, NativeExportError> {
        let record = self.load(task_id)?;
        let artifact = record
            .artifact()
            .ok_or_else(|| NativeExportError::ArtifactMissing {
                task_id: task_id.to_owned(),
            })?;
        let bytes =
            std::fs::read(artifact.path()).map_err(|_| NativeExportError::ArtifactMissing {
                task_id: task_id.to_owned(),
            })?;
        if bytes.len() as u64 != artifact.byte_length() || sha256(&bytes) != artifact.sha256() {
            return Err(NativeExportError::ArtifactDrift {
                task_id: task_id.to_owned(),
            });
        }
        Ok(artifact.clone())
    }

    /// 对 interrupted/failed 任务消费一条新审批后重新排队。
    pub fn retry(
        &self,
        submission: &NativeExportSubmission,
        approvals: &ApprovalStore,
        approval_id: &str,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        let mut record = self.load(submission.task_id())?;
        if !record.matches_submission(submission) {
            return Err(NativeExportError::InvalidSubmission(
                "retry submission differs from the persisted native export task".to_owned(),
            ));
        }
        if !matches!(
            record.state(),
            NativeExportState::Interrupted | NativeExportState::Failed
        ) {
            return Err(NativeExportError::InvalidTransition {
                from: record.state(),
                to: NativeExportState::Queued,
            });
        }
        let binding = submission.approval_binding()?;
        let original = approvals.load(approval_id)?;
        approvals.consume(approval_id, &binding, now)?;
        record.transition(NativeExportState::Queued, "explicit retry approved", now)?;
        record.replace_approval(approval_id);
        if let Err(error) = self.save(&record) {
            let _ = approvals.save(&original);
            return Err(error);
        }
        Ok(record)
    }

    /// 加载原生导出任务。
    pub fn load(&self, task_id: &str) -> Result<NativeExportRecord, NativeExportError> {
        crate::native_export_submission::validate_task_id(task_id)?;
        let path = self.path(task_id);
        if !path.is_file() {
            return Err(NativeExportError::NotFound(task_id.to_owned()));
        }
        Ok(serde_json::from_slice(
            &std::fs::read(path).map_err(NativeExportError::io)?,
        )?)
    }

    /// 返回任务记录路径。
    pub fn path(&self, task_id: &str) -> PathBuf {
        self.root.join(format!("{task_id}.json"))
    }

    fn transition(
        &self,
        task_id: &str,
        next: NativeExportState,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<NativeExportRecord, NativeExportError> {
        let mut record = self.load(task_id)?;
        record.transition(next, reason, now)?;
        self.save(&record)?;
        Ok(record)
    }

    fn save(&self, record: &NativeExportRecord) -> Result<(), NativeExportError> {
        std::fs::create_dir_all(&self.root).map_err(NativeExportError::io)?;
        let destination = self.path(record.task_id());
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(record)?)
            .map_err(NativeExportError::io)?;
        std::fs::rename(temporary, destination).map_err(NativeExportError::io)
    }
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

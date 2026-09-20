use crate::{ApprovalBinding, ApprovalError, ApprovalRecord};
use std::path::{Path, PathBuf};

/// 以每项审批一个 JSON 文件实现的本地持久化仓库。
pub struct ApprovalStore {
    root: PathBuf,
}

impl ApprovalStore {
    /// 打开指定审批根目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 原子保存审批记录。
    pub fn save(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        ApprovalRecord::validate_id(&record.approval_id)?;
        std::fs::create_dir_all(&self.root).map_err(ApprovalError::io)?;
        let destination = self.path(&record.approval_id)?;
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(record)?)
            .map_err(ApprovalError::io)?;
        std::fs::rename(&temporary, &destination).map_err(ApprovalError::io)?;
        Ok(())
    }

    /// 加载审批记录。
    pub fn load(&self, approval_id: &str) -> Result<ApprovalRecord, ApprovalError> {
        let path = self.path(approval_id)?;
        if !path.is_file() {
            return Err(ApprovalError::NotFound(approval_id.to_owned()));
        }
        Ok(serde_json::from_slice(
            &std::fs::read(path).map_err(ApprovalError::io)?,
        )?)
    }

    /// 列出全部审批记录。
    pub fn list(&self) -> Result<Vec<ApprovalRecord>, ApprovalError> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let mut records: Vec<ApprovalRecord> = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(ApprovalError::io)? {
            let path = entry.map_err(ApprovalError::io)?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                records.push(serde_json::from_slice(
                    &std::fs::read(path).map_err(ApprovalError::io)?,
                )?);
            }
        }
        records.sort_by(|left, right| left.approval_id.cmp(&right.approval_id));
        Ok(records)
    }

    /// 校验并消费完全匹配的审批；成功后同一记录不能再次使用。
    pub fn consume(
        &self,
        approval_id: &str,
        binding: &ApprovalBinding,
        now: u64,
    ) -> Result<ApprovalRecord, ApprovalError> {
        let mut record = self.load(approval_id)?;
        record.consume(binding, now)?;
        self.save(&record)?;
        Ok(record)
    }

    fn path(&self, approval_id: &str) -> Result<PathBuf, ApprovalError> {
        ApprovalRecord::validate_id(approval_id)?;
        Ok(self.root.join(format!("{approval_id}.json")))
    }

    /// 返回审批存储根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }
}

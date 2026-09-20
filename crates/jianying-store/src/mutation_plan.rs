use crate::{MutationEvent, MutationPhase, StoreError};
use std::path::{Path, PathBuf};

/// 以 plan、snapshot、work-copy、validate、atomic-commit 管理一次草稿写入。
pub struct MutationPlan {
    source: PathBuf,
    transaction_root: PathBuf,
    snapshot: PathBuf,
    work_copy: PathBuf,
    audit_file: PathBuf,
    rollback: PathBuf,
    phase: MutationPhase,
    events: Vec<MutationEvent>,
}

impl MutationPlan {
    /// 为现有草稿创建只读事务计划；此步骤不写磁盘。
    pub fn new(source: PathBuf, state_root: PathBuf) -> Result<Self, StoreError> {
        if !source.is_dir() {
            return Err(StoreError::InvalidSource(source.display().to_string()));
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        let transaction_root = state_root.join(&id);
        let parent = source.parent().ok_or_else(|| {
            StoreError::InvalidSource(format!("{} has no parent", source.display()))
        })?;
        let file_name = source
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| StoreError::InvalidSource(source.display().to_string()))?;
        let rollback = parent.join(format!(".{file_name}.jianying-rollback-{id}"));
        let mut plan = Self {
            source,
            snapshot: transaction_root.join("snapshot"),
            work_copy: transaction_root.join("work-copy"),
            audit_file: transaction_root.join("audit.json"),
            transaction_root,
            rollback,
            phase: MutationPhase::Planned,
            events: Vec::new(),
        };
        plan.record("plan", "mutation plan created");
        Ok(plan)
    }

    /// 创建不可变快照和隔离工作副本；源草稿不发生变化。
    pub fn stage(&mut self) -> Result<(), StoreError> {
        self.require_phase(MutationPhase::Planned)?;
        std::fs::create_dir_all(&self.transaction_root)?;
        copy_directory(&self.source, &self.snapshot)?;
        self.phase = MutationPhase::Snapshotted;
        self.record("snapshot", "source draft copied to immutable snapshot");
        copy_directory(&self.source, &self.work_copy)?;
        self.phase = MutationPhase::WorkCopy;
        self.record("work-copy", "isolated work copy is ready");
        self.persist_audit()?;
        Ok(())
    }

    /// 使用调用方提供的完整校验器验证工作副本。
    pub fn validate<F>(&mut self, validator: F) -> Result<(), StoreError>
    where
        F: FnOnce(&Path) -> Result<(), StoreError>,
    {
        self.require_phase(MutationPhase::WorkCopy)?;
        match validator(&self.work_copy) {
            Ok(()) => {
                self.phase = MutationPhase::Validated;
                self.record("validate", "work copy passed validation");
                self.persist_audit()?;
                Ok(())
            }
            Err(error) => {
                self.phase = MutationPhase::Rejected;
                self.record("validate-rejected", &error.to_string());
                self.persist_audit()?;
                Err(error)
            }
        }
    }

    /// 通过同目录 rename 原子替换源草稿；第二次 rename 失败时立即回滚。
    pub fn commit(&mut self) -> Result<(), StoreError> {
        self.require_phase(MutationPhase::Validated)?;
        std::fs::rename(&self.source, &self.rollback).map_err(|error| {
            StoreError::Commit(format!("move source to rollback path: {error}"))
        })?;
        if let Err(error) = std::fs::rename(&self.work_copy, &self.source) {
            let rollback_result = std::fs::rename(&self.rollback, &self.source);
            let message = match rollback_result {
                Ok(()) => format!("activate work copy: {error}; source restored"),
                Err(rollback_error) => format!(
                    "activate work copy: {error}; automatic rollback also failed: {rollback_error}"
                ),
            };
            self.record("atomic-commit-failed", &message);
            self.persist_audit()?;
            return Err(StoreError::Commit(message));
        }
        std::fs::remove_dir_all(&self.rollback)?;
        self.phase = MutationPhase::Committed;
        self.record("atomic-commit", "validated work copy replaced source draft");
        self.persist_audit()?;
        Ok(())
    }

    /// 返回可供编辑的隔离工作副本目录。
    pub fn work_copy(&self) -> &Path {
        &self.work_copy
    }

    /// 返回本次事务最终要替换的草稿目录。
    pub fn source(&self) -> &Path {
        &self.source
    }

    /// 用指定快照完整替换隔离工作副本，源草稿仍保持不变。
    pub fn replace_work_copy_from(&mut self, snapshot: &Path) -> Result<(), StoreError> {
        self.require_phase(MutationPhase::WorkCopy)?;
        if !snapshot.is_dir() {
            return Err(StoreError::InvalidSource(snapshot.display().to_string()));
        }
        std::fs::remove_dir_all(&self.work_copy)?;
        copy_directory(snapshot, &self.work_copy)?;
        self.record(
            "restore-work-copy",
            &format!("work copy replaced from snapshot {}", snapshot.display()),
        );
        self.persist_audit()?;
        Ok(())
    }

    /// 返回提交后仍保留的原始快照目录。
    pub fn snapshot(&self) -> &Path {
        &self.snapshot
    }

    /// 返回追加式审计文件路径。
    pub fn audit_file(&self) -> &Path {
        &self.audit_file
    }

    /// 返回当前事务阶段。
    pub fn phase(&self) -> MutationPhase {
        self.phase
    }

    fn require_phase(&self, expected: MutationPhase) -> Result<(), StoreError> {
        if self.phase != expected {
            return Err(StoreError::InvalidPhase(format!(
                "expected {expected:?}, actual {:?}",
                self.phase
            )));
        }
        Ok(())
    }

    fn record(&mut self, action: &str, message: &str) {
        self.events.push(MutationEvent {
            sequence: self.events.len() as u64 + 1,
            phase: self.phase,
            action: action.to_owned(),
            message: message.to_owned(),
            recovery_command: format!(
                "jianying store restore-snapshot --snapshot '{}' --target '{}'",
                self.snapshot.display(),
                self.source.display()
            ),
        });
    }

    fn persist_audit(&self) -> Result<(), StoreError> {
        std::fs::write(&self.audit_file, serde_json::to_vec_pretty(&self.events)?)?;
        Ok(())
    }
}

fn copy_directory(source: &Path, target: &Path) -> Result<(), StoreError> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

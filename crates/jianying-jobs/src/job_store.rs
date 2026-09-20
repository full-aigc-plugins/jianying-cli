use crate::{JobError, JobRecord};
use std::path::{Path, PathBuf};

/// 以每任务一个 JSON 文件实现的持久化作业仓库。
pub struct JobStore {
    root: PathBuf,
}

impl JobStore {
    /// 打开指定任务根目录；读取操作不会主动创建目录。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 原子保存任务记录。
    pub fn save(&self, record: &JobRecord) -> Result<(), JobError> {
        std::fs::create_dir_all(&self.root).map_err(JobError::io)?;
        let destination = self.path(&record.task_id);
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        std::fs::write(&temporary, serde_json::to_vec_pretty(record)?).map_err(JobError::io)?;
        std::fs::rename(&temporary, &destination).map_err(JobError::io)?;
        Ok(())
    }

    /// 加载任务记录。
    pub fn load(&self, task_id: &str) -> Result<JobRecord, JobError> {
        let path = self.path(task_id);
        if !path.is_file() {
            return Err(JobError::NotFound(task_id.to_owned()));
        }
        Ok(serde_json::from_slice(
            &std::fs::read(path).map_err(JobError::io)?,
        )?)
    }

    /// 列出全部任务记录。
    pub fn list(&self) -> Result<Vec<JobRecord>, JobError> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(JobError::io)? {
            let path = entry.map_err(JobError::io)?.path();
            if path.extension().and_then(|value| value.to_str()) == Some("json") {
                records.push(serde_json::from_slice(
                    &std::fs::read(path).map_err(JobError::io)?,
                )?);
            }
        }
        records.sort_by(|left: &JobRecord, right: &JobRecord| left.task_id.cmp(&right.task_id));
        Ok(records)
    }

    /// 返回状态文件路径。
    pub fn path(&self, task_id: &str) -> PathBuf {
        self.root.join(format!("{task_id}.json"))
    }

    /// 返回任务根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }
}

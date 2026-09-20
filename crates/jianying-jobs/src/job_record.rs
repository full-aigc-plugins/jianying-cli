use crate::{AuditEvent, JobCheckpoint, JobError, JobState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// 可恢复任务的持久化记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    pub task_id: String,
    pub state: JobState,
    pub revision: u64,
    pub job_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub attempts: u64,
    pub last_error: Option<String>,
    pub history: Vec<AuditEvent>,
}

impl JobRecord {
    /// 创建排队状态的任务记录。
    pub fn new(
        task_id: String,
        job_path: PathBuf,
        output_path: Option<PathBuf>,
    ) -> Result<Self, JobError> {
        JobCheckpoint::new(task_id.clone(), JobState::Queued)?;
        let mut record = Self {
            task_id,
            state: JobState::Queued,
            revision: 0,
            job_path,
            output_path,
            attempts: 0,
            last_error: None,
            history: Vec::new(),
        };
        record.record("created");
        Ok(record)
    }

    /// 执行受状态机约束的状态转换。
    pub fn transition(
        &mut self,
        next: JobState,
        reason: impl Into<String>,
    ) -> Result<(), JobError> {
        let current = JobCheckpoint::new(self.task_id.clone(), self.state)?
            .with_revision(self.revision)
            .transition(next)?;
        self.state = current.state();
        self.revision = current.revision();
        if next == JobState::Running {
            self.attempts += 1;
        }
        let reason = reason.into();
        if next == JobState::Failed {
            self.last_error = Some(reason.clone());
        }
        self.record(&reason);
        Ok(())
    }

    fn record(&mut self, reason: &str) {
        self.history.push(AuditEvent {
            sequence: self.history.len() as u64 + 1,
            state: self.state,
            reason: reason.to_owned(),
            epoch_seconds: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
    }
}

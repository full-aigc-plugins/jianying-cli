use crate::{JobError, JobState};
use serde::{Deserialize, Serialize};

/// 支持恢复和审计的不可变作业状态检查点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobCheckpoint {
    task_id: String,
    state: JobState,
    revision: u64,
}

impl JobCheckpoint {
    /// 创建初始检查点。
    pub fn new(task_id: impl Into<String>, state: JobState) -> Result<Self, JobError> {
        let task_id = task_id.into();
        if task_id.trim().is_empty() {
            return Err(JobError::EmptyTaskId);
        }
        Ok(Self {
            task_id,
            state,
            revision: 0,
        })
    }

    /// 按显式状态机生成下一版检查点。
    pub fn transition(&self, next: JobState) -> Result<Self, JobError> {
        let allowed = matches!(
            (self.state, next),
            (JobState::Queued, JobState::Running | JobState::Cancelled)
                | (
                    JobState::Running,
                    JobState::Succeeded | JobState::Failed | JobState::Cancelled
                )
                | (JobState::Failed, JobState::Queued)
        );
        if !allowed {
            return Err(JobError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        Ok(Self {
            task_id: self.task_id.clone(),
            state: next,
            revision: self.revision + 1,
        })
    }

    /// 恢复持久化记录中的 revision。
    pub fn with_revision(mut self, revision: u64) -> Self {
        self.revision = revision;
        self
    }

    /// 返回当前状态。
    pub fn state(&self) -> JobState {
        self.state
    }

    /// 返回当前修订号。
    pub fn revision(&self) -> u64 {
        self.revision
    }
}

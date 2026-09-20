use crate::{
    NativeExportArtifact, NativeExportError, NativeExportEvent, NativeExportState,
    NativeExportSubmission,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 可恢复的原生导出任务、进度和结果记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeExportRecord {
    task_id: String,
    profile_id: String,
    executable_sha256: String,
    draft: PathBuf,
    draft_sha256: String,
    output: PathBuf,
    overwrite: bool,
    output_before_sha256: Option<String>,
    approval_id: String,
    state: NativeExportState,
    progress_percent: u8,
    terminal_reason: Option<String>,
    recovery_argv: Vec<String>,
    artifact: Option<NativeExportArtifact>,
    created_at: u64,
    updated_at: u64,
    history: Vec<NativeExportEvent>,
}

impl NativeExportRecord {
    pub(crate) fn queued(
        submission: &NativeExportSubmission,
        approval_id: impl Into<String>,
        now: u64,
    ) -> Self {
        Self {
            task_id: submission.task_id().to_owned(),
            profile_id: submission.profile_id().to_owned(),
            executable_sha256: submission.executable_sha256().to_owned(),
            draft: submission.draft().to_path_buf(),
            draft_sha256: submission.draft_sha256().to_owned(),
            output: submission.output().to_path_buf(),
            overwrite: submission.overwrite(),
            output_before_sha256: submission.output_before_sha256().map(str::to_owned),
            approval_id: approval_id.into(),
            state: NativeExportState::Queued,
            progress_percent: 0,
            terminal_reason: None,
            recovery_argv: recovery_argv(submission.task_id()),
            artifact: None,
            created_at: now,
            updated_at: now,
            history: vec![NativeExportEvent::new(
                NativeExportState::Queued,
                0,
                "explicit approval consumed",
                now,
            )],
        }
    }

    pub(crate) fn transition(
        &mut self,
        next: NativeExportState,
        reason: impl Into<String>,
        now: u64,
    ) -> Result<(), NativeExportError> {
        let allowed = matches!(
            (self.state, next),
            (NativeExportState::Queued, NativeExportState::Running)
                | (NativeExportState::Running, NativeExportState::Verifying)
                | (NativeExportState::Running, NativeExportState::Failed)
                | (NativeExportState::Running, NativeExportState::Interrupted)
                | (NativeExportState::Verifying, NativeExportState::Succeeded)
                | (NativeExportState::Verifying, NativeExportState::Failed)
                | (NativeExportState::Verifying, NativeExportState::Interrupted)
                | (NativeExportState::Failed, NativeExportState::Queued)
                | (NativeExportState::Interrupted, NativeExportState::Queued)
        );
        if !allowed {
            return Err(NativeExportError::InvalidTransition {
                from: self.state,
                to: next,
            });
        }
        let reason = reason.into();
        self.state = next;
        self.updated_at = now;
        if matches!(
            next,
            NativeExportState::Failed | NativeExportState::Interrupted
        ) {
            self.terminal_reason = Some(reason.clone());
        }
        if next == NativeExportState::Queued {
            self.progress_percent = 0;
            self.terminal_reason = None;
            self.artifact = None;
        }
        if next == NativeExportState::Succeeded {
            self.progress_percent = 100;
            self.terminal_reason = None;
        }
        self.history.push(NativeExportEvent::new(
            next,
            self.progress_percent,
            reason,
            now,
        ));
        Ok(())
    }

    pub(crate) fn update_progress(&mut self, next: u8, now: u64) -> Result<(), NativeExportError> {
        if self.state != NativeExportState::Running {
            return Err(NativeExportError::InvalidTransition {
                from: self.state,
                to: NativeExportState::Running,
            });
        }
        if next > 99 {
            return Err(NativeExportError::InvalidProgress);
        }
        if next < self.progress_percent {
            return Err(NativeExportError::ProgressRegression {
                current: self.progress_percent,
                next,
            });
        }
        self.progress_percent = next;
        self.updated_at = now;
        self.history.push(NativeExportEvent::new(
            self.state,
            next,
            "progress checkpoint",
            now,
        ));
        Ok(())
    }

    pub(crate) fn replace_approval(&mut self, approval_id: impl Into<String>) {
        self.approval_id = approval_id.into();
    }

    pub(crate) fn attach_artifact(&mut self, artifact: NativeExportArtifact) {
        self.artifact = Some(artifact);
    }

    /// 返回任务状态。
    pub fn state(&self) -> NativeExportState {
        self.state
    }

    /// 返回最后持久化的进度百分比。
    pub fn progress_percent(&self) -> u8 {
        self.progress_percent
    }

    /// 返回最近审批 ID。
    pub fn approval_id(&self) -> &str {
        &self.approval_id
    }

    /// 返回安全恢复 argv。
    pub fn recovery_argv(&self) -> &[String] {
        &self.recovery_argv
    }

    /// 返回成功后验证过的原生导出制品。
    pub fn artifact(&self) -> Option<&NativeExportArtifact> {
        self.artifact.as_ref()
    }

    pub(crate) fn task_id(&self) -> &str {
        &self.task_id
    }

    pub(crate) fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn output_before_sha256(&self) -> Option<&str> {
        self.output_before_sha256.as_deref()
    }

    pub(crate) fn matches_submission(&self, submission: &NativeExportSubmission) -> bool {
        self.task_id == submission.task_id()
            && self.profile_id == submission.profile_id()
            && self.executable_sha256 == submission.executable_sha256()
            && self.draft == submission.draft()
            && self.draft_sha256 == submission.draft_sha256()
            && self.output == submission.output()
            && self.overwrite == submission.overwrite()
    }
}

fn recovery_argv(task_id: &str) -> Vec<String> {
    vec![
        "jianying".to_owned(),
        "render".to_owned(),
        "native-task".to_owned(),
        "show".to_owned(),
        task_id.to_owned(),
    ]
}

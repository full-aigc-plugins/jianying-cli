use crate::{ApprovalBinding, ApprovalError, NativeExportError};
use jianying_runtime::{DraftTreeSnapshot, RuntimeProbeReport, RuntimeProfile};
use jianying_schema::{ExportKind, ExportRequest};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// 一次显式请求的原生导出授权上下文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeExportSubmission {
    task_id: String,
    profile_id: String,
    executable_sha256: String,
    draft: PathBuf,
    draft_sha256: String,
    output: PathBuf,
    overwrite: bool,
    output_before_sha256: Option<String>,
    cwd: PathBuf,
}

impl NativeExportSubmission {
    /// 从 Native 导出请求和已通过的 Runtime Probe 创建提交上下文。
    pub fn new(
        request: ExportRequest,
        profile: &RuntimeProfile,
        report: &RuntimeProbeReport,
        draft: PathBuf,
        cwd: PathBuf,
        task_id: impl Into<String>,
    ) -> Result<Self, NativeExportError> {
        if request.kind() != ExportKind::Native {
            return Err(NativeExportError::NotNativeExport);
        }
        if !profile.supports("render.native") {
            return Err(NativeExportError::MissingCapability);
        }
        if !report.supported() || profile.file_identity() != Some(report.executable_identity()) {
            return Err(NativeExportError::RuntimeEvidenceMismatch);
        }
        if !draft.is_dir() || !cwd.is_absolute() || !request.output().is_absolute() {
            return Err(NativeExportError::InvalidSubmission(
                "draft must be a directory and cwd/output must be absolute".to_owned(),
            ));
        }
        let task_id = task_id.into();
        validate_task_id(&task_id)?;
        let snapshot = DraftTreeSnapshot::capture(&draft)
            .map_err(|error| NativeExportError::InvalidSubmission(error.to_string()))?;
        let draft_sha256 = format!("{:x}", Sha256::digest(serde_json::to_vec(&snapshot)?));
        let output_before_sha256 = if request.output().is_file() {
            Some(format!(
                "{:x}",
                Sha256::digest(std::fs::read(request.output()).map_err(NativeExportError::io)?)
            ))
        } else {
            None
        };
        Ok(Self {
            task_id,
            profile_id: profile.id().to_owned(),
            executable_sha256: report.executable_identity().sha256().to_owned(),
            draft,
            draft_sha256,
            output: request.output().clone(),
            overwrite: request.overwrite(),
            output_before_sha256,
            cwd,
        })
    }

    /// 构造绑定 profile、可执行身份、草稿哈希、输出和覆盖策略的审批。
    pub fn approval_binding(&self) -> Result<ApprovalBinding, ApprovalError> {
        ApprovalBinding::new(
            "render.native".to_owned(),
            vec![
                "--profile".to_owned(),
                self.profile_id.clone(),
                "--executable-sha256".to_owned(),
                self.executable_sha256.clone(),
                "--draft-sha256".to_owned(),
                self.draft_sha256.clone(),
                "--output".to_owned(),
                self.output.display().to_string(),
                "--overwrite".to_owned(),
                self.overwrite.to_string(),
            ],
            self.cwd.clone(),
            self.output.clone(),
            self.task_id.clone(),
        )
    }

    /// 返回任务 ID。
    pub fn task_id(&self) -> &str {
        &self.task_id
    }

    /// 返回目标文件。
    pub fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn profile_id(&self) -> &str {
        &self.profile_id
    }

    pub(crate) fn executable_sha256(&self) -> &str {
        &self.executable_sha256
    }

    pub(crate) fn draft(&self) -> &Path {
        &self.draft
    }

    pub(crate) fn draft_sha256(&self) -> &str {
        &self.draft_sha256
    }

    pub(crate) fn overwrite(&self) -> bool {
        self.overwrite
    }

    pub(crate) fn output_before_sha256(&self) -> Option<&str> {
        self.output_before_sha256.as_deref()
    }
}

pub(crate) fn validate_task_id(value: &str) -> Result<(), NativeExportError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(NativeExportError::InvalidSubmission(
            "task id must contain only ASCII letters, digits, '-' or '_'".to_owned(),
        ));
    }
    Ok(())
}

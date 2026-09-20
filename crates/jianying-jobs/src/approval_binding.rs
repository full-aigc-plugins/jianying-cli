use crate::ApprovalError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// 审批所授权的完整调用上下文，任一字段变化都会使审批失效。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalBinding {
    pub command: String,
    pub arguments: Vec<String>,
    pub arguments_sha256: String,
    pub cwd: PathBuf,
    pub target: PathBuf,
    pub task_id: String,
}

impl ApprovalBinding {
    /// 创建并校验精确审批绑定。
    pub fn new(
        command: String,
        arguments: Vec<String>,
        cwd: PathBuf,
        target: PathBuf,
        task_id: String,
    ) -> Result<Self, ApprovalError> {
        if command.trim().is_empty() {
            return Err(ApprovalError::EmptyField("command"));
        }
        if task_id.trim().is_empty() {
            return Err(ApprovalError::EmptyField("task_id"));
        }
        for (field, path) in [("cwd", &cwd), ("target", &target)] {
            if !path.is_absolute() {
                return Err(ApprovalError::RelativePath {
                    field,
                    value: path.display().to_string(),
                });
            }
        }
        let arguments_sha256 = Self::digest_arguments(&arguments);
        Ok(Self {
            command,
            arguments,
            arguments_sha256,
            cwd,
            target,
            task_id,
        })
    }

    fn digest_arguments(arguments: &[String]) -> String {
        let mut digest = Sha256::new();
        for argument in arguments {
            let bytes = argument.as_bytes();
            digest.update((bytes.len() as u64).to_be_bytes());
            digest.update(bytes);
        }
        format!("{:x}", digest.finalize())
    }

    pub(crate) fn is_self_consistent(&self) -> bool {
        self.arguments_sha256 == Self::digest_arguments(&self.arguments)
    }
}

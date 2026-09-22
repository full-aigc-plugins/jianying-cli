use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 视频混合模式资源名称。对应 pyJianYingDraft: BlendMode。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlendMode(String);

impl BlendMode {
    /// 创建非空混合模式；目录成员资格由应用层校验。
    pub fn new(name: impl Into<String>) -> Result<Self, DomainError> {
        let mode = Self(name.into());
        mode.validate()?;
        Ok(mode)
    }

    /// 返回混合模式名称。
    pub fn name(&self) -> &str {
        &self.0
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.0.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "mix_mode",
                reason: "must not be blank".to_owned(),
            });
        }
        Ok(())
    }
}

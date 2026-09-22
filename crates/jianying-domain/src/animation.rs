use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 片段动画资源和可选时长。对应 pyJianYingDraft 的 VideoAnimation 与 TextAnimation。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration_us: Option<i64>,
}

impl Animation {
    /// 创建动画引用；目录成员资格由应用层按片段类型校验。
    pub fn new(name: impl Into<String>, duration_us: Option<i64>) -> Result<Self, DomainError> {
        let animation = Self {
            name: name.into(),
            duration_us,
        };
        animation.validate()?;
        Ok(animation)
    }

    /// 返回动画名称。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 返回显式动画时长。
    pub fn duration_us(&self) -> Option<i64> {
        self.duration_us
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() || self.duration_us.is_some_and(|value| value <= 0) {
            return Err(DomainError::InvalidField {
                field: "animation",
                reason: "name must be non-blank and duration_us must be positive".to_owned(),
            });
        }
        Ok(())
    }
}

use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 视频出场转场资源和可选时长。对应 pyJianYingDraft: TransitionType。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    duration_us: Option<i64>,
}

impl Transition {
    /// 创建转场引用；是否存在后继片段由时间线应用层校验。
    pub fn new(name: impl Into<String>, duration_us: Option<i64>) -> Result<Self, DomainError> {
        let transition = Self {
            name: name.into(),
            duration_us,
        };
        transition.validate()?;
        Ok(transition)
    }

    /// 返回转场名称。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 返回显式转场时长。
    pub fn duration_us(&self) -> Option<i64> {
        self.duration_us
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() || self.duration_us.is_some_and(|value| value <= 0) {
            return Err(DomainError::InvalidField {
                field: "transition",
                reason: "name must be non-blank and duration_us must be positive".to_owned(),
            });
        }
        Ok(())
    }
}

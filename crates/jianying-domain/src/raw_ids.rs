use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 花字或气泡的原始双 ID。对应 pyJianYingDraft 的 effect_id/resource_id 透传。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawIds {
    effect_id: String,
    resource_id: String,
}

impl RawIds {
    /// 创建非空双 ID 引用。
    pub fn new(
        effect_id: impl Into<String>,
        resource_id: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let ids = Self {
            effect_id: effect_id.into(),
            resource_id: resource_id.into(),
        };
        if ids.effect_id.is_empty() || ids.resource_id.is_empty() {
            return Err(DomainError::InvalidField {
                field: "text.raw_ids",
                reason: "both ids must be non-empty".to_owned(),
            });
        }
        Ok(ids)
    }

    /// 返回效果 ID。
    pub fn effect_id(&self) -> &str {
        &self.effect_id
    }
    /// 返回资源 ID。
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }
}

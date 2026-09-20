use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 项目内稳定且非空的片段标识。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SegmentId(String);

impl SegmentId {
    /// 创建片段标识。
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "segment_id",
                reason: "must not be blank".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// 返回标识字符串。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

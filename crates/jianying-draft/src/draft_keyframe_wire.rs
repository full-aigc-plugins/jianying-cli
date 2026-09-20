use crate::{DraftError, DraftKeyframePointWire};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 片段某一属性的关键帧列表。对应 Python: `keyframe.KeyframeList`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftKeyframeWire {
    /// 关键帧列表标识。
    pub id: String,
    /// 剪映属性类型，如 `KFTypePositionX`。
    pub property_type: String,
    /// 排序后的关键帧点。
    pub keyframe_list: Vec<DraftKeyframePointWire>,
    /// 可选素材标识；空字符串表示片段公共关键帧。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_id: Option<String>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl DraftKeyframeWire {
    /// 校验关键帧属性、点和值均可用于资源语义迁移。
    pub fn validate(&self) -> Result<(), DraftError> {
        if self.id.trim().is_empty() {
            return Err(DraftError::InvalidKeyframe(
                "id must not be empty".to_owned(),
            ));
        }
        if self.property_type.trim().is_empty() {
            return Err(DraftError::InvalidKeyframe(
                "property_type must not be empty".to_owned(),
            ));
        }
        if self.keyframe_list.is_empty() {
            return Err(DraftError::InvalidKeyframe(
                "keyframe_list must not be empty".to_owned(),
            ));
        }
        if self
            .keyframe_list
            .iter()
            .any(|point| point.values.is_empty())
        {
            return Err(DraftError::InvalidKeyframe(
                "keyframe point values must not be empty".to_owned(),
            ));
        }
        Ok(())
    }
}

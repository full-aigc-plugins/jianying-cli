use crate::DraftSegmentWire;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 剪映时间线轨道 wire 模型。对应 Python: `track.Track`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftTrackWire {
    /// 轨道属性位。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attribute: Option<i64>,
    /// 轨道标志位。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flag: Option<i64>,
    /// 轨道标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 是否使用剪映默认轨道名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_default_name: Option<bool>,
    /// 轨道显示名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 轨道 wire 类型，如 video、audio、text。
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub track_type: Option<String>,
    /// 轨道片段；缺失与空数组按原始形状分别保留。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub segments: Option<Vec<DraftSegmentWire>>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl DraftTrackWire {
    /// 返回用于跨实现差分的稳定轨道及片段字段。
    pub fn normalized_core(&self) -> Value {
        let mut normalized = Map::new();
        if let Some(attribute) = self.attribute {
            normalized.insert("attribute".to_owned(), serde_json::json!(attribute));
        }
        if let Some(flag) = self.flag {
            normalized.insert("flag".to_owned(), serde_json::json!(flag));
        }
        if let Some(is_default_name) = self.is_default_name {
            normalized.insert(
                "is_default_name".to_owned(),
                serde_json::json!(is_default_name),
            );
        }
        if let Some(name) = &self.name {
            normalized.insert("name".to_owned(), serde_json::json!(name));
        }
        if let Some(track_type) = &self.track_type {
            normalized.insert("type".to_owned(), serde_json::json!(track_type));
        }
        if let Some(segments) = &self.segments {
            let include_source_timerange =
                matches!(self.track_type.as_deref(), Some("video") | Some("audio"));
            normalized.insert(
                "segments".to_owned(),
                Value::Array(
                    segments
                        .iter()
                        .map(|segment| segment.normalized_core(include_source_timerange))
                        .collect(),
                ),
            );
        }
        Value::Object(normalized)
    }
}

use crate::{DraftKeyframeWire, TimeRangeWire};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 剪映轨道中的通用片段 wire 模型。对应 Python: `segment.BaseSegment`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftSegmentWire {
    /// 片段标识；旧草稿或局部 fixture 可缺失。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 主素材标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_id: Option<String>,
    /// 片段在时间线上的目标范围。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_timerange: Option<TimeRangeWire>,
    /// 片段从源素材截取的范围；文字等可见片段可能为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_timerange: Option<TimeRangeWire>,
    /// 播放速度。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    /// 音量倍率。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volume: Option<f64>,
    /// 视觉变换配置，保留原始 JSON 结构以等待分项迁移。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clip: Option<Value>,
    /// 是否采用统一缩放及其值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uniform_scale: Option<Value>,
    /// 片段引用的附加素材标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra_material_refs: Option<Vec<String>>,
    /// 片段公共关键帧资源。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub common_keyframes: Option<Vec<DraftKeyframeWire>>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl DraftSegmentWire {
    /// 返回用于差分的稳定片段字段；调用方决定是否保留源时间范围。
    pub fn normalized_core(&self, include_source_timerange: bool) -> Value {
        let mut normalized = Map::new();
        if let Some(target) = &self.target_timerange {
            normalized.insert("target_timerange".to_owned(), target.normalized_core());
        }
        if include_source_timerange {
            if let Some(source) = &self.source_timerange {
                normalized.insert("source_timerange".to_owned(), source.normalized_core());
            }
        }
        if let Some(speed) = self.speed {
            normalized.insert("speed".to_owned(), serde_json::json!(speed));
        }
        if let Some(volume) = self.volume {
            normalized.insert("volume".to_owned(), serde_json::json!(volume));
        }
        if let Some(clip) = &self.clip {
            normalized.insert("clip".to_owned(), clip.clone());
        }
        if let Some(uniform_scale) = &self.uniform_scale {
            normalized.insert("uniform_scale".to_owned(), uniform_scale.clone());
        }
        Value::Object(normalized)
    }
}

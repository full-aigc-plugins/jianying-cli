use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 剪映 wire 协议中的单个关键帧点。对应 Python: `keyframe.Keyframe`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftKeyframePointWire {
    /// 关键帧点标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 插值曲线类型。
    #[serde(default, rename = "curveType", skip_serializing_if = "Option::is_none")]
    pub curve_type: Option<String>,
    /// 相对片段起点的微秒偏移。
    pub time_offset: i64,
    /// 属性值数组；剪映当前常用单元素数组。
    pub values: Vec<f64>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

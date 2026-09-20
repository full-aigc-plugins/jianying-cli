use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 剪映 wire 协议中的微秒时间范围。对应 Python: `time_util.Timerange`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRangeWire {
    /// 起始时间，单位为微秒。
    pub start: i64,
    /// 持续时间，单位为微秒。
    pub duration: i64,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl TimeRangeWire {
    /// 返回用于跨实现差分的稳定核心字段，不包含未知扩展字段。
    pub fn normalized_core(&self) -> Value {
        serde_json::json!({
            "start": self.start,
            "duration": self.duration,
        })
    }
}

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 剪映时间线的画布配置。对应 Python: `ScriptFile.width/height` 与 `canvas_config`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasWire {
    /// 画布宽度，单位为像素。
    pub width: u64,
    /// 画布高度，单位为像素。
    pub height: u64,
    /// 剪映画布比例标识。
    pub ratio: String,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl CanvasWire {
    /// 返回用于跨实现差分的稳定画布字段。
    pub fn normalized_core(&self) -> Value {
        serde_json::json!({
            "width": self.width,
            "height": self.height,
            "ratio": self.ratio,
        })
    }
}

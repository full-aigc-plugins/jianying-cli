use crate::{
    CanvasWire, DraftEditSemantics, DraftError, DraftReferenceIntegrity, DraftResourceInventory,
    DraftTrackWire,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `draft_content.json` 的无损时间线 wire 模型。对应 Python: `script_file.ScriptFile.content`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftTimelineWire {
    /// 草稿标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 草稿名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 时间线总时长，单位为微秒。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<i64>,
    /// 时间线帧率。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
    /// 画布配置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_config: Option<CanvasWire>,
    /// 轨道列表。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tracks: Option<Vec<DraftTrackWire>>,
    /// 素材资源分桶；资源对象将在后续协议任务中逐项强类型化。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub materials: Option<Value>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl DraftTimelineWire {
    /// 从任意 JSON 值解析时间线，并保留所有未知字段。
    pub fn from_value(value: Value) -> Result<Self, DraftError> {
        Ok(serde_json::from_value(value)?)
    }

    /// 序列化为完整 JSON，包含解析时保留的未知字段。
    pub fn to_value(&self) -> Result<Value, DraftError> {
        Ok(serde_json::to_value(self)?)
    }

    /// 返回跨 Python/Rust 比较使用的非易变核心字段。
    pub fn normalized_core(&self) -> Value {
        let mut normalized = Map::new();
        if let Some(duration) = self.duration {
            normalized.insert("duration".to_owned(), serde_json::json!(duration));
        }
        if let Some(fps) = self.fps {
            normalized.insert("fps".to_owned(), serde_json::json!(fps));
        }
        if let Some(canvas) = &self.canvas_config {
            normalized.insert("canvas_config".to_owned(), canvas.normalized_core());
        }
        if let Some(tracks) = &self.tracks {
            normalized.insert(
                "tracks".to_owned(),
                Value::Array(tracks.iter().map(DraftTrackWire::normalized_core).collect()),
            );
        }
        Value::Object(normalized)
    }

    /// 构建视频、音频、文字、贴纸、效果和关键帧等统一资源 inventory。
    pub fn resource_inventory(&self) -> Result<DraftResourceInventory, DraftError> {
        DraftResourceInventory::from_timeline(self)
    }

    /// 校验素材 ID 唯一性以及片段主素材、附加素材引用闭包。
    pub fn validate_references(&self) -> Result<(), DraftError> {
        DraftReferenceIntegrity::validate(self)
    }

    /// 校验字幕、样式、速度、音量、裁剪、变换和组合编辑语义。
    pub fn validate_edit_semantics(&self) -> Result<(), DraftError> {
        DraftEditSemantics::validate(self)
    }
}

use crate::DraftError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// `draft_meta_info.json` 的无损元数据 wire 模型。对应 Python: `assets/draft_meta_info.json`。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DraftMetadataWire {
    /// 草稿标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_id: Option<String>,
    /// 草稿名称。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_name: Option<String>,
    /// 草稿文件夹绝对路径。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_fold_path: Option<String>,
    /// 草稿库根路径。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_root_path: Option<String>,
    /// 时间线 JSON 文件路径。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_json_file: Option<String>,
    /// 草稿素材镜像信息。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub draft_materials: Option<Value>,
    /// 草稿总时长，单位为微秒。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm_duration: Option<i64>,
    /// 创建时间，单位沿用剪映原始 wire 值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm_draft_create: Option<i64>,
    /// 修改时间，单位沿用剪映原始 wire 值。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tm_draft_modified: Option<i64>,
    /// 当前 Rust 版本尚未识别但必须无损写回的字段。
    #[serde(flatten)]
    pub unknown_fields: Map<String, Value>,
}

impl DraftMetadataWire {
    /// 从任意 JSON 值解析草稿元数据，并保留所有未知字段。
    pub fn from_value(value: Value) -> Result<Self, DraftError> {
        Ok(serde_json::from_value(value)?)
    }

    /// 序列化为完整 JSON，包含解析时保留的未知字段。
    pub fn to_value(&self) -> Result<Value, DraftError> {
        Ok(serde_json::to_value(self)?)
    }
}

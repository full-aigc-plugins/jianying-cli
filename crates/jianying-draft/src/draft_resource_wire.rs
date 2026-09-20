use crate::DraftResourceKind;
use serde_json::Value;

/// 从具体素材分桶或片段内联节点投影出的统一资源视图。
#[derive(Debug, Clone, PartialEq)]
pub struct DraftResourceWire {
    pub(crate) id: String,
    pub(crate) kind: DraftResourceKind,
    pub(crate) bucket: String,
    pub(crate) wire_type: Option<String>,
    pub(crate) raw: Value,
}

impl DraftResourceWire {
    /// 返回资源标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回统一资源类别。
    pub fn kind(&self) -> DraftResourceKind {
        self.kind
    }

    /// 返回资源所在的原始素材分桶。
    pub fn bucket(&self) -> &str {
        &self.bucket
    }

    /// 返回原始 wire 类型或关键帧属性类型。
    pub fn wire_type(&self) -> Option<&str> {
        self.wire_type.as_deref()
    }

    /// 返回完整原始资源 JSON，供后续资源专用模型继续迁移。
    pub fn raw(&self) -> &Value {
        &self.raw
    }
}

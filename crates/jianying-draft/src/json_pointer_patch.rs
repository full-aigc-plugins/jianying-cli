use serde_json::Value;

/// 对已有 JSON Pointer 目标执行替换的声明式补丁。
#[derive(Debug, Clone, PartialEq)]
pub struct JsonPointerPatch {
    pub(crate) pointer: String,
    pub(crate) value: Value,
}

impl JsonPointerPatch {
    /// 创建替换补丁；指针存在性由 envelope 应用时校验。
    pub fn replace(pointer: impl Into<String>, value: Value) -> Self {
        Self {
            pointer: pointer.into(),
            value,
        }
    }
}

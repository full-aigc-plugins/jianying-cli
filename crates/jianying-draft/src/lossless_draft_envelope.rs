use crate::{DraftError, DraftWireView, JsonPointerPatch};
use serde_json::Value;

/// 保留原始 JSON 全量字段、仅允许声明式定点修改的草稿信封。
#[derive(Debug, Clone, PartialEq)]
pub struct LosslessDraftEnvelope {
    original: Value,
    working: Value,
    changed_pointers: Vec<String>,
}

impl LosslessDraftEnvelope {
    /// 从原始草稿构建信封；数组或标量根节点会被拒绝。
    pub fn from_value(value: Value) -> Result<Self, DraftError> {
        if !value.is_object() {
            return Err(DraftError::RootMustBeObject);
        }
        Ok(Self {
            original: value.clone(),
            working: value,
            changed_pointers: Vec::new(),
        })
    }

    /// 返回不会重建原始 JSON 的只读类型化视图。
    pub fn typed_view(&self) -> DraftWireView {
        DraftWireView::from_value(&self.working)
    }

    /// 将补丁应用到已存在的非根路径；失败时不改变信封。
    pub fn apply(&mut self, patch: JsonPointerPatch) -> Result<(), DraftError> {
        if patch.pointer.is_empty() || !patch.pointer.starts_with('/') {
            return Err(DraftError::InvalidPointer);
        }
        let target = self
            .working
            .pointer_mut(&patch.pointer)
            .ok_or(DraftError::InvalidPointer)?;
        *target = patch.value;
        if !self.changed_pointers.contains(&patch.pointer) {
            self.changed_pointers.push(patch.pointer);
        }
        Ok(())
    }

    /// 验证原始值与工作值的所有差异均落在已声明补丁路径内。
    pub fn verify_only_declared_changes(&self) -> Result<(), DraftError> {
        let mut differences = Vec::new();
        collect_differences(&self.original, &self.working, "", &mut differences);
        for pointer in differences {
            let declared = self
                .changed_pointers
                .iter()
                .any(|allowed| pointer == *allowed || pointer.starts_with(&format!("{allowed}/")));
            if !declared {
                return Err(DraftError::UndeclaredChange(pointer));
            }
        }
        Ok(())
    }

    /// 返回当前完整原始形状的 JSON 值。
    pub fn value(&self) -> &Value {
        &self.working
    }

    /// 返回已成功应用的补丁路径。
    pub fn changed_pointers(&self) -> &[String] {
        &self.changed_pointers
    }
}

fn collect_differences(left: &Value, right: &Value, pointer: &str, output: &mut Vec<String>) {
    if left == right {
        return;
    }
    match (left, right) {
        (Value::Object(left_map), Value::Object(right_map)) => {
            let mut keys: Vec<&String> = left_map.keys().chain(right_map.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                let child = format!("{pointer}/{}", escape_pointer_token(key));
                match (left_map.get(key), right_map.get(key)) {
                    (Some(left_value), Some(right_value)) => {
                        collect_differences(left_value, right_value, &child, output);
                    }
                    _ => output.push(child),
                }
            }
        }
        (Value::Array(left_values), Value::Array(right_values)) => {
            let length = left_values.len().max(right_values.len());
            for index in 0..length {
                let child = format!("{pointer}/{index}");
                match (left_values.get(index), right_values.get(index)) {
                    (Some(left_value), Some(right_value)) => {
                        collect_differences(left_value, right_value, &child, output);
                    }
                    _ => output.push(child),
                }
            }
        }
        _ => output.push(pointer.to_owned()),
    }
}

fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

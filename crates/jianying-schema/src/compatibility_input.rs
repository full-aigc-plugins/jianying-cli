use crate::SchemaError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 为旧输入保留完整语义的版本化兼容载荷。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityInput {
    schema: String,
    payload: Value,
}

impl CompatibilityInput {
    /// 创建冻结的 plan v1 兼容载荷。
    pub fn plan_v1(payload: Value) -> Result<Self, SchemaError> {
        if !payload.is_object() {
            return Err(SchemaError::InvalidField(
                "compatibility payload must be an object",
            ));
        }
        Ok(Self {
            schema: "jianying-cli-plan/v1".to_owned(),
            payload,
        })
    }

    /// 创建固定 capcut-cli 声明式 compile v1 兼容载荷。
    pub fn compile_v1(payload: Value) -> Result<Self, SchemaError> {
        if !payload.is_object() {
            return Err(SchemaError::InvalidField(
                "compatibility payload must be an object",
            ));
        }
        Ok(Self {
            schema: "capcut-cli-compile/v1".to_owned(),
            payload,
        })
    }

    /// 返回兼容输入 schema。
    pub fn schema(&self) -> &str {
        &self.schema
    }

    /// 返回未经删减的规范化旧输入。
    pub fn payload(&self) -> &Value {
        &self.payload
    }

    pub(crate) fn validate(&self) -> Result<(), SchemaError> {
        if !matches!(
            self.schema.as_str(),
            "jianying-cli-plan/v1" | "capcut-cli-compile/v1"
        ) {
            return Err(SchemaError::InvalidCompatibilitySchema {
                actual: self.schema.clone(),
            });
        }
        if !self.payload.is_object() {
            return Err(SchemaError::InvalidField(
                "compatibility payload must be an object",
            ));
        }
        Ok(())
    }
}

use crate::ConfigError;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// 单个隔离 profile 的版本化配置文档。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigDocument {
    pub schema: String,
    pub profile: String,
    pub values: Map<String, Value>,
}

impl ConfigDocument {
    /// 创建空的 v1 配置文档。
    pub fn new(profile: impl Into<String>) -> Self {
        Self {
            schema: "jianying-config/v1".to_owned(),
            profile: profile.into(),
            values: Map::new(),
        }
    }

    /// 验证 schema、profile 和 values 顶层类型约束。
    pub fn validate_for(&self, expected_profile: &str) -> Result<(), ConfigError> {
        if self.schema != "jianying-config/v1" {
            return Err(ConfigError::InvalidSchema(self.schema.clone()));
        }
        if self.profile != expected_profile {
            return Err(ConfigError::ProfileMismatch {
                expected: expected_profile.to_owned(),
                actual: self.profile.clone(),
            });
        }
        Ok(())
    }
}

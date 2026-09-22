use crate::DomainError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 单个音频效果及其命名参数。对应 pyJianYingDraft 的音频场景与音色效果。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioEffect {
    name: String,
    #[serde(default)]
    params: BTreeMap<String, f64>,
}

impl AudioEffect {
    /// 创建音频效果；资源目录成员资格由应用层校验。
    pub fn new(
        name: impl Into<String>,
        params: BTreeMap<String, f64>,
    ) -> Result<Self, DomainError> {
        let effect = Self {
            name: name.into(),
            params,
        };
        effect.validate()?;
        Ok(effect)
    }

    /// 返回效果名称。
    pub fn name(&self) -> &str {
        &self.name
    }

    /// 返回效果参数。
    pub fn params(&self) -> &BTreeMap<String, f64> {
        &self.params
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty()
            || self
                .params
                .values()
                .any(|value| !value.is_finite() || !(0.0..=100.0).contains(value))
        {
            return Err(DomainError::InvalidField {
                field: "audio_effect",
                reason: "name must be non-blank and parameters must be within 0..100".to_owned(),
            });
        }
        Ok(())
    }
}

use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 视频色度抠图设置。对应 pyJianYingDraft 的 chroma_key 素材参数。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChromaKey {
    color: String,
    #[serde(default)]
    intensity: f64,
    #[serde(default)]
    shadow: f64,
    #[serde(default)]
    edge_smooth: f64,
    #[serde(default)]
    spill: f64,
}

impl ChromaKey {
    /// 创建并校验色度抠图颜色和 0..100 参数。
    pub fn new(
        color: impl Into<String>,
        intensity: f64,
        shadow: f64,
        edge_smooth: f64,
        spill: f64,
    ) -> Result<Self, DomainError> {
        let chroma = Self {
            color: color.into(),
            intensity,
            shadow,
            edge_smooth,
            spill,
        };
        chroma.validate()?;
        Ok(chroma)
    }

    /// 返回抠图颜色。
    pub fn color(&self) -> &str {
        &self.color
    }
    /// 返回抠图强度。
    pub fn intensity(&self) -> f64 {
        self.intensity
    }
    /// 返回阴影参数。
    pub fn shadow(&self) -> f64 {
        self.shadow
    }
    /// 返回边缘平滑参数。
    pub fn edge_smooth(&self) -> f64 {
        self.edge_smooth
    }
    /// 返回溢色抑制参数。
    pub fn spill(&self) -> f64 {
        self.spill
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if !valid_color(&self.color)
            || [self.intensity, self.shadow, self.edge_smooth, self.spill]
                .iter()
                .any(|value| !value.is_finite() || !(0.0..=100.0).contains(value))
        {
            return Err(DomainError::InvalidField {
                field: "chroma",
                reason: "color must be RGB/RGBA hex and parameters must be within 0..100"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

pub(crate) fn valid_color(value: &str) -> bool {
    let value = value.trim().trim_start_matches('#');
    matches!(value.len(), 6 | 8) && value.chars().all(|character| character.is_ascii_hexdigit())
}

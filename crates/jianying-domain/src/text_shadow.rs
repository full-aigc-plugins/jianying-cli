use crate::{chroma_key::valid_color, DomainError};
use serde::{Deserialize, Serialize};

/// 文字阴影样式。对应 pyJianYingDraft 的 TextShadow。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextShadow {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    angle: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    distance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    diffuse: Option<f64>,
}

impl TextShadow {
    /// 创建文字阴影。
    pub fn new(
        color: Option<String>,
        alpha: Option<f64>,
        angle: Option<f64>,
        distance: Option<f64>,
        diffuse: Option<f64>,
    ) -> Result<Self, DomainError> {
        let shadow = Self {
            color,
            alpha,
            angle,
            distance,
            diffuse,
        };
        shadow.validate()?;
        Ok(shadow)
    }

    /// 返回阴影颜色。
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
    /// 返回阴影透明度。
    pub fn alpha(&self) -> Option<f64> {
        self.alpha
    }
    /// 返回阴影角度。
    pub fn angle(&self) -> Option<f64> {
        self.angle
    }
    /// 返回阴影距离。
    pub fn distance(&self) -> Option<f64> {
        self.distance
    }
    /// 返回阴影扩散。
    pub fn diffuse(&self) -> Option<f64> {
        self.diffuse
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.color.as_deref().is_some_and(|value| !valid_rgb(value))
            || self
                .alpha
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || self
                .angle
                .is_some_and(|value| !value.is_finite() || !(-180.0..=180.0).contains(&value))
            || self
                .distance
                .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
            || self
                .diffuse
                .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
        {
            return Err(DomainError::InvalidField {
                field: "text.shadow",
                reason: "invalid shadow value".to_owned(),
            });
        }
        Ok(())
    }
}

fn valid_rgb(value: &str) -> bool {
    valid_color(value) && value.trim().trim_start_matches('#').len() == 6
}

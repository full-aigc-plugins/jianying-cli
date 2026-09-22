use crate::{chroma_key::valid_color, DomainError};
use serde::{Deserialize, Serialize};

/// 视频画布背景填充。对应 pyJianYingDraft 的 CanvasColor 与 CanvasBlur。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundFilling {
    #[serde(rename = "type")]
    fill_type: String,
    #[serde(default)]
    blur: f64,
    #[serde(default)]
    color: String,
}

impl BackgroundFilling {
    /// 创建模糊或纯色背景填充。
    pub fn new(
        fill_type: impl Into<String>,
        blur: f64,
        color: impl Into<String>,
    ) -> Result<Self, DomainError> {
        let filling = Self {
            fill_type: fill_type.into(),
            blur,
            color: color.into(),
        };
        filling.validate()?;
        Ok(filling)
    }

    /// 返回填充类型。
    pub fn fill_type(&self) -> &str {
        &self.fill_type
    }
    /// 返回模糊强度。
    pub fn blur(&self) -> f64 {
        self.blur
    }
    /// 返回背景颜色。
    pub fn color(&self) -> &str {
        &self.color
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if !matches!(self.fill_type.as_str(), "blur" | "color")
            || !self.blur.is_finite()
            || !(0.0..=1.0).contains(&self.blur)
            || (!self.color.is_empty() && !valid_color(&self.color))
        {
            return Err(DomainError::InvalidField {
                field: "background_filling",
                reason: "type must be blur/color, blur 0..1, and color valid RGB/RGBA hex"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 片段静态视觉变换。对应 pyJianYingDraft 的缩放、位置、旋转与透明度语义。
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transform {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scale: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rotation: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    opacity: Option<f64>,
}

impl Transform {
    /// 创建并校验静态视觉变换。
    pub fn new(
        scale: Option<f64>,
        x: Option<f64>,
        y: Option<f64>,
        rotation: Option<f64>,
        opacity: Option<f64>,
    ) -> Result<Self, DomainError> {
        let transform = Self {
            scale,
            x,
            y,
            rotation,
            opacity,
        };
        transform.validate()?;
        Ok(transform)
    }

    /// 返回缩放倍数。
    pub fn scale(&self) -> Option<f64> {
        self.scale
    }

    /// 返回归一化横向位置。
    pub fn x(&self) -> Option<f64> {
        self.x
    }

    /// 返回归一化纵向位置。
    pub fn y(&self) -> Option<f64> {
        self.y
    }

    /// 返回旋转角度。
    pub fn rotation(&self) -> Option<f64> {
        self.rotation
    }

    /// 返回透明度。
    pub fn opacity(&self) -> Option<f64> {
        self.opacity
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        validate_optional(self.scale, 0.01, 10.0, "scale")?;
        validate_optional(self.x, -2.0, 2.0, "x")?;
        validate_optional(self.y, -2.0, 2.0, "y")?;
        validate_optional(self.rotation, -360.0, 360.0, "rotation")?;
        validate_optional(self.opacity, 0.0, 1.0, "opacity")?;
        Ok(())
    }
}

fn validate_optional(
    value: Option<f64>,
    minimum: f64,
    maximum: f64,
    field: &'static str,
) -> Result<(), DomainError> {
    if value.is_some_and(|number| !number.is_finite() || !(minimum..=maximum).contains(&number)) {
        return Err(DomainError::InvalidField {
            field,
            reason: format!("must be finite and within {minimum}..{maximum}"),
        });
    }
    Ok(())
}

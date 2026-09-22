use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 视频蒙版的资源名称与几何参数。对应 pyJianYingDraft: MaskType 和 Mask。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    name: String,
    #[serde(default)]
    center_x: f64,
    #[serde(default)]
    center_y: f64,
    #[serde(default)]
    size: f64,
    #[serde(default)]
    rotation: f64,
    #[serde(default)]
    feather: f64,
    #[serde(default)]
    invert: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    rect_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    round_corner: Option<f64>,
}

impl Mask {
    /// 创建并校验蒙版几何值；资源目录名称在应用层校验。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        name: impl Into<String>,
        center_x: f64,
        center_y: f64,
        size: f64,
        rotation: f64,
        feather: f64,
        invert: bool,
        rect_width: Option<f64>,
        round_corner: Option<f64>,
    ) -> Result<Self, DomainError> {
        let mask = Self {
            name: name.into(),
            center_x,
            center_y,
            size,
            rotation,
            feather,
            invert,
            rect_width,
            round_corner,
        };
        mask.validate()?;
        Ok(mask)
    }

    /// 返回蒙版资源名称。
    pub fn name(&self) -> &str {
        &self.name
    }
    /// 返回横向中心。
    pub fn center_x(&self) -> f64 {
        self.center_x
    }
    /// 返回纵向中心。
    pub fn center_y(&self) -> f64 {
        self.center_y
    }
    /// 返回蒙版尺寸。
    pub fn size(&self) -> f64 {
        self.size
    }
    /// 返回旋转角度。
    pub fn rotation(&self) -> f64 {
        self.rotation
    }
    /// 返回羽化值。
    pub fn feather(&self) -> f64 {
        self.feather
    }
    /// 返回是否反转蒙版。
    pub fn invert(&self) -> bool {
        self.invert
    }
    /// 返回矩形宽度。
    pub fn rect_width(&self) -> Option<f64> {
        self.rect_width
    }
    /// 返回矩形圆角。
    pub fn round_corner(&self) -> Option<f64> {
        self.round_corner
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        let finite = [
            self.center_x,
            self.center_y,
            self.size,
            self.rotation,
            self.feather,
        ]
        .iter()
        .all(|value| value.is_finite());
        let optional_finite = [self.rect_width, self.round_corner]
            .iter()
            .flatten()
            .all(|value| value.is_finite());
        let size_ok = self.size == 0.0 || (0.01..=1.0).contains(&self.size);
        let rectangle_ok =
            (self.rect_width.is_none() && self.round_corner.is_none()) || self.name == "矩形";
        if self.name.trim().is_empty()
            || !finite
            || !optional_finite
            || !size_ok
            || !(-360.0..=360.0).contains(&self.rotation)
            || !(0.0..=100.0).contains(&self.feather)
            || self
                .rect_width
                .is_some_and(|value| !(0.01..=1.0).contains(&value))
            || self
                .round_corner
                .is_some_and(|value| !(0.0..=100.0).contains(&value))
            || !rectangle_ok
        {
            return Err(DomainError::InvalidField {
                field: "mask",
                reason: "invalid name, geometry, feather, or rectangle-only settings".to_owned(),
            });
        }
        Ok(())
    }
}

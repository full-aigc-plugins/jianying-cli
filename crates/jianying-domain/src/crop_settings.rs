use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 归一化八点裁剪区域。对应 pyJianYingDraft: CropSettings。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CropSettings {
    upper_left_x: f64,
    upper_left_y: f64,
    upper_right_x: f64,
    upper_right_y: f64,
    lower_left_x: f64,
    lower_left_y: f64,
    lower_right_x: f64,
    lower_right_y: f64,
}

impl CropSettings {
    /// 按左上、右上、左下、右下顺序创建八点裁剪区域。
    pub fn new(points: [f64; 8]) -> Result<Self, DomainError> {
        let settings = Self {
            upper_left_x: points[0],
            upper_left_y: points[1],
            upper_right_x: points[2],
            upper_right_y: points[3],
            lower_left_x: points[4],
            lower_left_y: points[5],
            lower_right_x: points[6],
            lower_right_y: points[7],
        };
        settings.validate()?;
        Ok(settings)
    }

    /// 返回左上角横坐标。
    pub fn upper_left_x(&self) -> f64 {
        self.upper_left_x
    }

    /// 返回左上角纵坐标。
    pub fn upper_left_y(&self) -> f64 {
        self.upper_left_y
    }

    /// 返回右上角横坐标。
    pub fn upper_right_x(&self) -> f64 {
        self.upper_right_x
    }

    /// 返回右上角纵坐标。
    pub fn upper_right_y(&self) -> f64 {
        self.upper_right_y
    }

    /// 返回左下角横坐标。
    pub fn lower_left_x(&self) -> f64 {
        self.lower_left_x
    }

    /// 返回左下角纵坐标。
    pub fn lower_left_y(&self) -> f64 {
        self.lower_left_y
    }

    /// 返回右下角横坐标。
    pub fn lower_right_x(&self) -> f64 {
        self.lower_right_x
    }

    /// 返回右下角纵坐标。
    pub fn lower_right_y(&self) -> f64 {
        self.lower_right_y
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        let points = [
            self.upper_left_x,
            self.upper_left_y,
            self.upper_right_x,
            self.upper_right_y,
            self.lower_left_x,
            self.lower_left_y,
            self.lower_right_x,
            self.lower_right_y,
        ];
        if points
            .iter()
            .any(|value| !value.is_finite() || !(0.0..=1.0).contains(value))
        {
            return Err(DomainError::InvalidField {
                field: "crop",
                reason: "all crop coordinates must be finite and within 0..1".to_owned(),
            });
        }
        Ok(())
    }
}

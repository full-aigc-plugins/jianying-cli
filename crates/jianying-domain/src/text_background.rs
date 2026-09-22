use crate::{chroma_key::valid_color, DomainError};
use serde::{Deserialize, Serialize};

/// 文字背景样式。对应 pyJianYingDraft 的 TextBackground。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextBackground {
    color: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    style: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alpha: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    round_radius: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    horizontal_offset: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vertical_offset: Option<f64>,
}

impl TextBackground {
    /// 创建文字背景。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        color: impl Into<String>,
        style: Option<u8>,
        alpha: Option<f64>,
        round_radius: Option<f64>,
        height: Option<f64>,
        width: Option<f64>,
        horizontal_offset: Option<f64>,
        vertical_offset: Option<f64>,
    ) -> Result<Self, DomainError> {
        let background = Self {
            color: color.into(),
            style,
            alpha,
            round_radius,
            height,
            width,
            horizontal_offset,
            vertical_offset,
        };
        background.validate()?;
        Ok(background)
    }

    /// 返回背景颜色。
    pub fn color(&self) -> &str {
        &self.color
    }
    /// 返回背景样式编号。
    pub fn style(&self) -> Option<u8> {
        self.style
    }
    /// 返回背景透明度。
    pub fn alpha(&self) -> Option<f64> {
        self.alpha
    }
    /// 返回圆角半径。
    pub fn round_radius(&self) -> Option<f64> {
        self.round_radius
    }
    /// 返回背景高度。
    pub fn height(&self) -> Option<f64> {
        self.height
    }
    /// 返回背景宽度。
    pub fn width(&self) -> Option<f64> {
        self.width
    }
    /// 返回横向偏移。
    pub fn horizontal_offset(&self) -> Option<f64> {
        self.horizontal_offset
    }
    /// 返回纵向偏移。
    pub fn vertical_offset(&self) -> Option<f64> {
        self.vertical_offset
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        let finite = [
            self.alpha,
            self.round_radius,
            self.height,
            self.width,
            self.horizontal_offset,
            self.vertical_offset,
        ]
        .iter()
        .flatten()
        .all(|value| value.is_finite());
        if !valid_rgb(&self.color)
            || self.style.is_some_and(|value| !matches!(value, 1 | 2))
            || self
                .alpha
                .is_some_and(|value| !(0.0..=1.0).contains(&value))
            || !finite
        {
            return Err(DomainError::InvalidField {
                field: "text.background",
                reason: "invalid RGB color, style, alpha, or non-finite geometry".to_owned(),
            });
        }
        Ok(())
    }
}

fn valid_rgb(value: &str) -> bool {
    valid_color(value) && value.trim().trim_start_matches('#').len() == 6
}

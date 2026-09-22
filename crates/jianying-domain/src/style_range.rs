use crate::{chroma_key::valid_color, DomainError};
use serde::{Deserialize, Serialize};

/// 文字 UTF-16 范围样式。对应 pyJianYingDraft 的 TextStyleRange。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleRange {
    range: [usize; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
}

impl StyleRange {
    /// 创建 UTF-16 文字样式区间。
    pub fn new(
        range: [usize; 2],
        size: Option<f64>,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        color: Option<String>,
    ) -> Result<Self, DomainError> {
        let style = Self {
            range,
            size,
            bold,
            italic,
            underline,
            color,
        };
        style.validate_value()?;
        Ok(style)
    }

    /// 返回 UTF-16 区间。
    pub fn range(&self) -> [usize; 2] {
        self.range
    }
    /// 返回字号。
    pub fn size(&self) -> Option<f64> {
        self.size
    }
    /// 返回粗体选择。
    pub fn bold(&self) -> Option<bool> {
        self.bold
    }
    /// 返回斜体选择。
    pub fn italic(&self) -> Option<bool> {
        self.italic
    }
    /// 返回下划线选择。
    pub fn underline(&self) -> Option<bool> {
        self.underline
    }
    /// 返回颜色。
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    pub(crate) fn validate_value(&self) -> Result<(), DomainError> {
        if self.range[0] >= self.range[1]
            || self
                .size
                .is_some_and(|value| !value.is_finite() || !(1.0..=100.0).contains(&value))
            || self.color.as_deref().is_some_and(|value| !valid_rgb(value))
        {
            return Err(DomainError::InvalidField {
                field: "text.styles",
                reason: "invalid range, size, or color".to_owned(),
            });
        }
        Ok(())
    }
}

fn valid_rgb(value: &str) -> bool {
    valid_color(value) && value.trim().trim_start_matches('#').len() == 6
}

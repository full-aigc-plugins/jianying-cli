use crate::{DomainError, RawIds, StyleRange, TextBackground, TextShadow};
use serde::{Deserialize, Serialize};

/// 文字片段完整样式；序列化时保持 v1/Job v2 平面字段。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextStyle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    border_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    border_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bold: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    italic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    underline: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    alignment: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background: Option<TextBackground>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    shadow: Option<TextShadow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    styles: Vec<StyleRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_effect: Option<RawIds>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bubble: Option<RawIds>,
}

impl TextStyle {
    /// 创建并校验完整文字样式；字体目录成员资格由应用层校验。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        size: Option<f64>,
        color: Option<String>,
        border_color: Option<String>,
        border_width: Option<f64>,
        bold: Option<bool>,
        italic: Option<bool>,
        underline: Option<bool>,
        alignment: Option<u8>,
        font: Option<String>,
        background: Option<TextBackground>,
        shadow: Option<TextShadow>,
        styles: Vec<StyleRange>,
        text_effect: Option<RawIds>,
        bubble: Option<RawIds>,
    ) -> Result<Self, DomainError> {
        let style = Self {
            size,
            color,
            border_color,
            border_width,
            bold,
            italic,
            underline,
            alignment,
            font,
            background,
            shadow,
            styles,
            text_effect,
            bubble,
        };
        style.validate("")?;
        Ok(style)
    }

    /// 返回基础字号。
    pub fn size(&self) -> Option<f64> {
        self.size
    }
    /// 返回基础颜色。
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
    /// 返回描边颜色。
    pub fn border_color(&self) -> Option<&str> {
        self.border_color.as_deref()
    }
    /// 返回描边宽度。
    pub fn border_width(&self) -> Option<f64> {
        self.border_width
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
    /// 返回对齐方式。
    pub fn alignment(&self) -> Option<u8> {
        self.alignment
    }
    /// 返回字体名称。
    pub fn font(&self) -> Option<&str> {
        self.font.as_deref()
    }
    /// 返回背景样式。
    pub fn background(&self) -> Option<&TextBackground> {
        self.background.as_ref()
    }
    /// 返回阴影样式。
    pub fn shadow(&self) -> Option<&TextShadow> {
        self.shadow.as_ref()
    }
    /// 返回范围样式。
    pub fn styles(&self) -> &[StyleRange] {
        &self.styles
    }
    /// 返回花字原始 ID。
    pub fn text_effect(&self) -> Option<&RawIds> {
        self.text_effect.as_ref()
    }
    /// 返回气泡原始 ID。
    pub fn bubble(&self) -> Option<&RawIds> {
        self.bubble.as_ref()
    }

    pub(crate) fn validate(&self, text: &str) -> Result<(), DomainError> {
        if self.size.is_some_and(|value| !value.is_finite())
            || self
                .border_width
                .is_some_and(|value| !value.is_finite() || !(0.0..=100.0).contains(&value))
            || self
                .font
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
        {
            return Err(DomainError::InvalidField {
                field: "text.style",
                reason: "invalid size, border width, or font".to_owned(),
            });
        }
        if let Some(background) = &self.background {
            background.validate()?;
        }
        if let Some(shadow) = &self.shadow {
            shadow.validate()?;
        }
        let utf16_len = text.encode_utf16().count();
        let mut previous_end = 0;
        for style in &self.styles {
            style.validate_value()?;
            let [start, end] = style.range();
            if (!text.is_empty() && end > utf16_len) || start < previous_end {
                return Err(DomainError::InvalidField {
                    field: "text.styles",
                    reason: "ranges must be ordered within the UTF-16 text".to_owned(),
                });
            }
            previous_end = end;
        }
        Ok(())
    }
}

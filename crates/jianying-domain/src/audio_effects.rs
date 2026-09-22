use crate::{AudioEffect, DomainError, Fade};
use serde::{Deserialize, Serialize};

/// 音频片段的淡化和效果集合；序列化时保持 v1/Job v2 平面字段。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioEffects {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    fade: Option<Fade>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    audio_effects: Vec<AudioEffect>,
}

impl AudioEffects {
    /// 创建音频片段效果设置。
    pub fn new(fade: Option<Fade>, effects: Vec<AudioEffect>) -> Result<Self, DomainError> {
        let settings = Self {
            fade,
            audio_effects: effects,
        };
        settings.validate()?;
        Ok(settings)
    }

    /// 返回淡化设置。
    pub fn fade(&self) -> Option<Fade> {
        self.fade
    }

    /// 返回音频效果列表。
    pub fn effects(&self) -> &[AudioEffect] {
        &self.audio_effects
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if let Some(fade) = self.fade {
            fade.validate()?;
        }
        for effect in &self.audio_effects {
            effect.validate()?;
        }
        Ok(())
    }
}

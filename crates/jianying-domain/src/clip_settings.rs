use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 媒体片段的播放与音量设置。对应 pyJianYingDraft 的速度、音量与同步变调语义。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipSettings {
    speed: f64,
    volume: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    change_pitch: Option<bool>,
}

impl ClipSettings {
    /// 创建并校验媒体片段设置。
    pub fn new(speed: f64, volume: f64, change_pitch: Option<bool>) -> Result<Self, DomainError> {
        let settings = Self {
            speed,
            volume,
            change_pitch,
        };
        settings.validate()?;
        Ok(settings)
    }

    /// 返回播放速度倍数。
    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// 返回线性音量倍数。
    pub fn volume(&self) -> f64 {
        self.volume
    }

    /// 返回变速时是否同步变调的显式选择。
    pub fn change_pitch(&self) -> Option<bool> {
        self.change_pitch
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if !self.speed.is_finite()
            || !self.volume.is_finite()
            || !(0.1..=8.0).contains(&self.speed)
            || !(0.0..=4.0).contains(&self.volume)
        {
            return Err(DomainError::InvalidField {
                field: "clip_settings",
                reason: "speed must be 0.1..8 and volume must be 0..4".to_owned(),
            });
        }
        Ok(())
    }
}

impl Default for ClipSettings {
    fn default() -> Self {
        Self {
            speed: 1.0,
            volume: 1.0,
            change_pitch: None,
        }
    }
}

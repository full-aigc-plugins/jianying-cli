use crate::{
    Animation, AudioEffects, BackgroundFilling, BlendMode, ChromaKey, ClipSettings, CropSettings,
    DomainError, Fade, Keyframes, Mask, MaterialId, SegmentId, TextStyle, TimeRange, TrackKind,
    Transform, Transition,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 强类型时间线片段；不相容字段无法出现在错误的片段变体中。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Segment {
    Video {
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        #[serde(flatten)]
        clip: ClipSettings,
        #[serde(flatten)]
        transform: Transform,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        crop: Option<CropSettings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keyframes: Option<Keyframes>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mask: Option<Mask>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        chroma: Option<ChromaKey>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        background_filling: Option<BackgroundFilling>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mix_mode: Option<BlendMode>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_in: Option<Animation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_out: Option<Animation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_group: Option<Animation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        transition_out: Option<Transition>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fade: Option<Fade>,
    },
    Audio {
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        #[serde(flatten)]
        clip: ClipSettings,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keyframes: Option<Keyframes>,
        #[serde(flatten)]
        audio_effects: AudioEffects,
    },
    Text {
        id: SegmentId,
        range: TimeRange,
        text: String,
        #[serde(flatten)]
        transform: Transform,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keyframes: Option<Keyframes>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_in: Option<Animation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_out: Option<Animation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        animation_group: Option<Animation>,
        #[serde(flatten)]
        style: TextStyle,
    },
    Sticker {
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        #[serde(flatten)]
        transform: Transform,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        keyframes: Option<Keyframes>,
    },
    Filter {
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        intensity: f64,
    },
    Effect {
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        parameters: BTreeMap<String, f64>,
    },
    Composite {
        id: SegmentId,
        range: TimeRange,
        children: Vec<SegmentId>,
    },
}

impl Segment {
    /// 返回项目内稳定的片段标识。
    pub fn id(&self) -> &SegmentId {
        match self {
            Self::Video { id, .. }
            | Self::Audio { id, .. }
            | Self::Text { id, .. }
            | Self::Sticker { id, .. }
            | Self::Filter { id, .. }
            | Self::Effect { id, .. }
            | Self::Composite { id, .. } => id,
        }
    }

    /// 创建视频片段。
    pub fn video(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        Self::video_with_settings(
            id,
            range,
            material_id,
            source_range,
            ClipSettings::default(),
            Transform::default(),
            None,
        )
    }

    /// 使用强类型 clip、视觉变换和裁剪设置创建视频片段。
    pub fn video_with_settings(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        clip: ClipSettings,
        transform: Transform,
        crop: Option<CropSettings>,
    ) -> Result<Self, DomainError> {
        Self::video_with_advanced_settings(
            id,
            range,
            material_id,
            source_range,
            clip,
            transform,
            crop,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    /// 使用完整强类型视觉设置创建视频片段。
    #[allow(clippy::too_many_arguments)]
    pub fn video_with_advanced_settings(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        clip: ClipSettings,
        transform: Transform,
        crop: Option<CropSettings>,
        keyframes: Option<Keyframes>,
        mask: Option<Mask>,
        chroma: Option<ChromaKey>,
        background_filling: Option<BackgroundFilling>,
        mix_mode: Option<BlendMode>,
        animation_in: Option<Animation>,
        animation_out: Option<Animation>,
        animation_group: Option<Animation>,
        transition_out: Option<Transition>,
        fade: Option<Fade>,
    ) -> Result<Self, DomainError> {
        let segment = Self::Video {
            id,
            range,
            material_id,
            source_range,
            clip,
            transform,
            crop,
            keyframes,
            mask,
            chroma,
            background_filling,
            mix_mode,
            animation_in,
            animation_out,
            animation_group,
            transition_out,
            fade,
        };
        segment.validate()?;
        Ok(segment)
    }

    /// 创建音频片段。
    pub fn audio(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        Self::audio_with_settings(
            id,
            range,
            material_id,
            source_range,
            ClipSettings::default(),
        )
    }

    /// 使用强类型 clip 设置创建音频片段。
    pub fn audio_with_settings(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        clip: ClipSettings,
    ) -> Result<Self, DomainError> {
        Self::audio_with_advanced_settings(
            id,
            range,
            material_id,
            source_range,
            clip,
            None,
            AudioEffects::default(),
        )
    }

    /// 使用关键帧和音频效果创建音频片段。
    pub fn audio_with_advanced_settings(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        clip: ClipSettings,
        keyframes: Option<Keyframes>,
        audio_effects: AudioEffects,
    ) -> Result<Self, DomainError> {
        let segment = Self::Audio {
            id,
            range,
            material_id,
            source_range,
            clip,
            keyframes,
            audio_effects,
        };
        segment.validate()?;
        Ok(segment)
    }

    /// 创建文本片段并拒绝空正文。
    pub fn text(id: SegmentId, range: TimeRange, text: String) -> Result<Self, DomainError> {
        if text.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "text",
                reason: "must not be blank".to_owned(),
            });
        }
        Self::text_with_transform(id, range, text, Transform::default())
    }

    /// 使用强类型视觉变换创建文字片段。
    pub fn text_with_transform(
        id: SegmentId,
        range: TimeRange,
        text: String,
        transform: Transform,
    ) -> Result<Self, DomainError> {
        Self::text_with_settings(
            id,
            range,
            text,
            transform,
            None,
            None,
            None,
            None,
            TextStyle::default(),
        )
    }

    /// 使用关键帧、动画和文字样式创建文字片段。
    #[allow(clippy::too_many_arguments)]
    pub fn text_with_settings(
        id: SegmentId,
        range: TimeRange,
        text: String,
        transform: Transform,
        keyframes: Option<Keyframes>,
        animation_in: Option<Animation>,
        animation_out: Option<Animation>,
        animation_group: Option<Animation>,
        style: TextStyle,
    ) -> Result<Self, DomainError> {
        let segment = Self::Text {
            id,
            range,
            text,
            transform,
            keyframes,
            animation_in,
            animation_out,
            animation_group,
            style,
        };
        segment.validate()?;
        Ok(segment)
    }

    /// 创建贴纸片段。
    pub fn sticker(
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
    ) -> Result<Self, DomainError> {
        if resource_id.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "resource_id",
                reason: "must not be blank".to_owned(),
            });
        }
        Self::sticker_with_transform(id, range, resource_id, Transform::default())
    }

    /// 使用强类型视觉变换创建贴纸片段。
    pub fn sticker_with_transform(
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        transform: Transform,
    ) -> Result<Self, DomainError> {
        Self::sticker_with_motion(id, range, resource_id, transform, None)
    }

    /// 使用视觉变换和关键帧创建贴纸片段。
    pub fn sticker_with_motion(
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        transform: Transform,
        keyframes: Option<Keyframes>,
    ) -> Result<Self, DomainError> {
        let segment = Self::Sticker {
            id,
            range,
            resource_id,
            transform,
            keyframes,
        };
        segment.validate()?;
        Ok(segment)
    }

    /// 创建滤镜片段。
    pub fn filter(
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        intensity: f64,
    ) -> Result<Self, DomainError> {
        if resource_id.trim().is_empty() || !(0.0..=100.0).contains(&intensity) {
            return Err(DomainError::InvalidField {
                field: "filter",
                reason: "resource_id must be non-blank and intensity must be 0..100".to_owned(),
            });
        }
        Ok(Self::Filter {
            id,
            range,
            resource_id,
            intensity,
        })
    }

    /// 创建特效片段。
    pub fn effect(
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
        parameters: BTreeMap<String, f64>,
    ) -> Result<Self, DomainError> {
        if resource_id.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "resource_id",
                reason: "must not be blank".to_owned(),
            });
        }
        Ok(Self::Effect {
            id,
            range,
            resource_id,
            parameters,
        })
    }

    /// 返回片段时间区间。
    pub fn range(&self) -> TimeRange {
        match self {
            Self::Video { range, .. }
            | Self::Audio { range, .. }
            | Self::Text { range, .. }
            | Self::Sticker { range, .. }
            | Self::Filter { range, .. }
            | Self::Effect { range, .. }
            | Self::Composite { range, .. } => *range,
        }
    }

    /// 返回片段引用的本地素材标识；编辑器资源片段返回空。
    pub fn material_id(&self) -> Option<&MaterialId> {
        match self {
            Self::Video { material_id, .. } | Self::Audio { material_id, .. } => Some(material_id),
            _ => None,
        }
    }

    /// 返回该片段唯一兼容的轨道类型。
    pub fn track_kind(&self) -> TrackKind {
        match self {
            Self::Video { .. } => TrackKind::Video,
            Self::Audio { .. } => TrackKind::Audio,
            Self::Text { .. } => TrackKind::Text,
            Self::Sticker { .. } => TrackKind::Sticker,
            Self::Filter { .. } => TrackKind::Filter,
            Self::Effect { .. } => TrackKind::Effect,
            Self::Composite { .. } => TrackKind::Composite,
        }
    }

    pub(crate) fn type_name(&self) -> &'static str {
        self.track_kind().as_str()
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        let range = self.range();
        TimeRange::new(range.start_us(), range.duration_us())?;
        match self {
            Self::Video {
                source_range,
                clip,
                transform,
                crop,
                keyframes,
                mask,
                chroma,
                background_filling,
                mix_mode,
                animation_in,
                animation_out,
                animation_group,
                transition_out,
                fade,
                ..
            } => {
                TimeRange::new(source_range.start_us(), source_range.duration_us())?;
                clip.validate()?;
                transform.validate()?;
                if let Some(crop) = crop {
                    crop.validate()?;
                }
                if let Some(keyframes) = keyframes {
                    keyframes.validate_for(TrackKind::Video, range.duration_us())?;
                }
                if let Some(mask) = mask {
                    mask.validate()?;
                }
                if let Some(chroma) = chroma {
                    chroma.validate()?;
                }
                if let Some(background) = background_filling {
                    background.validate()?;
                }
                if let Some(mix_mode) = mix_mode {
                    mix_mode.validate()?;
                }
                for animation in [animation_in, animation_out, animation_group]
                    .into_iter()
                    .flatten()
                {
                    animation.validate()?;
                }
                if let Some(transition) = transition_out {
                    transition.validate()?;
                }
                if fade.is_some_and(|fade| fade.in_us() < 0 || fade.out_us() < 0) {
                    return Err(DomainError::InvalidField {
                        field: "fade",
                        reason: "durations must be non-negative".to_owned(),
                    });
                }
                Ok(())
            }
            Self::Audio {
                source_range,
                clip,
                keyframes,
                audio_effects,
                ..
            } => {
                TimeRange::new(source_range.start_us(), source_range.duration_us())?;
                clip.validate()?;
                if let Some(keyframes) = keyframes {
                    keyframes.validate_for(TrackKind::Audio, range.duration_us())?;
                }
                audio_effects.validate()
            }
            Self::Text {
                text,
                transform,
                keyframes,
                animation_in,
                animation_out,
                animation_group,
                style,
                ..
            } => {
                if text.trim().is_empty() {
                    return Err(DomainError::InvalidField {
                        field: "text",
                        reason: "must not be blank".to_owned(),
                    });
                }
                transform.validate()?;
                if let Some(keyframes) = keyframes {
                    keyframes.validate_for(TrackKind::Text, range.duration_us())?;
                }
                for animation in [animation_in, animation_out, animation_group]
                    .into_iter()
                    .flatten()
                {
                    animation.validate()?;
                }
                style.validate(text)
            }
            Self::Sticker {
                resource_id,
                transform,
                keyframes,
                ..
            } => {
                if resource_id.trim().is_empty() {
                    return Err(DomainError::InvalidField {
                        field: "resource_id",
                        reason: "must not be blank".to_owned(),
                    });
                }
                transform.validate()?;
                if let Some(keyframes) = keyframes {
                    keyframes.validate_for(TrackKind::Sticker, range.duration_us())?;
                }
                Ok(())
            }
            Self::Filter {
                resource_id,
                intensity,
                ..
            } if resource_id.trim().is_empty()
                || !intensity.is_finite()
                || !(0.0..=100.0).contains(intensity) =>
            {
                Err(DomainError::InvalidField {
                    field: "filter",
                    reason: "resource_id must be non-blank and intensity must be 0..100".to_owned(),
                })
            }
            Self::Effect { parameters, .. }
                if parameters.values().any(|value| !value.is_finite()) =>
            {
                Err(DomainError::InvalidField {
                    field: "effect.parameters",
                    reason: "values must be finite".to_owned(),
                })
            }
            Self::Composite { children, .. } if children.is_empty() => {
                Err(DomainError::InvalidField {
                    field: "composite.children",
                    reason: "must not be empty".to_owned(),
                })
            }
            Self::Effect { resource_id, .. } if resource_id.trim().is_empty() => {
                Err(DomainError::InvalidField {
                    field: "resource_id",
                    reason: "must not be blank".to_owned(),
                })
            }
            _ => Ok(()),
        }
    }
}

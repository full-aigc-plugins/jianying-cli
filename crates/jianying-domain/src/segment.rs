use crate::{DomainError, MaterialId, SegmentId, TimeRange, TrackKind};
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
        speed: f64,
        volume: f64,
    },
    Audio {
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
        speed: f64,
        volume: f64,
    },
    Text {
        id: SegmentId,
        range: TimeRange,
        text: String,
    },
    Sticker {
        id: SegmentId,
        range: TimeRange,
        resource_id: String,
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
    /// 创建视频片段。
    pub fn video(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        Ok(Self::Video {
            id,
            range,
            material_id,
            source_range,
            speed: 1.0,
            volume: 1.0,
        })
    }

    /// 创建音频片段。
    pub fn audio(
        id: SegmentId,
        range: TimeRange,
        material_id: MaterialId,
        source_range: TimeRange,
    ) -> Result<Self, DomainError> {
        Ok(Self::Audio {
            id,
            range,
            material_id,
            source_range,
            speed: 1.0,
            volume: 1.0,
        })
    }

    /// 创建文本片段并拒绝空正文。
    pub fn text(id: SegmentId, range: TimeRange, text: String) -> Result<Self, DomainError> {
        if text.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "text",
                reason: "must not be blank".to_owned(),
            });
        }
        Ok(Self::Text { id, range, text })
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
        Ok(Self::Sticker {
            id,
            range,
            resource_id,
        })
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

    pub(crate) fn track_kind(&self) -> TrackKind {
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
                speed,
                volume,
                ..
            }
            | Self::Audio {
                source_range,
                speed,
                volume,
                ..
            } => {
                TimeRange::new(source_range.start_us(), source_range.duration_us())?;
                if !speed.is_finite()
                    || !volume.is_finite()
                    || !(0.1..=8.0).contains(speed)
                    || !(0.0..=4.0).contains(volume)
                {
                    return Err(DomainError::InvalidField {
                        field: "media_segment",
                        reason: "speed must be 0.1..8 and volume must be 0..4".to_owned(),
                    });
                }
                Ok(())
            }
            Self::Text { text, .. } if text.trim().is_empty() => Err(DomainError::InvalidField {
                field: "text",
                reason: "must not be blank".to_owned(),
            }),
            Self::Sticker { resource_id, .. } | Self::Effect { resource_id, .. }
                if resource_id.trim().is_empty() =>
            {
                Err(DomainError::InvalidField {
                    field: "resource_id",
                    reason: "must not be blank".to_owned(),
                })
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
            _ => Ok(()),
        }
    }
}

use crate::{DomainError, Material, Segment, SegmentId, TimeRange, TrackKind};
use serde::{Deserialize, Serialize};

/// 可审计的强类型编辑操作；执行层据此生成 MutationPlan。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum EditOperation {
    AddMaterial {
        material: Material,
    },
    AddSegment {
        track_id: String,
        segment: Box<Segment>,
    },
    AddTrack {
        track_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        kind: TrackKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    RemoveTrack {
        track_id: String,
    },
    ReorderTrack {
        track_id: String,
        index: usize,
    },
    RemoveSegment {
        segment_id: SegmentId,
    },
    MoveSegment {
        segment_id: SegmentId,
        target: TimeRange,
    },
    ReplaceText {
        segment_id: SegmentId,
        text: String,
    },
}

impl EditOperation {
    /// 校验反序列化后的编辑操作，防止绕过领域构造器注入空标识或非法区间。
    pub fn validate(&self) -> Result<(), DomainError> {
        match self {
            Self::AddMaterial { material } => material.validate(),
            Self::AddSegment { track_id, segment } => {
                validate_track_id(track_id)?;
                segment.validate()
            }
            Self::AddTrack {
                track_id,
                name,
                kind,
                ..
            } => {
                validate_track_id(track_id)?;
                if name.as_deref().is_some_and(|value| value.trim().is_empty()) {
                    return Err(DomainError::InvalidField {
                        field: "track_name",
                        reason: "must not be blank when present".to_owned(),
                    });
                }
                if *kind == TrackKind::Composite {
                    return Err(DomainError::InvalidField {
                        field: "track_kind",
                        reason: "composite tracks are not writable by the draft adapter".to_owned(),
                    });
                }
                Ok(())
            }
            Self::RemoveTrack { track_id } | Self::ReorderTrack { track_id, .. } => {
                validate_track_id(track_id)
            }
            Self::RemoveSegment { segment_id } => validate_segment_id(segment_id),
            Self::MoveSegment { segment_id, target } => {
                validate_segment_id(segment_id)?;
                TimeRange::new(target.start_us(), target.duration_us())?;
                Ok(())
            }
            Self::ReplaceText { segment_id, text } => {
                validate_segment_id(segment_id)?;
                if text.trim().is_empty() {
                    return Err(DomainError::InvalidField {
                        field: "text",
                        reason: "must not be blank".to_owned(),
                    });
                }
                Ok(())
            }
        }
    }
}

fn validate_track_id(track_id: &str) -> Result<(), DomainError> {
    if track_id.trim().is_empty() {
        return Err(DomainError::InvalidField {
            field: "track_id",
            reason: "must not be blank".to_owned(),
        });
    }
    Ok(())
}

fn validate_segment_id(segment_id: &SegmentId) -> Result<(), DomainError> {
    if segment_id.as_str().trim().is_empty() {
        return Err(DomainError::InvalidField {
            field: "segment_id",
            reason: "must not be blank".to_owned(),
        });
    }
    Ok(())
}

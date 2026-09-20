use crate::{DomainError, Material, Segment, SegmentId, TimeRange};
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
        segment: Segment,
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
                if track_id.trim().is_empty() {
                    return Err(DomainError::InvalidField {
                        field: "track_id",
                        reason: "must not be blank".to_owned(),
                    });
                }
                segment.validate()
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

fn validate_segment_id(segment_id: &SegmentId) -> Result<(), DomainError> {
    if segment_id.as_str().trim().is_empty() {
        return Err(DomainError::InvalidField {
            field: "segment_id",
            reason: "must not be blank".to_owned(),
        });
    }
    Ok(())
}

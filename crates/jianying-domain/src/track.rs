use crate::{DomainError, Segment, TrackKind};
use serde::{Deserialize, Serialize};

/// 同一类型、按时间排序且不重叠的片段轨道。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    id: String,
    kind: TrackKind,
    segments: Vec<Segment>,
}

impl Track {
    /// 创建并校验轨道类型、排序和重叠不变量。
    pub fn new(
        id: impl Into<String>,
        kind: TrackKind,
        segments: Vec<Segment>,
    ) -> Result<Self, DomainError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(DomainError::InvalidField {
                field: "track_id",
                reason: "must not be blank".to_owned(),
            });
        }
        let mut previous_end_us = 0;
        for segment in &segments {
            segment.validate()?;
            if segment.track_kind() != kind {
                return Err(DomainError::IncompatibleSegment {
                    segment_type: segment.type_name(),
                    track_type: kind.as_str(),
                });
            }
            if segment.range().start_us() < previous_end_us {
                return Err(DomainError::SegmentOverlap {
                    track_id: id.clone(),
                    previous_end_us,
                    next_start_us: segment.range().start_us(),
                });
            }
            previous_end_us = segment.range().end_us();
        }
        Ok(Self { id, kind, segments })
    }

    /// 返回轨道标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回轨道类型。
    pub fn kind(&self) -> TrackKind {
        self.kind
    }

    /// 返回只读片段列表。
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }
}

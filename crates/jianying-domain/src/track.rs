use crate::{DomainError, Segment, TrackKind};
use serde::{Deserialize, Serialize};

/// 同一类型、按时间排序且不重叠的片段轨道。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
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
        Self::new_named(id, None, kind, segments)
    }

    /// 创建带可选显示名的轨道；显示名与稳定内部标识相互独立。
    pub fn new_named(
        id: impl Into<String>,
        name: Option<String>,
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
        if name.as_deref().is_some_and(|value| value.trim().is_empty()) {
            return Err(DomainError::InvalidField {
                field: "track_name",
                reason: "must not be blank when present".to_owned(),
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
        Ok(Self {
            id,
            name,
            kind,
            segments,
        })
    }

    /// 返回轨道标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回用户可见的轨道名称；缺失时由 wire adapter 使用轨道类型默认名。
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
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

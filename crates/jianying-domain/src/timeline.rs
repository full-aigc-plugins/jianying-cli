use crate::{DomainError, Track};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 项目时间线；轨道标识在项目内唯一。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timeline {
    tracks: Vec<Track>,
}

impl Timeline {
    /// 创建至少包含一条轨道、且轨道标识唯一的时间线。
    pub fn new(tracks: Vec<Track>) -> Result<Self, DomainError> {
        if tracks.is_empty() {
            return Err(DomainError::InvalidField {
                field: "tracks",
                reason: "must contain at least one track".to_owned(),
            });
        }
        let mut ids = BTreeSet::new();
        for track in &tracks {
            if !ids.insert(track.id()) {
                return Err(DomainError::InvalidField {
                    field: "track_id",
                    reason: format!("duplicate track id {}", track.id()),
                });
            }
        }
        Ok(Self { tracks })
    }

    /// 返回只读轨道列表。
    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }
}

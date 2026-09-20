use crate::{MediaError, MediaKind};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 经探测并校验后可供作业引用的媒体素材。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAsset {
    path: PathBuf,
    kind: MediaKind,
    duration_us: i64,
    dimensions: Option<(u32, u32)>,
}

impl MediaAsset {
    /// 创建媒体素材元数据。
    pub fn new(
        path: PathBuf,
        kind: MediaKind,
        duration_us: i64,
        dimensions: Option<(u32, u32)>,
    ) -> Result<Self, MediaError> {
        if path.as_os_str().is_empty() {
            return Err(MediaError::EmptyPath);
        }
        if duration_us <= 0 {
            return Err(MediaError::InvalidDuration);
        }
        if dimensions.is_some_and(|(width, height)| width == 0 || height == 0) {
            return Err(MediaError::InvalidDimensions);
        }
        Ok(Self {
            path,
            kind,
            duration_us,
            dimensions,
        })
    }

    /// 返回媒体时长，单位微秒。
    pub fn duration_us(&self) -> i64 {
        self.duration_us
    }
}

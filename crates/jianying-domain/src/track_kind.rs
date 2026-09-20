use serde::{Deserialize, Serialize};

/// 时间线轨道种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    Video,
    Audio,
    Text,
    Sticker,
    Filter,
    Effect,
    Composite,
}

impl TrackKind {
    /// 返回稳定的 snake_case 轨道类型名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Text => "text",
            Self::Sticker => "sticker",
            Self::Filter => "filter",
            Self::Effect => "effect",
            Self::Composite => "composite",
        }
    }
}

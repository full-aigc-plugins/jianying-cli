use serde::{Deserialize, Serialize};

/// 剪映官方资源域；未知类型只允许保留证据，不允许自动应用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficialResourceKind {
    Media,
    Music,
    TextTemplate,
    Sticker,
    VideoEffect,
    Transition,
    CaptionStyle,
    SmartPackage,
    SmartBRoll,
    Filter,
    Adjustment,
    Template,
    DigitalHuman,
    Animation,
    SoundEffect,
    Unknown,
}

impl OfficialResourceKind {
    /// 未知资源域不得进入自动编译路径。
    pub fn supports_automatic_application(self) -> bool {
        self != Self::Unknown
    }
}

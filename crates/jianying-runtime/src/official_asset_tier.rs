use crate::JianyingEdition;
use serde::{Deserialize, Serialize};

/// 剪映官方素材的权益层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficialAssetTier {
    Basic,
    Professional,
}

impl OfficialAssetTier {
    /// 返回使用该素材要求的账号版型。
    pub fn required_edition(self) -> JianyingEdition {
        match self {
            Self::Basic => JianyingEdition::Basic,
            Self::Professional => JianyingEdition::Professional,
        }
    }

    /// 返回使用该素材要求的权益能力。
    pub fn required_capability(self) -> &'static str {
        match self {
            Self::Basic => "official_assets.basic",
            Self::Professional => "official_assets.professional",
        }
    }
}

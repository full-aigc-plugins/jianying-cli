use serde::{Deserialize, Serialize};
use std::fmt;

/// 官方素材收据声明的允许用途。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfficialAssetUsage {
    Personal,
    Commercial,
    Editorial,
}

impl fmt::Display for OfficialAssetUsage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Personal => "personal",
            Self::Commercial => "commercial",
            Self::Editorial => "editorial",
        };
        formatter.write_str(value)
    }
}

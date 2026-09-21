use serde::{Deserialize, Serialize};
use std::fmt;

/// 账号权益在观察时刻的状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntitlementStatus {
    Unknown,
    Active,
    Inactive,
}

impl fmt::Display for EntitlementStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Unknown => "unknown",
            Self::Active => "active",
            Self::Inactive => "inactive",
        };
        formatter.write_str(value)
    }
}

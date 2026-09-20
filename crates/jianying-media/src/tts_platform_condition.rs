use serde::{Deserialize, Serialize};

/// Provider 可运行的平台约束。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsPlatformCondition {
    Any,
    MacOs,
    Windows,
    Linux,
}

use serde::{Deserialize, Serialize};

/// 被探测的剪映运行平台。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimePlatform {
    MacOs,
    Windows,
    Linux,
}

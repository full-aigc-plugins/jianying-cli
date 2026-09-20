use serde::{Deserialize, Serialize};

/// Provider 原始响应承载音频的方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsOutputMode {
    File,
    Binary,
    Base64,
    Hex,
    Url,
}

use serde::{Deserialize, Serialize};

/// ASR 转录产物格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrOutputFormat {
    Json,
    Text,
    Srt,
    VerboseJson,
    Vtt,
}

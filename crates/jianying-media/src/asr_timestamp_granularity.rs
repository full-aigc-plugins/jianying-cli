use serde::{Deserialize, Serialize};

/// ASR 详细 JSON 中允许请求的时间戳粒度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AsrTimestampGranularity {
    Segment,
    Word,
}

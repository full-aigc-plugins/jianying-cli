use serde::{Deserialize, Serialize};

/// TTS 文本输入的语义类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsInputKind {
    Text,
    Ssml,
}

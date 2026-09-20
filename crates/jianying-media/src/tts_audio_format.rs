use serde::{Deserialize, Serialize};

/// TTS 统一输出音频格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsAudioFormat {
    Wav,
    Mp3,
    Pcm,
    Ogg,
    Aac,
    Flac,
    Aiff,
}

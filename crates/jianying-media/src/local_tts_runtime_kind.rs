/// 已建立来源档案的本地开源 TTS 运行时种类。
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum LocalTtsRuntimeKind {
    /// 阿里 Qwen3-TTS 本地运行时。
    Qwen3Tts,
    /// FunAudioLLM CosyVoice 本地运行时。
    CosyVoice,
    /// GPT-SoVITS 本地运行时。
    GptSovits,
    /// MyShell MeloTTS 本地运行时。
    MeloTts,
    /// 网易有道 EmotiVoice 本地运行时。
    EmotiVoice,
    /// CAMB.AI MARS5-TTS 本地运行时。
    Mars5Tts,
    /// ChatTTS 本地运行时。
    ChatTts,
    /// F5-TTS 本地运行时。
    F5Tts,
    /// Fish Speech 本地运行时。
    FishSpeech,
    /// IndexTTS 本地运行时。
    IndexTts,
}

impl LocalTtsRuntimeKind {
    /// 返回稳定的机器可读运行时标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Qwen3Tts => "qwen3-tts",
            Self::CosyVoice => "cosyvoice",
            Self::GptSovits => "gpt-sovits",
            Self::MeloTts => "melotts",
            Self::EmotiVoice => "emotivoice",
            Self::Mars5Tts => "mars5-tts",
            Self::ChatTts => "chattts",
            Self::F5Tts => "f5-tts",
            Self::FishSpeech => "fish-speech",
            Self::IndexTts => "indextts",
        }
    }
}

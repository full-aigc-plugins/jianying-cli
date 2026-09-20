use crate::{CredentialRef, TtsAudioFormat, TtsError, TtsInputKind};
use serde::{Deserialize, Serialize};

/// 与厂商无关的统一 TTS 合成请求。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TtsRequest {
    text: String,
    input_kind: TtsInputKind,
    model: Option<String>,
    voice: Option<String>,
    language: Option<String>,
    speed: f32,
    volume: f32,
    pitch: f32,
    emotion: Option<String>,
    format: TtsAudioFormat,
    sample_rate_hz: Option<u32>,
    streaming: bool,
    timestamps: bool,
    credential: Option<CredentialRef>,
}

impl TtsRequest {
    /// 创建最小 TTS 请求，默认使用普通文本、标准语速/音量和零音高偏移。
    pub fn new(text: impl Into<String>, format: TtsAudioFormat) -> Result<Self, TtsError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(TtsError::EmptyText);
        }
        Ok(Self {
            text,
            input_kind: TtsInputKind::Text,
            model: None,
            voice: None,
            language: None,
            speed: 1.0,
            volume: 1.0,
            pitch: 0.0,
            emotion: None,
            format,
            sample_rate_hz: None,
            streaming: false,
            timestamps: false,
            credential: None,
        })
    }

    /// 设置输入为普通文本或 SSML。
    pub fn with_input_kind(mut self, input_kind: TtsInputKind) -> Self {
        self.input_kind = input_kind;
        self
    }

    /// 设置 Provider 模型标识。
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置音色标识。
    pub fn with_voice(mut self, voice: impl Into<String>) -> Self {
        self.voice = Some(voice.into());
        self
    }

    /// 设置 BCP-47 或 Provider 接受的语言标识。
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// 设置语速倍率，允许范围为 0.25 至 4.0。
    pub fn with_speed(mut self, speed: f32) -> Result<Self, TtsError> {
        validate_scalar("speed", speed, 0.25, 4.0)?;
        self.speed = speed;
        Ok(self)
    }

    /// 设置音量倍率，允许范围为 0.0 至 2.0。
    pub fn with_volume(mut self, volume: f32) -> Result<Self, TtsError> {
        validate_scalar("volume", volume, 0.0, 2.0)?;
        self.volume = volume;
        Ok(self)
    }

    /// 设置归一化音高偏移，允许范围为 -1.0 至 1.0。
    pub fn with_pitch(mut self, pitch: f32) -> Result<Self, TtsError> {
        validate_scalar("pitch", pitch, -1.0, 1.0)?;
        self.pitch = pitch;
        Ok(self)
    }

    /// 设置情绪标识。
    pub fn with_emotion(mut self, emotion: impl Into<String>) -> Self {
        self.emotion = Some(emotion.into());
        self
    }

    /// 设置采样率。
    pub fn with_sample_rate_hz(mut self, sample_rate_hz: u32) -> Result<Self, TtsError> {
        if sample_rate_hz == 0 {
            return Err(TtsError::InvalidSampleRate);
        }
        self.sample_rate_hz = Some(sample_rate_hz);
        Ok(self)
    }

    /// 请求流式响应。
    pub fn with_streaming(mut self, streaming: bool) -> Self {
        self.streaming = streaming;
        self
    }

    /// 请求字词或句子时间戳。
    pub fn with_timestamps(mut self, timestamps: bool) -> Self {
        self.timestamps = timestamps;
        self
    }

    /// 绑定凭据引用；引用不包含秘密值。
    pub fn with_credential(mut self, credential: CredentialRef) -> Self {
        self.credential = Some(credential);
        self
    }

    /// 返回待合成文本。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 返回输入语义类型。
    pub fn input_kind(&self) -> TtsInputKind {
        self.input_kind
    }

    /// 返回模型标识。
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// 返回音色标识。
    pub fn voice(&self) -> Option<&str> {
        self.voice.as_deref()
    }

    /// 返回语言标识。
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// 返回语速倍率。
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// 返回音量倍率。
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// 返回归一化音高偏移。
    pub fn pitch(&self) -> f32 {
        self.pitch
    }

    /// 返回情绪标识。
    pub fn emotion(&self) -> Option<&str> {
        self.emotion.as_deref()
    }

    /// 返回目标音频格式。
    pub fn format(&self) -> TtsAudioFormat {
        self.format
    }

    /// 返回目标采样率。
    pub fn sample_rate_hz(&self) -> Option<u32> {
        self.sample_rate_hz
    }

    /// 返回是否请求流式传输。
    pub fn streaming(&self) -> bool {
        self.streaming
    }

    /// 返回是否请求时间戳。
    pub fn timestamps(&self) -> bool {
        self.timestamps
    }

    /// 返回凭据引用。
    pub fn credential(&self) -> Option<&CredentialRef> {
        self.credential.as_ref()
    }
}

fn validate_scalar(
    field: &'static str,
    value: f32,
    minimum: f32,
    maximum: f32,
) -> Result<(), TtsError> {
    if !value.is_finite() || value < minimum || value > maximum {
        return Err(TtsError::InvalidScalar { field, value });
    }
    Ok(())
}

use crate::{
    TtsAudioFormat, TtsAuthScheme, TtsError, TtsInputKind, TtsOutputMode, TtsPlatformCondition,
    TtsRequest,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// 单个 TTS Provider 可验证的能力声明。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCapability {
    provider_id: String,
    billed: bool,
    platform: TtsPlatformCondition,
    auth_scheme: TtsAuthScheme,
    formats: BTreeSet<TtsAudioFormat>,
    output_modes: BTreeSet<TtsOutputMode>,
    max_text_chars: usize,
    models: BTreeSet<String>,
    model_selection: bool,
    voice_catalog: bool,
    ssml: bool,
    voice_clone: bool,
    voice_design: bool,
    emotions: bool,
    language_selection: bool,
    speed_control: bool,
    volume_control: bool,
    pitch_control: bool,
    sample_rates_hz: BTreeSet<u32>,
    timestamps: bool,
    streaming: bool,
}

impl ProviderCapability {
    /// 创建 Provider 能力声明并校验必填边界。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider_id: impl Into<String>,
        billed: bool,
        platform: TtsPlatformCondition,
        auth_scheme: TtsAuthScheme,
        formats: BTreeSet<TtsAudioFormat>,
        output_modes: BTreeSet<TtsOutputMode>,
        max_text_chars: usize,
    ) -> Result<Self, TtsError> {
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(TtsError::EmptyProviderId);
        }
        if formats.is_empty() {
            return Err(TtsError::MissingAudioFormat);
        }
        if output_modes.is_empty() {
            return Err(TtsError::MissingOutputMode);
        }
        if max_text_chars == 0 {
            return Err(TtsError::InvalidTextLimit);
        }
        Ok(Self {
            provider_id,
            billed,
            platform,
            auth_scheme,
            formats,
            output_modes,
            max_text_chars,
            models: BTreeSet::new(),
            model_selection: false,
            voice_catalog: false,
            ssml: false,
            voice_clone: false,
            voice_design: false,
            emotions: false,
            language_selection: false,
            speed_control: false,
            volume_control: false,
            pitch_control: false,
            sample_rates_hz: BTreeSet::new(),
            timestamps: false,
            streaming: false,
        })
    }

    /// 声明允许的模型集合；空集合表示模型名由本地适配器自行处理。
    pub fn with_models<I, S>(mut self, models: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.models = models.into_iter().map(Into::into).collect();
        self.model_selection = true;
        self
    }

    /// 声明是否允许自由模型标识，适用于本地通用协议。
    pub fn with_model_selection(mut self, supported: bool) -> Self {
        self.model_selection = supported;
        self
    }

    /// 声明是否支持音色目录或显式音色选择。
    pub fn with_voice_catalog(mut self, supported: bool) -> Self {
        self.voice_catalog = supported;
        self
    }

    /// 声明是否支持 SSML。
    pub fn with_ssml(mut self, supported: bool) -> Self {
        self.ssml = supported;
        self
    }

    /// 声明是否支持音色克隆。
    pub fn with_voice_clone(mut self, supported: bool) -> Self {
        self.voice_clone = supported;
        self
    }

    /// 声明是否支持音色设计。
    pub fn with_voice_design(mut self, supported: bool) -> Self {
        self.voice_design = supported;
        self
    }

    /// 声明是否支持情绪参数。
    pub fn with_emotions(mut self, supported: bool) -> Self {
        self.emotions = supported;
        self
    }

    /// 声明是否支持显式语言选择。
    pub fn with_language_selection(mut self, supported: bool) -> Self {
        self.language_selection = supported;
        self
    }

    /// 声明是否支持语速控制。
    pub fn with_speed_control(mut self, supported: bool) -> Self {
        self.speed_control = supported;
        self
    }

    /// 声明是否支持音量控制。
    pub fn with_volume_control(mut self, supported: bool) -> Self {
        self.volume_control = supported;
        self
    }

    /// 声明是否支持音高控制。
    pub fn with_pitch_control(mut self, supported: bool) -> Self {
        self.pitch_control = supported;
        self
    }

    /// 声明允许显式选择的采样率；空集合表示不允许显式选择。
    pub fn with_sample_rates<I>(mut self, sample_rates_hz: I) -> Self
    where
        I: IntoIterator<Item = u32>,
    {
        self.sample_rates_hz = sample_rates_hz.into_iter().collect();
        self
    }

    /// 声明是否支持时间戳。
    pub fn with_timestamps(mut self, supported: bool) -> Self {
        self.timestamps = supported;
        self
    }

    /// 声明是否支持流式响应。
    pub fn with_streaming(mut self, supported: bool) -> Self {
        self.streaming = supported;
        self
    }

    /// 按声明逐项验证统一请求，禁止 adapter 静默忽略不支持参数。
    pub fn validate_request(&self, request: &TtsRequest) -> Result<(), TtsError> {
        if !self.formats.contains(&request.format()) {
            return Err(TtsError::UnsupportedAudioFormat {
                provider: self.provider_id.clone(),
                format: request.format(),
            });
        }
        let text_chars = request.text().chars().count();
        if text_chars > self.max_text_chars {
            return Err(TtsError::TextTooLong {
                provider: self.provider_id.clone(),
                actual: text_chars,
                maximum: self.max_text_chars,
            });
        }
        if request.input_kind() == TtsInputKind::Ssml && !self.ssml {
            return Err(self.unsupported("SSML"));
        }
        if request.voice().is_some() && !self.voice_catalog {
            return Err(self.unsupported("voice selection"));
        }
        if request.emotion().is_some() && !self.emotions {
            return Err(self.unsupported("emotion"));
        }
        if request.language().is_some() && !self.language_selection {
            return Err(self.unsupported("language selection"));
        }
        if request.speed() != 1.0 && !self.speed_control {
            return Err(self.unsupported("speed control"));
        }
        if request.volume() != 1.0 && !self.volume_control {
            return Err(self.unsupported("volume control"));
        }
        if request.pitch() != 0.0 && !self.pitch_control {
            return Err(self.unsupported("pitch control"));
        }
        if let Some(sample_rate_hz) = request.sample_rate_hz() {
            if !self.sample_rates_hz.contains(&sample_rate_hz) {
                return Err(self.unsupported("sample rate"));
            }
        }
        if request.streaming() && !self.streaming {
            return Err(self.unsupported("streaming"));
        }
        if request.timestamps() && !self.timestamps {
            return Err(self.unsupported("timestamps"));
        }
        if let Some(model) = request.model() {
            if !self.model_selection {
                return Err(self.unsupported("model selection"));
            }
            if !self.models.is_empty() && !self.models.contains(model) {
                return Err(TtsError::UnsupportedModel {
                    provider: self.provider_id.clone(),
                    model: model.to_owned(),
                });
            }
        }
        if self.auth_scheme != TtsAuthScheme::None && request.credential().is_none() {
            return Err(TtsError::MissingCredential {
                provider: self.provider_id.clone(),
            });
        }
        Ok(())
    }

    /// 返回稳定 Provider 标识。
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// 返回调用是否可能产生外部费用。
    pub fn billed(&self) -> bool {
        self.billed
    }

    /// 返回平台约束。
    pub fn platform(&self) -> TtsPlatformCondition {
        self.platform
    }

    /// 返回鉴权方式。
    pub fn auth_scheme(&self) -> TtsAuthScheme {
        self.auth_scheme
    }

    fn unsupported(&self, capability: &'static str) -> TtsError {
        TtsError::UnsupportedCapability {
            provider: self.provider_id.clone(),
            capability,
        }
    }
}

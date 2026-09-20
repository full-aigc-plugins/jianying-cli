use crate::{
    CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsRequest,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// 智谱 GLM-TTS JSON 请求与二进制音频响应映射器。
#[derive(Debug)]
pub struct ZhipuGlmTtsAdapter {
    capability: ProviderCapability,
}

impl ZhipuGlmTtsAdapter {
    /// 创建官方 GLM-TTS adapter。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "zhipu-glm",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::BearerToken,
            BTreeSet::from([TtsAudioFormat::Wav, TtsAudioFormat::Pcm]),
            BTreeSet::from([TtsOutputMode::Binary]),
            1_024,
        )?
        .with_models(["glm-tts"])
        .with_voice_catalog(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_streaming(true);
        Ok(Self { capability })
    }

    /// 返回官方文本转语音 endpoint。
    pub fn endpoint(&self) -> &str {
        "https://open.bigmodel.cn/api/paas/v4/audio/speech"
    }

    /// 返回能力声明。
    pub fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    /// 将统一请求编码为 GLM-TTS JSON 请求体。
    pub fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        if !(0.5..=2.0).contains(&request.speed()) {
            return Err(TtsError::InvalidScalar {
                field: "zhipu.speed",
                value: request.speed(),
            });
        }
        if request.volume() <= 0.0 {
            return Err(TtsError::InvalidScalar {
                field: "zhipu.volume",
                value: request.volume(),
            });
        }
        if request.streaming() && request.format() != TtsAudioFormat::Pcm {
            return Err(TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "streaming requires pcm",
            });
        }
        let voice = request
            .voice()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit voice",
            })?;
        Ok(json!({
            "model": request.model().unwrap_or("glm-tts"),
            "input": request.text(),
            "voice": voice,
            "response_format": match request.format() {
                TtsAudioFormat::Wav => "wav",
                TtsAudioFormat::Pcm => "pcm",
                _ => unreachable!("capability validation rejects other formats"),
            },
            "stream": request.streaming(),
            "speed": request.speed(),
            "volume": request.volume()
        }))
    }

    /// 校验非流式二进制音频响应；流式 Event Stream 由网络执行器分帧后再交付。
    pub fn decode_audio(
        &self,
        bytes: &[u8],
        content_type: &str,
        request_id: Option<&str>,
    ) -> Result<CloudTtsOutput, TtsError> {
        if bytes.is_empty() {
            return Err(TtsError::EmptyArtifact);
        }
        if !content_type.to_ascii_lowercase().starts_with("audio/") {
            return Err(TtsError::InvalidProviderResponse {
                provider: self.capability.provider_id().to_owned(),
                reason: format!("expected audio response, received {content_type}"),
            });
        }
        Ok(CloudTtsOutput::Inline {
            bytes: bytes.to_vec(),
            request_id: request_id.map(str::to_owned),
        })
    }
}

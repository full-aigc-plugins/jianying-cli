use crate::{
    CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsRequest,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::IpAddr;
use url::{Host, Url};

/// EmotiVoice 本地 OpenAI Audio 兼容协议 codec。
#[derive(Debug)]
pub struct EmotiVoiceTtsAdapter {
    endpoint: String,
    capability: ProviderCapability,
}

impl EmotiVoiceTtsAdapter {
    /// 创建只允许回环 HTTP endpoint 的 EmotiVoice codec。
    pub fn new(endpoint: impl Into<String>) -> Result<Self, TtsError> {
        let endpoint = endpoint.into();
        validate_loopback_endpoint(&endpoint)?;
        let capability = ProviderCapability::new(
            "emotivoice-local",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([
                TtsAudioFormat::Mp3,
                TtsAudioFormat::Ogg,
                TtsAudioFormat::Aac,
                TtsAudioFormat::Flac,
            ]),
            BTreeSet::from([TtsOutputMode::Binary]),
            4_096,
        )?
        .with_models(["tts-1", "tts-1-hd"])
        .with_voice_catalog(true)
        .with_speed_control(true);
        Ok(Self {
            endpoint,
            capability,
        })
    }

    /// 返回本地 endpoint。
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// 返回可验证能力声明。
    pub fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    /// 将统一请求映射为本地 OpenAI Audio JSON。
    pub fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        Ok(json!({
            "model": request.model().unwrap_or("tts-1"),
            "input": request.text(),
            "voice": request.voice().unwrap_or("alloy"),
            "response_format": format_name(request.format()),
            "speed": request.speed()
        }))
    }

    /// 校验离线二进制响应并提取审计请求 ID。
    pub fn decode_audio(
        &self,
        bytes: &[u8],
        content_type: &str,
        request_id: Option<&str>,
    ) -> Result<CloudTtsOutput, TtsError> {
        if bytes.is_empty() {
            return Err(TtsError::EmptyArtifact);
        }
        if !content_type.starts_with("audio/") && content_type != "application/octet-stream" {
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

fn format_name(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Mp3 => "mp3",
        TtsAudioFormat::Ogg => "opus",
        TtsAudioFormat::Aac => "aac",
        TtsAudioFormat::Flac => "flac",
        TtsAudioFormat::Wav => "wav",
        TtsAudioFormat::Pcm => "pcm",
        TtsAudioFormat::Aiff => "aiff",
    }
}

fn validate_loopback_endpoint(endpoint: &str) -> Result<(), TtsError> {
    let url =
        Url::parse(endpoint).map_err(|error| TtsError::InvalidLocalEndpoint(error.to_string()))?;
    let loopback = match url.host() {
        Some(Host::Domain(value)) => value.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(value)) => IpAddr::V4(value).is_loopback(),
        Some(Host::Ipv6(value)) => IpAddr::V6(value).is_loopback(),
        None => false,
    };
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() || !loopback
    {
        return Err(TtsError::InvalidLocalEndpoint(
            "EmotiVoice endpoint must be credential-free loopback HTTP".to_owned(),
        ));
    }
    Ok(())
}

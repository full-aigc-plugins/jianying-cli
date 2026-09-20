use crate::{
    CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsRequest,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;
use url::{Host, Url};

/// ChatTTS-ui 本地表单协议的独立 Rust codec。
#[derive(Debug)]
pub struct ChatTtsUiAdapter {
    endpoint: String,
    capability: ProviderCapability,
}

impl ChatTtsUiAdapter {
    /// 创建只允许回环 HTTP endpoint 的 ChatTTS-ui codec。
    pub fn new(endpoint: impl Into<String>) -> Result<Self, TtsError> {
        let endpoint = endpoint.into();
        validate_loopback_endpoint(&endpoint)?;
        let capability = ProviderCapability::new(
            "chattts-ui",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([TtsAudioFormat::Wav]),
            BTreeSet::from([TtsOutputMode::Url]),
            10_000,
        )?
        .with_voice_catalog(true)
        .with_emotions(true);
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

    /// 将统一请求映射成 ChatTTS-ui 的表单字段。
    pub fn encode_form(&self, request: &TtsRequest) -> Result<BTreeMap<String, String>, TtsError> {
        self.capability.validate_request(request)?;
        let mut form = BTreeMap::from([
            ("text".to_owned(), request.text().to_owned()),
            ("is_stream".to_owned(), "0".to_owned()),
        ]);
        if let Some(voice) = request.voice() {
            form.insert("voice".to_owned(), voice.to_owned());
        }
        if let Some(prompt) = request.emotion() {
            form.insert("prompt".to_owned(), prompt.to_owned());
        }
        Ok(form)
    }

    /// 解码 ChatTTS-ui JSON 响应，并拒绝非回环音频 URL。
    pub fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        if response.get("code").and_then(Value::as_i64) != Some(0) {
            return Err(self.invalid_response("response code is not zero"));
        }
        let audio_url = response
            .get("url")
            .and_then(Value::as_str)
            .or_else(|| {
                response
                    .pointer("/audio_files/0/url")
                    .and_then(Value::as_str)
            })
            .ok_or_else(|| self.invalid_response("missing audio URL"))?;
        validate_loopback_endpoint(audio_url)
            .map_err(|_| self.invalid_response("audio URL must use a loopback HTTP host"))?;
        Ok(CloudTtsOutput::RemoteUrl {
            url: audio_url.to_owned(),
            request_id: None,
            expires_at: None,
        })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

fn validate_loopback_endpoint(endpoint: &str) -> Result<(), TtsError> {
    let url =
        Url::parse(endpoint).map_err(|error| TtsError::InvalidLocalEndpoint(error.to_string()))?;
    if url.scheme() != "http" || !url.username().is_empty() || url.password().is_some() {
        return Err(TtsError::InvalidLocalEndpoint(
            "local endpoint must use credential-free http".to_owned(),
        ));
    }
    let loopback = match url.host() {
        Some(Host::Domain(value)) => value.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(value)) => IpAddr::V4(value).is_loopback(),
        Some(Host::Ipv6(value)) => IpAddr::V6(value).is_loopback(),
        None => false,
    };
    if !loopback {
        return Err(TtsError::InvalidLocalEndpoint(
            "local endpoint host must be loopback".to_owned(),
        ));
    }
    Ok(())
}

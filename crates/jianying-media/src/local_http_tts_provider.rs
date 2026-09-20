use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;
use url::{Host, Url};

/// 通过限定在回环地址的统一 JSON/二进制协议调用本地 TTS 服务。
#[derive(Debug)]
pub struct LocalHttpTtsProvider {
    endpoint: String,
    capability: ProviderCapability,
}

impl LocalHttpTtsProvider {
    /// 创建本地 HTTP Provider。
    ///
    /// 为避免 SSRF 和意外明文远程传输，仅接受 `http://localhost` 或回环 IP。
    pub fn new(endpoint: impl Into<String>, format: TtsAudioFormat) -> Result<Self, TtsError> {
        let endpoint = endpoint.into();
        let url = Url::parse(&endpoint)
            .map_err(|error| TtsError::InvalidLocalEndpoint(error.to_string()))?;
        if url.scheme() != "http" {
            return Err(TtsError::InvalidLocalEndpoint(
                "local endpoint must use http://".to_owned(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(TtsError::InvalidLocalEndpoint(
                "credentials are not allowed in the endpoint URL".to_owned(),
            ));
        }
        if !is_loopback(url.host()) {
            return Err(TtsError::InvalidLocalEndpoint(
                "local endpoint host must be loopback".to_owned(),
            ));
        }
        let capability = ProviderCapability::new(
            "local-http",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([format]),
            BTreeSet::from([TtsOutputMode::Binary]),
            1_000_000,
        )?
        .with_model_selection(true)
        .with_voice_catalog(true)
        .with_ssml(true)
        .with_emotions(true)
        .with_language_selection(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_pitch_control(true)
        .with_streaming(false);
        Ok(Self {
            endpoint,
            capability,
        })
    }
}

impl TtsProvider for LocalHttpTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .build()
            .new_agent();
        let mut response = agent
            .post(&self.endpoint)
            .header("Accept", mime_type(request.format()))
            .send_json(request)
            .map_err(|error| TtsError::Provider(format!("local HTTP request failed: {error}")))?;
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned();
        if !content_type.starts_with("audio/") && content_type != "application/octet-stream" {
            return Err(TtsError::Provider(format!(
                "local HTTP response is not audio: {content_type}"
            )));
        }
        let request_id = response
            .headers()
            .get("x-request-id")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let mut audio = Vec::new();
        response
            .body_mut()
            .as_reader()
            .read_to_end(&mut audio)
            .map_err(|error| {
                TtsError::Provider(format!("could not read audio response: {error}"))
            })?;
        if audio.is_empty() {
            return Err(TtsError::EmptyArtifact);
        }
        let partial = output.with_extension(format!(
            "{}.partial-{}",
            output
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("audio"),
            std::process::id()
        ));
        std::fs::write(&partial, &audio)
            .map_err(|error| TtsError::Provider(format!("could not stage audio: {error}")))?;
        if let Err(error) = std::fs::rename(&partial, output) {
            let _ = std::fs::remove_file(&partial);
            return Err(TtsError::Provider(format!(
                "could not commit audio artifact: {error}"
            )));
        }
        let mut artifact = TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            self.capability.provider_id(),
            audio.len() as u64,
        )?;
        if let Some(request_id) = request_id {
            artifact = artifact.with_request_id(request_id);
        }
        Ok(artifact)
    }
}

fn is_loopback(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(value)) => value.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(value)) => IpAddr::V4(value).is_loopback(),
        Some(Host::Ipv6(value)) => IpAddr::V6(value).is_loopback(),
        None => false,
    }
}

fn mime_type(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Wav => "audio/wav",
        TtsAudioFormat::Mp3 => "audio/mpeg",
        TtsAudioFormat::Pcm => "audio/L16",
        TtsAudioFormat::Ogg => "audio/ogg",
        TtsAudioFormat::Aac => "audio/aac",
        TtsAudioFormat::Flac => "audio/flac",
        TtsAudioFormat::Aiff => "audio/aiff",
    }
}

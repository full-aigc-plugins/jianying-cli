use crate::TtsAudioFormat;
use thiserror::Error;

/// TTS 领域、能力校验和 Provider 执行错误。
#[derive(Debug, Error)]
pub enum TtsError {
    #[error("TTS text must not be empty")]
    EmptyText,
    #[error("TTS provider id must not be empty")]
    EmptyProviderId,
    #[error("TTS credential reference must not be empty")]
    InvalidCredentialRef,
    #[error("TTS capability must declare at least one audio format")]
    MissingAudioFormat,
    #[error("TTS capability must declare at least one output mode")]
    MissingOutputMode,
    #[error("TTS maximum text length must be positive")]
    InvalidTextLimit,
    #[error("TTS {field} value {value} is outside the supported domain range")]
    InvalidScalar { field: &'static str, value: f32 },
    #[error("TTS sample rate must be positive")]
    InvalidSampleRate,
    #[error("provider {provider} does not support audio format {format:?}")]
    UnsupportedAudioFormat {
        provider: String,
        format: TtsAudioFormat,
    },
    #[error("provider {provider} text length {actual} exceeds limit {maximum}")]
    TextTooLong {
        provider: String,
        actual: usize,
        maximum: usize,
    },
    #[error("provider {provider} does not support {capability}")]
    UnsupportedCapability {
        provider: String,
        capability: &'static str,
    },
    #[error("provider {provider} does not support model {model}")]
    UnsupportedModel { provider: String, model: String },
    #[error("provider {provider} requires a credential reference")]
    MissingCredential { provider: String },
    #[error("TTS artifact output path must not be empty")]
    EmptyArtifactPath,
    #[error("TTS artifact byte length must be positive")]
    EmptyArtifact,
    #[error("invalid TTS command template: {0}")]
    InvalidCommandTemplate(String),
    #[error("invalid local TTS endpoint: {0}")]
    InvalidLocalEndpoint(String),
    #[error("invalid local TTS process: {0}")]
    InvalidLocalProcess(String),
    #[error("local TTS runtime {runtime} requires selected model artifact license review")]
    MissingModelLicenseReview { runtime: String },
    #[error("local TTS runtime {runtime} is denied for commercial use by policy {policy}")]
    CommercialUseDenied { runtime: String, policy: String },
    #[error("TTS provider failed: {0}")]
    Provider(String),
    #[error("invalid {provider} TTS fixture response: {reason}")]
    InvalidProviderResponse { provider: String, reason: String },
}

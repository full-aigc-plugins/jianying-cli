use crate::{
    AliyunTtsFamily, CloudTtsAdapter, CloudTtsOutput, ProviderCapability, TtsAudioFormat,
    TtsAuthScheme, TtsError, TtsOutputMode, TtsPlatformCondition, TtsRequest,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// 阿里云百炼 Qwen-Audio、CosyVoice 与 Qwen-TTS 协议映射器。
#[derive(Debug)]
pub struct AliyunBailianTtsAdapter {
    family: AliyunTtsFamily,
    endpoint: String,
    capability: ProviderCapability,
}

impl AliyunBailianTtsAdapter {
    /// 创建指定协议族的百炼 adapter；工作空间协议必须提供 Workspace ID。
    pub fn new(family: AliyunTtsFamily, workspace_id: Option<&str>) -> Result<Self, TtsError> {
        let endpoint = match family {
            AliyunTtsFamily::QwenAudio | AliyunTtsFamily::CosyVoice => {
                let workspace_id = workspace_id
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| {
                        TtsError::Provider(
                            "Aliyun Qwen-Audio/CosyVoice requires a Workspace ID".to_owned(),
                        )
                    })?;
                format!(
                    "https://{workspace_id}.cn-beijing.maas.aliyuncs.com/api/v1/services/audio/tts/SpeechSynthesizer"
                )
            }
            AliyunTtsFamily::QwenTts => {
                "https://dashscope.aliyuncs.com/api/v1/services/aigc/multimodal-generation/generation"
                    .to_owned()
            }
        };
        let models: &[&str] = match family {
            AliyunTtsFamily::QwenAudio => &[
                "qwen-audio-3.1-tts-flash",
                "qwen-audio-3.0-tts-plus",
                "qwen-audio-3.0-tts-flash",
            ],
            AliyunTtsFamily::CosyVoice => &[
                "cosyvoice-v3.5-plus",
                "cosyvoice-v3.5-flash",
                "cosyvoice-v3-plus",
                "cosyvoice-v3-flash",
            ],
            AliyunTtsFamily::QwenTts => &["qwen3-tts-flash", "qwen3-tts-instruct-flash"],
        };
        let capability = ProviderCapability::new(
            "aliyun-bailian",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::BearerToken,
            BTreeSet::from([TtsAudioFormat::Wav]),
            BTreeSet::from([TtsOutputMode::Url]),
            32_768,
        )?
        .with_models(models.iter().copied())
        .with_voice_catalog(true)
        .with_language_selection(matches!(family, AliyunTtsFamily::QwenTts))
        .with_emotions(matches!(family, AliyunTtsFamily::CosyVoice))
        .with_sample_rates([24_000]);
        Ok(Self {
            family,
            endpoint,
            capability,
        })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

impl CloudTtsAdapter for AliyunBailianTtsAdapter {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn endpoint(&self) -> &str {
        &self.endpoint
    }

    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        let model = request
            .model()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit model",
            })?;
        let voice = request
            .voice()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit voice",
            })?;
        let mut input = serde_json::Map::new();
        input.insert("text".to_owned(), json!(request.text()));
        input.insert("voice".to_owned(), json!(voice));
        match self.family {
            AliyunTtsFamily::QwenAudio | AliyunTtsFamily::CosyVoice => {
                input.insert("format".to_owned(), json!("wav"));
                input.insert(
                    "sample_rate".to_owned(),
                    json!(request.sample_rate_hz().unwrap_or(24_000)),
                );
                if let Some(emotion) = request.emotion() {
                    input.insert(
                        "instruction".to_owned(),
                        json!(format!("请用{emotion}的情绪表达。")),
                    );
                }
            }
            AliyunTtsFamily::QwenTts => {
                if let Some(language) = request.language() {
                    input.insert("language_type".to_owned(), json!(language));
                }
            }
        }
        Ok(json!({"model":model,"input":input}))
    }

    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        let url = response
            .pointer("/output/audio/url")
            .and_then(Value::as_str)
            .ok_or_else(|| self.invalid_response("missing output.audio.url"))?;
        if !url.starts_with("https://") {
            return Err(self.invalid_response("audio URL must use https"));
        }
        Ok(CloudTtsOutput::RemoteUrl {
            url: url.to_owned(),
            request_id: response
                .get("request_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
            expires_at: response
                .pointer("/output/audio/expires_at")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

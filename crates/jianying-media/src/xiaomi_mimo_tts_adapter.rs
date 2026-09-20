use crate::{
    CloudTtsAdapter, CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError,
    TtsOutputMode, TtsPlatformCondition, TtsRequest,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// 小米 MiMo-V2.5-TTS OpenAI 兼容协议映射器。
#[derive(Debug)]
pub struct XiaomiMimoTtsAdapter {
    capability: ProviderCapability,
}

impl XiaomiMimoTtsAdapter {
    /// 创建固定到官方 `/v1/chat/completions` 协议的 adapter。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "xiaomi-mimo",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::BearerToken,
            BTreeSet::from([TtsAudioFormat::Wav, TtsAudioFormat::Pcm]),
            BTreeSet::from([TtsOutputMode::Base64]),
            32_768,
        )?
        .with_models([
            "mimo-v2.5-tts",
            "mimo-v2.5-tts-voicedesign",
            "mimo-v2.5-tts-voiceclone",
        ])
        .with_voice_catalog(true)
        .with_voice_clone(true)
        .with_voice_design(true)
        .with_emotions(true)
        .with_streaming(true)
        .with_sample_rates([24_000]);
        Ok(Self { capability })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

impl CloudTtsAdapter for XiaomiMimoTtsAdapter {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn endpoint(&self) -> &str {
        "https://api.xiaomimimo.com/v1/chat/completions"
    }

    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        if request.streaming() && request.format() != TtsAudioFormat::Pcm {
            return Err(TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "streaming requires pcm16",
            });
        }
        let model = request
            .model()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit model",
            })?;
        let mut messages = Vec::new();
        if let Some(emotion) = request.emotion() {
            messages.push(json!({
                "role": "user",
                "content": format!("请使用 {emotion} 的情绪和语气朗读。")
            }));
        }
        messages.push(json!({"role":"assistant","content":request.text()}));
        let mut audio = serde_json::Map::new();
        audio.insert(
            "format".to_owned(),
            json!(match request.format() {
                TtsAudioFormat::Wav => "wav",
                TtsAudioFormat::Pcm => "pcm16",
                _ => unreachable!("capability validation rejects other formats"),
            }),
        );
        if let Some(voice) = request.voice() {
            audio.insert("voice".to_owned(), json!(voice));
        }
        Ok(json!({
            "model": model,
            "messages": messages,
            "audio": audio,
            "stream": request.streaming()
        }))
    }

    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        let encoded = response
            .pointer("/choices/0/message/audio/data")
            .and_then(Value::as_str)
            .ok_or_else(|| self.invalid_response("missing choices[0].message.audio.data"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| self.invalid_response(format!("invalid base64 audio: {error}")))?;
        if bytes.is_empty() {
            return Err(self.invalid_response("decoded audio is empty"));
        }
        Ok(CloudTtsOutput::Inline {
            bytes,
            request_id: response
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

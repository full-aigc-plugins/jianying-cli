use crate::{
    CloudTtsAdapter, CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError,
    TtsOutputMode, TtsPlatformCondition, TtsRequest,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// 腾讯云 TextToVoice API 3.0 协议映射器。
#[derive(Debug)]
pub struct TencentCloudTtsAdapter {
    capability: ProviderCapability,
}

impl TencentCloudTtsAdapter {
    /// 创建腾讯云基础语音合成 adapter。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "tencent-cloud",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::SignedRequest,
            BTreeSet::from([
                TtsAudioFormat::Wav,
                TtsAudioFormat::Mp3,
                TtsAudioFormat::Pcm,
            ]),
            BTreeSet::from([TtsOutputMode::Base64]),
            500,
        )?
        .with_models(["1"])
        .with_voice_catalog(true)
        .with_language_selection(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_emotions(true)
        .with_timestamps(true)
        .with_sample_rates([8_000, 16_000, 24_000]);
        Ok(Self { capability })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

impl CloudTtsAdapter for TencentCloudTtsAdapter {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn endpoint(&self) -> &str {
        "https://tts.tencentcloudapi.com"
    }

    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        let voice = request
            .voice()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit numeric voice type",
            })?
            .parse::<i64>()
            .map_err(|_| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "numeric VoiceType",
            })?;
        let language = match request.language().unwrap_or("zh-CN") {
            "zh" | "zh-CN" | "Chinese" => 1,
            "en" | "en-US" | "English" => 2,
            _ => {
                return Err(TtsError::UnsupportedCapability {
                    provider: self.capability.provider_id().to_owned(),
                    capability: "requested language",
                })
            }
        };
        let model = request.model().unwrap_or("1").parse::<i64>().map_err(|_| {
            TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "numeric ModelType",
            }
        })?;
        let mut digest = Sha256::new();
        digest.update(request.text().as_bytes());
        digest.update(voice.to_be_bytes());
        digest.update(model.to_be_bytes());
        let session_id = format!("jy-{:x}", digest.finalize());
        let mut body = serde_json::Map::new();
        body.insert("Text".to_owned(), json!(request.text()));
        body.insert("SessionId".to_owned(), json!(&session_id[..35]));
        body.insert("Volume".to_owned(), json!((request.volume() - 1.0) * 10.0));
        body.insert("Speed".to_owned(), json!(tencent_speed(request.speed())));
        body.insert("ModelType".to_owned(), json!(model));
        body.insert("VoiceType".to_owned(), json!(voice));
        body.insert("PrimaryLanguage".to_owned(), json!(language));
        body.insert(
            "SampleRate".to_owned(),
            json!(request.sample_rate_hz().unwrap_or(16_000)),
        );
        body.insert("Codec".to_owned(), json!(format_name(request.format())));
        body.insert("EnableSubtitle".to_owned(), json!(request.timestamps()));
        if let Some(emotion) = request.emotion() {
            body.insert("EmotionCategory".to_owned(), json!(emotion));
        }
        Ok(Value::Object(body))
    }

    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        let encoded = response
            .pointer("/Response/Audio")
            .and_then(Value::as_str)
            .ok_or_else(|| self.invalid_response("missing Response.Audio"))?;
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|error| self.invalid_response(format!("invalid base64 audio: {error}")))?;
        if bytes.is_empty() {
            return Err(self.invalid_response("decoded audio is empty"));
        }
        Ok(CloudTtsOutput::Inline {
            bytes,
            request_id: response
                .pointer("/Response/RequestId")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

fn format_name(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Wav => "wav",
        TtsAudioFormat::Mp3 => "mp3",
        TtsAudioFormat::Pcm => "pcm",
        _ => unreachable!("capability validation rejects other formats"),
    }
}

fn tencent_speed(speed: f32) -> f32 {
    const POINTS: [(f32, f32); 6] = [
        (0.6, -2.0),
        (0.8, -1.0),
        (1.0, 0.0),
        (1.2, 1.0),
        (1.5, 2.0),
        (2.5, 6.0),
    ];
    if speed <= POINTS[0].0 {
        return POINTS[0].1;
    }
    for pair in POINTS.windows(2) {
        let (left_speed, left_value) = pair[0];
        let (right_speed, right_value) = pair[1];
        if speed <= right_speed {
            let ratio = (speed - left_speed) / (right_speed - left_speed);
            return left_value + ratio * (right_value - left_value);
        }
    }
    POINTS[POINTS.len() - 1].1
}

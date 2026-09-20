use crate::{
    CloudTtsAdapter, CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError,
    TtsOutputMode, TtsPlatformCondition, TtsRequest,
};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// MiniMax 同步语音合成 T2A V2 JSON/hex 协议映射器。
#[derive(Debug)]
pub struct MiniMaxTtsAdapter {
    capability: ProviderCapability,
}

impl MiniMaxTtsAdapter {
    /// 创建固定到中国区官方 T2A V2 endpoint 的 adapter。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "minimax",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::BearerToken,
            BTreeSet::from([
                TtsAudioFormat::Mp3,
                TtsAudioFormat::Pcm,
                TtsAudioFormat::Flac,
                TtsAudioFormat::Wav,
                TtsAudioFormat::Ogg,
            ]),
            BTreeSet::from([TtsOutputMode::Hex]),
            9_999,
        )?
        .with_models([
            "speech-2.8-hd",
            "speech-2.8-turbo",
            "speech-2.6-hd",
            "speech-2.6-turbo",
            "speech-02-hd",
            "speech-02-turbo",
            "speech-01-hd",
            "speech-01-turbo",
        ])
        .with_voice_catalog(true)
        .with_emotions(true)
        .with_language_selection(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_pitch_control(true)
        .with_sample_rates([8_000, 16_000, 22_050, 24_000, 32_000, 44_100])
        .with_timestamps(true)
        .with_streaming(true);
        Ok(Self { capability })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

impl CloudTtsAdapter for MiniMaxTtsAdapter {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn endpoint(&self) -> &str {
        "https://api.minimax.cn/v1/t2a_v2"
    }

    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        if !(0.5..=2.0).contains(&request.speed()) {
            return Err(TtsError::InvalidScalar {
                field: "minimax.voice_setting.speed",
                value: request.speed(),
            });
        }
        if request.volume() <= 0.0 {
            return Err(TtsError::InvalidScalar {
                field: "minimax.voice_setting.vol",
                value: request.volume(),
            });
        }
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
                capability: "explicit voice_id",
            })?;
        let mut voice_setting = serde_json::Map::from_iter([
            ("voice_id".to_owned(), json!(voice)),
            ("speed".to_owned(), json!(request.speed())),
            ("vol".to_owned(), json!(request.volume())),
            (
                "pitch".to_owned(),
                json!((request.pitch() * 12.0).round() as i32),
            ),
        ]);
        if let Some(emotion) = request.emotion() {
            voice_setting.insert("emotion".to_owned(), json!(emotion));
        }
        let mut body = serde_json::Map::from_iter([
            ("model".to_owned(), json!(model)),
            ("text".to_owned(), json!(request.text())),
            ("stream".to_owned(), json!(request.streaming())),
            ("voice_setting".to_owned(), Value::Object(voice_setting)),
            (
                "audio_setting".to_owned(),
                json!({
                    "sample_rate": request.sample_rate_hz().unwrap_or(32_000),
                    "bitrate": 128_000,
                    "format": format_name(request.format()),
                    "channel": 1
                }),
            ),
            ("subtitle_enable".to_owned(), json!(request.timestamps())),
            ("output_format".to_owned(), json!("hex")),
        ]);
        if let Some(language) = request.language() {
            body.insert("language_boost".to_owned(), json!(language));
        }
        Ok(Value::Object(body))
    }

    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        let status_code = response
            .pointer("/base_resp/status_code")
            .and_then(Value::as_i64)
            .ok_or_else(|| self.invalid_response("missing base_resp.status_code"))?;
        if status_code != 0 {
            let message = response
                .pointer("/base_resp/status_msg")
                .and_then(Value::as_str)
                .unwrap_or("unknown provider error");
            return Err(self.invalid_response(format!("provider status {status_code}: {message}")));
        }
        let encoded = response
            .pointer("/data/audio")
            .and_then(Value::as_str)
            .ok_or_else(|| self.invalid_response("missing data.audio"))?;
        let bytes = decode_hex(encoded).map_err(|reason| self.invalid_response(reason))?;
        if bytes.is_empty() {
            return Err(self.invalid_response("decoded audio is empty"));
        }
        Ok(CloudTtsOutput::Inline {
            bytes,
            request_id: response
                .get("trace_id")
                .and_then(Value::as_str)
                .map(str::to_owned),
        })
    }
}

fn format_name(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Mp3 => "mp3",
        TtsAudioFormat::Pcm => "pcm",
        TtsAudioFormat::Flac => "flac",
        TtsAudioFormat::Wav => "wav",
        TtsAudioFormat::Ogg => "opus",
        _ => unreachable!("capability validation rejects other formats"),
    }
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("hex audio length must be even".to_owned());
    }
    let (pairs, remainder) = value.as_bytes().as_chunks::<2>();
    debug_assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair).map_err(|error| error.to_string())?;
            u8::from_str_radix(text, 16).map_err(|error| format!("invalid hex audio: {error}"))
        })
        .collect()
}

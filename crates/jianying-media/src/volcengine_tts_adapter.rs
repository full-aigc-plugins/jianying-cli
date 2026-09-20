use crate::{
    CloudTtsAdapter, CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError,
    TtsInputKind, TtsOutputMode, TtsPlatformCondition, TtsRequest,
};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// 火山引擎 V3 单向 SSE 语音合成协议映射器。
#[derive(Debug)]
pub struct VolcengineTtsAdapter {
    resource_id: String,
    uid: String,
    capability: ProviderCapability,
}

impl VolcengineTtsAdapter {
    /// 创建 V3 SSE adapter；API key 与资源 ID 由执行器分别写入专用请求头。
    pub fn new(resource_id: impl Into<String>, uid: impl Into<String>) -> Result<Self, TtsError> {
        let resource_id = required_config("resource_id", resource_id.into())?;
        let uid = required_config("uid", uid.into())?;
        let capability = ProviderCapability::new(
            "volcengine",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::ApiKey,
            BTreeSet::from([
                TtsAudioFormat::Mp3,
                TtsAudioFormat::Wav,
                TtsAudioFormat::Pcm,
                TtsAudioFormat::Ogg,
            ]),
            BTreeSet::from([TtsOutputMode::Base64]),
            1_024,
        )?
        .with_voice_catalog(true)
        .with_ssml(true)
        .with_streaming(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_pitch_control(true)
        .with_sample_rates([8_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000]);
        Ok(Self {
            resource_id,
            uid,
            capability,
        })
    }

    /// 返回必须写入 `X-Api-Resource-Id` 的资源标识。
    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }

    /// 解码 V3 SSE 或逐行 JSON 响应，并按到达顺序拼接所有音频分片。
    pub fn decode_stream_response(
        &self,
        response: &[u8],
        request_id: Option<&str>,
    ) -> Result<CloudTtsOutput, TtsError> {
        let text = std::str::from_utf8(response)
            .map_err(|error| self.invalid_response(format!("response is not UTF-8: {error}")))?;
        let mut audio = Vec::new();
        let mut frames = 0_u64;
        let mut terminal_seen = false;

        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(':') || line.starts_with("event:") {
                continue;
            }
            let payload = line.strip_prefix("data:").map(str::trim).unwrap_or(line);
            if payload.is_empty() || payload == "[DONE]" {
                continue;
            }
            let frame: Value = serde_json::from_str(payload).map_err(|error| {
                self.invalid_response(format!("invalid V3 stream frame: {error}"))
            })?;
            frames += 1;
            match frame.get("code").and_then(Value::as_i64) {
                Some(0) => {
                    if let Some(encoded) = frame.get("data").and_then(Value::as_str) {
                        let fragment = STANDARD.decode(encoded).map_err(|error| {
                            self.invalid_response(format!("invalid base64 audio: {error}"))
                        })?;
                        audio.extend_from_slice(&fragment);
                    }
                }
                Some(20_000_000) => terminal_seen = true,
                Some(code) => {
                    let message = frame
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown provider error");
                    return Err(self.invalid_response(format!("provider code {code}: {message}")));
                }
                None => return Err(self.invalid_response("missing numeric code in V3 frame")),
            }
        }

        if frames == 0 {
            return Err(self.invalid_response("V3 response contained no JSON frames"));
        }
        if !terminal_seen {
            return Err(self.invalid_response("V3 response ended before terminal frame"));
        }
        if audio.is_empty() {
            return Err(self.invalid_response("V3 response contained no audio data"));
        }
        Ok(CloudTtsOutput::Inline {
            bytes: audio,
            request_id: request_id.map(str::to_owned),
        })
    }

    fn invalid_response(&self, reason: impl Into<String>) -> TtsError {
        TtsError::InvalidProviderResponse {
            provider: self.capability.provider_id().to_owned(),
            reason: reason.into(),
        }
    }
}

impl CloudTtsAdapter for VolcengineTtsAdapter {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn endpoint(&self) -> &str {
        "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse"
    }

    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        self.capability.validate_request(request)?;
        if !(0.5..=2.0).contains(&request.speed()) {
            return Err(TtsError::InvalidScalar {
                field: "volcengine.speech_rate",
                value: request.speed(),
            });
        }
        if !(0.5..=2.0).contains(&request.volume()) {
            return Err(TtsError::InvalidScalar {
                field: "volcengine.loudness_rate",
                value: request.volume(),
            });
        }
        let voice = request
            .voice()
            .ok_or_else(|| TtsError::UnsupportedCapability {
                provider: self.capability.provider_id().to_owned(),
                capability: "explicit speaker",
            })?;
        let additions = serde_json::to_string(&json!({
            "post_process": {"pitch": (request.pitch().clamp(-1.0, 1.0) * 12.0).round() as i32}
        }))
        .map_err(|error| TtsError::Provider(error.to_string()))?;
        let mut req_params = Map::from_iter([
            ("text".to_owned(), json!(request.text())),
            ("speaker".to_owned(), json!(voice)),
            (
                "sample_rate".to_owned(),
                json!(request.sample_rate_hz().unwrap_or(24_000)),
            ),
            (
                "audio_params".to_owned(),
                json!({
                    "format": format_name(request.format()),
                    "speech_rate": scalar_percent(request.speed(), 0.5, 2.0, 100.0),
                    "loudness_rate": scalar_percent(request.volume(), 0.5, 2.0, 100.0)
                }),
            ),
            ("additions".to_owned(), json!(additions)),
        ]);
        if request.input_kind() == TtsInputKind::Ssml {
            req_params.insert("text_type".to_owned(), json!("ssml"));
        }
        Ok(json!({
            "user": {"uid": self.uid},
            "req_params": req_params
        }))
    }

    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError> {
        let line = format!("data: {response}\n\ndata: {{\"code\":20000000}}\n\n");
        self.decode_stream_response(line.as_bytes(), None)
    }
}

fn required_config(name: &str, value: String) -> Result<String, TtsError> {
    if value.trim().is_empty() {
        return Err(TtsError::Provider(format!(
            "volcengine {name} must not be empty"
        )));
    }
    Ok(value)
}

fn format_name(format: TtsAudioFormat) -> &'static str {
    match format {
        TtsAudioFormat::Mp3 => "mp3",
        TtsAudioFormat::Wav => "wav",
        TtsAudioFormat::Pcm => "pcm",
        TtsAudioFormat::Ogg => "ogg_opus",
        _ => unreachable!("capability validation rejects other formats"),
    }
}

fn scalar_percent(value: f32, minimum: f32, maximum: f32, scale: f32) -> i32 {
    ((value.clamp(minimum, maximum) - 1.0) * scale).round() as i32
}

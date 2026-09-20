use crate::{
    CloudTtsOutput, ProviderCapability, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsRequest,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// 百度智能云短文本在线合成 form/binary 协议映射器。
#[derive(Debug)]
pub struct BaiduTtsAdapter {
    cuid: String,
    capability: ProviderCapability,
}

impl BaiduTtsAdapter {
    /// 创建短文本在线合成 adapter；access token 由执行器注入 `tok` 字段。
    pub fn new(cuid: impl Into<String>) -> Result<Self, TtsError> {
        let cuid = cuid.into();
        if cuid.trim().is_empty() || cuid.chars().count() > 60 {
            return Err(TtsError::Provider(
                "baidu cuid must contain 1 to 60 characters".to_owned(),
            ));
        }
        let capability = ProviderCapability::new(
            "baidu",
            true,
            TtsPlatformCondition::Any,
            TtsAuthScheme::OAuthToken,
            BTreeSet::from([
                TtsAudioFormat::Mp3,
                TtsAudioFormat::Pcm,
                TtsAudioFormat::Wav,
            ]),
            BTreeSet::from([TtsOutputMode::Binary]),
            512,
        )?
        .with_voice_catalog(true)
        .with_language_selection(true)
        .with_speed_control(true)
        .with_volume_control(true)
        .with_pitch_control(true)
        .with_emotions(true)
        .with_sample_rates([8_000, 16_000, 24_000]);
        Ok(Self { cuid, capability })
    }

    /// 返回百度短文本在线合成 endpoint。
    pub fn endpoint(&self) -> &str {
        "https://tsn.baidu.com/text2audio"
    }

    /// 返回能力声明。
    pub fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    /// 生成除 `tok` 外的 URL-encoded form 字段，避免凭据引用进入序列化正文。
    pub fn encode_form(&self, request: &TtsRequest) -> Result<BTreeMap<String, String>, TtsError> {
        self.capability.validate_request(request)?;
        if let Some(language) = request.language() {
            if language != "zh" && language != "zh-CN" {
                return Err(TtsError::UnsupportedCapability {
                    provider: self.capability.provider_id().to_owned(),
                    capability: "requested language",
                });
            }
        }
        let mut form = BTreeMap::from([
            ("tex".to_owned(), request.text().to_owned()),
            ("cuid".to_owned(), self.cuid.clone()),
            ("ctp".to_owned(), "1".to_owned()),
            ("lan".to_owned(), "zh".to_owned()),
            ("spd".to_owned(), speed_value(request.speed()).to_string()),
            ("pit".to_owned(), pitch_value(request.pitch()).to_string()),
            ("vol".to_owned(), volume_value(request.volume()).to_string()),
            (
                "aue".to_owned(),
                audio_encoding(request.format(), request.sample_rate_hz()).to_owned(),
            ),
        ]);
        if let Some(voice) = request.voice() {
            if voice.parse::<u32>().is_err() {
                return Err(TtsError::UnsupportedCapability {
                    provider: self.capability.provider_id().to_owned(),
                    capability: "numeric per voice id",
                });
            }
            form.insert("per".to_owned(), voice.to_owned());
        }
        if let Some(emotion) = request.emotion() {
            form.insert("text_ctrl".to_owned(), json!({"emo": emotion}).to_string());
        }
        if let Some(sample_rate_hz) = request.sample_rate_hz() {
            form.insert(
                "audio_ctrl".to_owned(),
                json!({"sampling_rate": sample_rate_hz}).to_string(),
            );
        }
        Ok(form)
    }

    /// 按 Content-Type 区分成功音频和 JSON 错误响应。
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

fn audio_encoding(format: TtsAudioFormat, sample_rate_hz: Option<u32>) -> &'static str {
    match (format, sample_rate_hz) {
        (TtsAudioFormat::Mp3, _) => "3",
        (TtsAudioFormat::Pcm, Some(8_000)) => "5",
        (TtsAudioFormat::Pcm, _) => "4",
        (TtsAudioFormat::Wav, _) => "6",
        _ => unreachable!("capability validation rejects other formats"),
    }
}

fn speed_value(speed: f32) -> u8 {
    (speed * 5.0).round().clamp(0.0, 15.0) as u8
}

fn pitch_value(pitch: f32) -> u8 {
    if pitch < 0.0 {
        (5.0 + pitch * 5.0).round() as u8
    } else {
        (5.0 + pitch * 10.0).round() as u8
    }
}

fn volume_value(volume: f32) -> u8 {
    if volume < 1.0 {
        (volume * 5.0).round() as u8
    } else {
        (5.0 + (volume - 1.0) * 10.0).round() as u8
    }
}

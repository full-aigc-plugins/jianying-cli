use jianying_media::{
    CredentialRef, CredentialSource, ProviderCapability, TtsArtifact, TtsAudioFormat,
    TtsAuthScheme, TtsInputKind, TtsOutputMode, TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::path::Path;

fn formats(values: &[TtsAudioFormat]) -> BTreeSet<TtsAudioFormat> {
    values.iter().copied().collect()
}

fn output_modes(values: &[TtsOutputMode]) -> BTreeSet<TtsOutputMode> {
    values.iter().copied().collect()
}

#[test]
fn request_and_capability_round_trip_without_secret_values() {
    let credential = CredentialRef::new(CredentialSource::Environment, "XIAOMI_API_KEY")
        .expect("环境变量引用应有效");
    let capability = ProviderCapability::new(
        "xiaomi-mimo",
        true,
        TtsPlatformCondition::Any,
        TtsAuthScheme::BearerToken,
        formats(&[TtsAudioFormat::Wav, TtsAudioFormat::Pcm]),
        output_modes(&[TtsOutputMode::Binary, TtsOutputMode::Base64]),
        10_000,
    )
    .expect("能力声明应有效")
    .with_models(["mimo-v2.5-tts"])
    .with_voice_catalog(true)
    .with_voice_design(true)
    .with_voice_clone(true)
    .with_emotions(true)
    .with_language_selection(true)
    .with_speed_control(true)
    .with_volume_control(true)
    .with_pitch_control(true)
    .with_sample_rates([24_000])
    .with_timestamps(true)
    .with_streaming(true);

    let request = TtsRequest::new("你好，剪映", TtsAudioFormat::Pcm)
        .expect("请求应有效")
        .with_input_kind(TtsInputKind::Text)
        .with_model("mimo-v2.5-tts")
        .with_voice("default")
        .with_language("zh-CN")
        .with_speed(1.1)
        .expect("语速应有效")
        .with_volume(0.8)
        .expect("音量应有效")
        .with_pitch(0.2)
        .expect("音高应有效")
        .with_emotion("happy")
        .with_sample_rate_hz(24_000)
        .expect("采样率应有效")
        .with_streaming(true)
        .with_timestamps(true)
        .with_credential(credential);

    capability
        .validate_request(&request)
        .expect("请求应匹配能力声明");
    let encoded = serde_json::to_string(&request).expect("请求应可序列化");
    assert!(encoded.contains("XIAOMI_API_KEY"));
    assert!(!encoded.contains("secret-value"));
    assert_eq!(
        serde_json::from_str::<TtsRequest>(&encoded).expect("请求应可反序列化"),
        request
    );
}

#[test]
fn capability_rejects_unsupported_or_oversized_requests() {
    let capability = ProviderCapability::new(
        "system-macos",
        false,
        TtsPlatformCondition::MacOs,
        TtsAuthScheme::None,
        formats(&[TtsAudioFormat::Aiff]),
        output_modes(&[TtsOutputMode::File]),
        20,
    )
    .expect("能力声明应有效");

    let unsupported = TtsRequest::new("<speak>你好</speak>", TtsAudioFormat::Wav)
        .expect("请求应有效")
        .with_input_kind(TtsInputKind::Ssml)
        .with_streaming(true)
        .with_timestamps(true);
    let error = capability
        .validate_request(&unsupported)
        .expect_err("不支持的能力必须拒绝");
    assert!(error.to_string().contains("audio format"));

    let oversized = TtsRequest::new(
        "这是一段明显超过二十个字符的语音合成输入文本",
        TtsAudioFormat::Aiff,
    )
    .expect("请求应有效");
    assert!(capability
        .validate_request(&oversized)
        .expect_err("超长文本必须拒绝")
        .to_string()
        .contains("text length"));
}

struct FixtureProvider {
    capability: ProviderCapability,
}

impl TtsProvider for FixtureProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(
        &self,
        request: &TtsRequest,
        output: &Path,
    ) -> Result<TtsArtifact, jianying_media::TtsError> {
        self.capability.validate_request(request)?;
        TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            self.capability.provider_id(),
            128,
        )
    }
}

#[test]
fn provider_trait_returns_auditable_artifact() {
    let provider = FixtureProvider {
        capability: ProviderCapability::new(
            "fixture",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            formats(&[TtsAudioFormat::Wav]),
            output_modes(&[TtsOutputMode::File]),
            1_000,
        )
        .expect("能力声明应有效"),
    };
    let request = TtsRequest::new("contract", TtsAudioFormat::Wav).expect("请求应有效");
    let artifact = provider
        .synthesize(&request, Path::new("voice.wav"))
        .expect("fixture provider 应返回产物");

    assert_eq!(artifact.provider_id(), "fixture");
    assert_eq!(artifact.byte_length(), 128);
    assert_eq!(artifact.format(), TtsAudioFormat::Wav);
}

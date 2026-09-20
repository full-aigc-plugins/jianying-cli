use jianying_media::{
    AliyunBailianTtsAdapter, AliyunTtsFamily, CloudTtsAdapter, CredentialRef, CredentialSource,
    TtsAudioFormat, TtsRequest,
};
use serde_json::json;

fn credential() -> CredentialRef {
    CredentialRef::new(CredentialSource::Environment, "DASHSCOPE_API_KEY").unwrap()
}

#[test]
fn aliyun_keeps_workspace_and_dashscope_endpoints_distinct() {
    let cosy = AliyunBailianTtsAdapter::new(AliyunTtsFamily::CosyVoice, Some("ws-123"))
        .expect("CosyVoice adapter 应有效");
    let cosy_request = TtsRequest::new("百炼语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("cosyvoice-v3-flash")
        .with_voice("longanyang")
        .with_emotion("开心")
        .with_sample_rate_hz(24_000)
        .unwrap()
        .with_credential(credential());
    let cosy_payload = cosy.encode_request(&cosy_request).unwrap();
    assert!(cosy
        .endpoint()
        .contains("ws-123.cn-beijing.maas.aliyuncs.com"));
    assert_eq!(cosy_payload["input"]["sample_rate"], 24_000);
    assert!(cosy_payload["input"]["instruction"]
        .as_str()
        .unwrap()
        .contains("开心"));

    let qwen = AliyunBailianTtsAdapter::new(AliyunTtsFamily::QwenTts, None).unwrap();
    let qwen_request = TtsRequest::new("Qwen TTS", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("qwen3-tts-flash")
        .with_voice("Cherry")
        .with_language("Chinese")
        .with_credential(credential());
    let qwen_payload = qwen.encode_request(&qwen_request).unwrap();
    assert!(qwen.endpoint().contains("dashscope.aliyuncs.com"));
    assert_eq!(qwen_payload["input"]["language_type"], "Chinese");
    assert!(qwen_payload["input"].get("sample_rate").is_none());
}

#[test]
fn aliyun_decodes_expiring_remote_audio_url() {
    let adapter = AliyunBailianTtsAdapter::new(AliyunTtsFamily::QwenTts, None).unwrap();
    let output = adapter
        .decode_response(&json!({
            "request_id":"aliyun-42",
            "output":{"audio":{
                "url":"https://fixtures.example/audio.wav",
                "expires_at":"2030-01-01T00:00:00Z"
            }}
        }))
        .unwrap();
    assert_eq!(
        output.remote_url(),
        Some("https://fixtures.example/audio.wav")
    );
    assert_eq!(output.request_id(), Some("aliyun-42"));
}

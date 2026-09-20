use jianying_media::{
    CloudTtsAdapter, CredentialRef, CredentialSource, TencentCloudTtsAdapter, TtsAudioFormat,
    TtsRequest,
};
use serde_json::json;

#[test]
fn tencent_maps_text_to_voice_contract_and_decodes_audio() {
    let adapter = TencentCloudTtsAdapter::new().unwrap();
    let request = TtsRequest::new("腾讯语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("1")
        .with_voice("1001")
        .with_language("zh-CN")
        .with_speed(1.2)
        .unwrap()
        .with_volume(1.1)
        .unwrap()
        .with_emotion("happy")
        .with_sample_rate_hz(16_000)
        .unwrap()
        .with_timestamps(true)
        .with_credential(
            CredentialRef::new(CredentialSource::SecretProvider, "tencent-tts").unwrap(),
        );

    let payload = adapter.encode_request(&request).unwrap();

    assert_eq!(adapter.endpoint(), "https://tts.tencentcloudapi.com");
    assert_eq!(payload["Text"], "腾讯语音");
    assert_eq!(payload["VoiceType"], 1001);
    assert_eq!(payload["Speed"], 1.0);
    assert_eq!(payload["EnableSubtitle"], true);
    assert!(payload["SessionId"].as_str().unwrap().starts_with("jy-"));
    assert!(payload.to_string().find("tencent-tts").is_none());

    let output = adapter
        .decode_response(&json!({"Response":{
            "Audio":"UklGRmZha2U=",
            "RequestId":"tencent-42",
            "SessionId":payload["SessionId"]
        }}))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(&b"RIFFfake"[..]));
    assert_eq!(output.request_id(), Some("tencent-42"));
}

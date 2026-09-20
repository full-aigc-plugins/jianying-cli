use jianying_media::{
    CloudTtsAdapter, CredentialRef, CredentialSource, TtsAudioFormat, TtsRequest,
    XiaomiMimoTtsAdapter,
};
use serde_json::json;

#[test]
fn xiaomi_maps_text_to_assistant_message_and_decodes_base64() {
    let adapter = XiaomiMimoTtsAdapter::new().unwrap();
    let request = TtsRequest::new("小米语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("mimo-v2.5-tts")
        .with_voice("冰糖")
        .with_emotion("开心")
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "MIMO_API_KEY").unwrap(),
        );

    let payload = adapter.encode_request(&request).unwrap();

    assert_eq!(
        adapter.endpoint(),
        "https://api.xiaomimimo.com/v1/chat/completions"
    );
    assert_eq!(payload["messages"][1]["role"], "assistant");
    assert_eq!(payload["messages"][1]["content"], "小米语音");
    assert_eq!(payload["audio"]["voice"], "冰糖");
    assert_eq!(payload["audio"]["format"], "wav");
    assert!(payload.to_string().find("MIMO_API_KEY").is_none());

    let output = adapter
        .decode_response(&json!({
            "id":"mimo-request-1",
            "choices":[{"message":{"audio":{"data":"UklGRmZha2U="}}}]
        }))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(&b"RIFFfake"[..]));
    assert_eq!(output.request_id(), Some("mimo-request-1"));
}

#[test]
fn xiaomi_streaming_requires_pcm16() {
    let adapter = XiaomiMimoTtsAdapter::new().unwrap();
    let request = TtsRequest::new("stream", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("mimo-v2.5-tts")
        .with_streaming(true)
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "MIMO_API_KEY").unwrap(),
        );
    assert!(adapter
        .encode_request(&request)
        .expect_err("流式 WAV 必须拒绝")
        .to_string()
        .contains("pcm16"));
}

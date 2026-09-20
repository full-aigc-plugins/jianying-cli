use jianying_media::{
    CredentialRef, CredentialSource, TtsAudioFormat, TtsRequest, ZhipuGlmTtsAdapter,
};

#[test]
fn zhipu_maps_glm_tts_binary_contract_without_serializing_credential_reference() {
    let adapter = ZhipuGlmTtsAdapter::new().unwrap();
    let request = TtsRequest::new("智谱语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_model("glm-tts")
        .with_voice("tongtong")
        .with_speed(1.25)
        .unwrap()
        .with_volume(1.5)
        .unwrap()
        .with_credential(
            CredentialRef::new(CredentialSource::SecretProvider, "zhipu-glm-tts").unwrap(),
        );

    let payload = adapter.encode_request(&request).unwrap();

    assert_eq!(
        adapter.endpoint(),
        "https://open.bigmodel.cn/api/paas/v4/audio/speech"
    );
    assert_eq!(payload["model"], "glm-tts");
    assert_eq!(payload["input"], "智谱语音");
    assert_eq!(payload["voice"], "tongtong");
    assert_eq!(payload["response_format"], "wav");
    assert_eq!(payload["speed"], 1.25);
    assert_eq!(payload["volume"], 1.5);
    assert!(payload.to_string().find("zhipu-glm-tts").is_none());

    let output = adapter
        .decode_audio(b"fixture-wav", "audio/wav", Some("zhipu-42"))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(b"fixture-wav".as_slice()));
    assert_eq!(output.request_id(), Some("zhipu-42"));
}

#[test]
fn zhipu_enforces_streaming_pcm_and_binary_content_type() {
    let adapter = ZhipuGlmTtsAdapter::new().unwrap();
    let request = TtsRequest::new("流式", TtsAudioFormat::Wav)
        .unwrap()
        .with_voice("tongtong")
        .with_streaming(true)
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "ZHIPU_API_KEY").unwrap(),
        );
    assert!(adapter.encode_request(&request).is_err());
    assert!(adapter
        .decode_audio(br#"{"error":{"message":"bad"}}"#, "application/json", None)
        .is_err());
}

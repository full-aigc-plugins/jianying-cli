use jianying_media::{EmotiVoiceTtsAdapter, TtsAudioFormat, TtsRequest};

#[test]
fn emotivoice_maps_the_local_openai_audio_contract() {
    let adapter = EmotiVoiceTtsAdapter::new("http://127.0.0.1:8000/v1/audio/speech").unwrap();
    let request = TtsRequest::new("带有情绪的本地语音", TtsAudioFormat::Mp3)
        .unwrap()
        .with_model("tts-1")
        .with_voice("alloy")
        .with_speed(1.25)
        .unwrap();

    let payload = adapter.encode_request(&request).unwrap();
    assert_eq!(payload["model"], "tts-1");
    assert_eq!(payload["input"], "带有情绪的本地语音");
    assert_eq!(payload["voice"], "alloy");
    assert_eq!(payload["response_format"], "mp3");
    assert_eq!(payload["speed"], 1.25);
}

#[test]
fn emotivoice_validates_binary_audio_without_network_io() {
    let adapter = EmotiVoiceTtsAdapter::new("http://localhost:8000/v1/audio/speech").unwrap();
    let output = adapter
        .decode_audio(b"fixture-mp3", "audio/mpeg", Some("emoti-42"))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(b"fixture-mp3".as_slice()));
    assert_eq!(output.request_id(), Some("emoti-42"));

    assert!(adapter
        .decode_audio(br#"{"error":"boom"}"#, "application/json", None)
        .is_err());
}

#[test]
fn emotivoice_rejects_non_loopback_endpoint() {
    assert!(EmotiVoiceTtsAdapter::new("https://example.com/v1/audio/speech").is_err());
}

use jianying_media::{
    CloudTtsAdapter, CredentialRef, CredentialSource, TtsAudioFormat, TtsInputKind, TtsRequest,
    VolcengineTtsAdapter,
};
use serde_json::json;

#[test]
fn volcengine_maps_v3_sse_contract_without_serializing_credential_reference() {
    let adapter = VolcengineTtsAdapter::new("seed-tts-2.0", "jianying-cli").unwrap();
    let request = TtsRequest::new("<speak>火山语音</speak>", TtsAudioFormat::Mp3)
        .unwrap()
        .with_input_kind(TtsInputKind::Ssml)
        .with_voice("zh_female_fixture")
        .with_speed(1.25)
        .unwrap()
        .with_volume(1.2)
        .unwrap()
        .with_pitch(0.5)
        .unwrap()
        .with_sample_rate_hz(24_000)
        .unwrap()
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "VOLCENGINE_TTS_API_KEY").unwrap(),
        );

    let payload = adapter.encode_request(&request).unwrap();

    assert_eq!(
        adapter.endpoint(),
        "https://openspeech.bytedance.com/api/v3/tts/unidirectional/sse"
    );
    assert_eq!(adapter.resource_id(), "seed-tts-2.0");
    assert_eq!(payload["user"]["uid"], "jianying-cli");
    assert_eq!(payload["req_params"]["speaker"], "zh_female_fixture");
    assert_eq!(payload["req_params"]["sample_rate"], 24_000);
    assert_eq!(payload["req_params"]["audio_params"]["format"], "mp3");
    assert_eq!(payload["req_params"]["audio_params"]["speech_rate"], 25);
    assert_eq!(payload["req_params"]["audio_params"]["loudness_rate"], 20);
    assert_eq!(payload["req_params"]["text_type"], "ssml");
    let additions: serde_json::Value =
        serde_json::from_str(payload["req_params"]["additions"].as_str().unwrap()).unwrap();
    assert_eq!(additions["post_process"]["pitch"], 6);
    assert!(!payload.to_string().contains("VOLCENGINE_TTS_API_KEY"));
}

#[test]
fn volcengine_decodes_v3_sse_frames_and_rejects_provider_errors() {
    let adapter = VolcengineTtsAdapter::new("seed-tts-2.0", "jianying-cli").unwrap();
    let output = adapter
        .decode_stream_response(
            b"data: {\"code\":0,\"data\":\"UklG\"}\n\ndata: {\"code\":0,\"data\":\"RmZha2U=\"}\n\ndata: {\"code\":20000000,\"message\":\"done\"}\n\n",
            Some("volc-v3-request-42"),
        )
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(&b"RIFFfake"[..]));
    assert_eq!(output.request_id(), Some("volc-v3-request-42"));

    assert!(adapter
        .decode_stream_response(
            b"data: {\"code\":55000000,\"message\":\"invalid speaker\"}\n\n",
            None,
        )
        .is_err());
    assert!(adapter
        .decode_response(&json!({"code": 55000000, "message": "busy"}))
        .is_err());
}

#[test]
fn volcengine_rejects_provider_range_and_empty_resource_identity() {
    assert!(VolcengineTtsAdapter::new("", "jianying-cli").is_err());
    let adapter = VolcengineTtsAdapter::new("seed-tts-2.0", "jianying-cli").unwrap();
    let request = TtsRequest::new("过快", TtsAudioFormat::Wav)
        .unwrap()
        .with_voice("fixture")
        .with_speed(3.0)
        .unwrap()
        .with_credential(
            CredentialRef::new(CredentialSource::SecretProvider, "volcengine-tts").unwrap(),
        );
    assert!(adapter.encode_request(&request).is_err());
}

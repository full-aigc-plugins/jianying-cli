use jianying_media::{
    CloudTtsAdapter, CredentialRef, CredentialSource, MiniMaxTtsAdapter, TtsAudioFormat, TtsRequest,
};
use serde_json::json;

#[test]
fn minimax_maps_t2a_v2_and_decodes_hex_audio() {
    let adapter = MiniMaxTtsAdapter::new().unwrap();
    let request = TtsRequest::new("MiniMax 语音", TtsAudioFormat::Mp3)
        .unwrap()
        .with_model("speech-2.8-hd")
        .with_voice("male-qn-qingse")
        .with_language("Chinese")
        .with_speed(1.1)
        .unwrap()
        .with_volume(1.2)
        .unwrap()
        .with_pitch(0.5)
        .unwrap()
        .with_emotion("happy")
        .with_sample_rate_hz(32_000)
        .unwrap()
        .with_timestamps(true)
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "MINIMAX_API_KEY").unwrap(),
        );

    let payload = adapter.encode_request(&request).unwrap();

    assert_eq!(adapter.endpoint(), "https://api.minimax.cn/v1/t2a_v2");
    assert_eq!(payload["model"], "speech-2.8-hd");
    assert_eq!(payload["voice_setting"]["voice_id"], "male-qn-qingse");
    assert_eq!(payload["voice_setting"]["pitch"], 6);
    assert_eq!(payload["audio_setting"]["sample_rate"], 32_000);
    assert_eq!(payload["audio_setting"]["format"], "mp3");
    assert_eq!(payload["language_boost"], "Chinese");
    assert_eq!(payload["subtitle_enable"], true);
    assert_eq!(payload["output_format"], "hex");
    assert!(payload.to_string().find("MINIMAX_API_KEY").is_none());

    let output = adapter
        .decode_response(&json!({
            "data":{"audio":"5249464666616b65","status":2},
            "trace_id":"minimax-42",
            "base_resp":{"status_code":0,"status_msg":"success"}
        }))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(&b"RIFFfake"[..]));
    assert_eq!(output.request_id(), Some("minimax-42"));
}

#[test]
fn minimax_rejects_failed_or_malformed_fixture() {
    let adapter = MiniMaxTtsAdapter::new().unwrap();
    assert!(adapter
        .decode_response(&json!({
            "base_resp":{"status_code":1004,"status_msg":"auth failed"}
        }))
        .is_err());
    assert!(adapter
        .decode_response(&json!({
            "data":{"audio":"abc","status":2},
            "base_resp":{"status_code":0,"status_msg":"success"}
        }))
        .is_err());
}

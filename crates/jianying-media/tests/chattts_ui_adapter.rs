use jianying_media::{ChatTtsUiAdapter, TtsAudioFormat, TtsRequest};
use serde_json::json;

#[test]
fn chattts_ui_maps_only_verified_form_fields() {
    let adapter = ChatTtsUiAdapter::new("http://127.0.0.1:9966/tts").unwrap();
    let request = TtsRequest::new("你好，剪映", TtsAudioFormat::Wav)
        .unwrap()
        .with_voice("seed_1518_restored_emb.pt")
        .with_emotion("[laugh]");

    let form = adapter.encode_form(&request).unwrap();
    assert_eq!(form.get("text").map(String::as_str), Some("你好，剪映"));
    assert_eq!(
        form.get("voice").map(String::as_str),
        Some("seed_1518_restored_emb.pt")
    );
    assert_eq!(form.get("prompt").map(String::as_str), Some("[laugh]"));
    assert_eq!(form.get("is_stream").map(String::as_str), Some("0"));
    assert!(!form.contains_key("temperature"));
}

#[test]
fn chattts_ui_accepts_only_loopback_audio_urls() {
    let adapter = ChatTtsUiAdapter::new("http://localhost:9966/tts").unwrap();
    let output = adapter
        .decode_response(&json!({
            "code": 0,
            "msg": "ok",
            "audio_files": [{
                "audio_duration": 1.25,
                "filename": "fixture.wav",
                "inference_time": 0.3,
                "url": "http://127.0.0.1:9966/static/fixture.wav"
            }]
        }))
        .unwrap();
    assert_eq!(
        output.remote_url(),
        Some("http://127.0.0.1:9966/static/fixture.wav")
    );

    let error = adapter
        .decode_response(&json!({"code":0,"url":"http://169.254.169.254/latest/meta-data"}))
        .unwrap_err();
    assert!(error.to_string().contains("loopback"));
}

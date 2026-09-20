use jianying_media::{
    BaiduTtsAdapter, CredentialRef, CredentialSource, TtsAudioFormat, TtsRequest,
};

#[test]
fn baidu_maps_short_text_form_without_embedding_access_token() {
    let adapter = BaiduTtsAdapter::new("jianying-fixture-client").unwrap();
    let request = TtsRequest::new("百度语音", TtsAudioFormat::Wav)
        .unwrap()
        .with_voice("4189")
        .with_speed(1.2)
        .unwrap()
        .with_volume(1.4)
        .unwrap()
        .with_pitch(0.5)
        .unwrap()
        .with_emotion("happy")
        .with_sample_rate_hz(16_000)
        .unwrap()
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "BAIDU_ACCESS_TOKEN").unwrap(),
        );

    let form = adapter.encode_form(&request).unwrap();

    assert_eq!(adapter.endpoint(), "https://tsn.baidu.com/text2audio");
    assert_eq!(form.get("tex").map(String::as_str), Some("百度语音"));
    assert_eq!(
        form.get("cuid").map(String::as_str),
        Some("jianying-fixture-client")
    );
    assert_eq!(form.get("ctp").map(String::as_str), Some("1"));
    assert_eq!(form.get("lan").map(String::as_str), Some("zh"));
    assert_eq!(form.get("aue").map(String::as_str), Some("6"));
    assert_eq!(form.get("per").map(String::as_str), Some("4189"));
    assert!(form.contains_key("text_ctrl"));
    assert!(!form.contains_key("tok"));
    assert!(!format!("{form:?}").contains("BAIDU_ACCESS_TOKEN"));
}

#[test]
fn baidu_distinguishes_audio_from_json_error() {
    let adapter = BaiduTtsAdapter::new("jianying-fixture-client").unwrap();
    let output = adapter
        .decode_audio(b"fixture-wav", "audio/wav;rate=16000", Some("baidu-42"))
        .unwrap();
    assert_eq!(output.inline_bytes(), Some(b"fixture-wav".as_slice()));
    assert_eq!(output.request_id(), Some("baidu-42"));

    assert!(adapter
        .decode_audio(
            br#"{"err_no":500,"err_msg":"notsupport"}"#,
            "application/json",
            None,
        )
        .is_err());
}

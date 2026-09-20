use jianying_media::{
    TtsAudioFormat, TtsPlatformCondition, TtsProvider, TtsRequest, WindowsSystemTtsProvider,
};

#[test]
fn windows_provider_declares_wav_voice_ssml_speed_and_volume() {
    let provider = WindowsSystemTtsProvider::new().expect("固定能力声明应有效");
    assert_eq!(
        provider.capability().platform(),
        TtsPlatformCondition::Windows
    );
    let request = TtsRequest::new("<speak>hello</speak>", TtsAudioFormat::Wav)
        .unwrap()
        .with_input_kind(jianying_media::TtsInputKind::Ssml)
        .with_voice("Microsoft Huihui Desktop")
        .with_speed(1.5)
        .unwrap()
        .with_volume(0.5)
        .unwrap();
    provider
        .capability()
        .validate_request(&request)
        .expect("Windows 系统 TTS 应声明实际支持项");
}

#[test]
#[cfg(not(target_os = "windows"))]
fn windows_provider_fails_closed_on_other_platforms() {
    let provider = WindowsSystemTtsProvider::new().unwrap();
    let request = TtsRequest::new("hello", TtsAudioFormat::Wav).unwrap();
    let output = std::env::temp_dir().join("jianying-windows-tts-must-not-exist.wav");
    let _ = std::fs::remove_file(&output);

    let error = provider
        .synthesize(&request, &output)
        .expect_err("非 Windows 主机必须拒绝");

    assert!(error.to_string().contains("unavailable"));
    assert!(!output.exists());
}

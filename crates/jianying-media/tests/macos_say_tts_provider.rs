use jianying_media::{
    MacOsSayTtsProvider, TtsAudioFormat, TtsPlatformCondition, TtsProvider, TtsRequest,
};
#[cfg(target_os = "macos")]
use std::process::Command;

#[test]
fn macos_say_declares_platform_and_supported_controls() {
    let provider = MacOsSayTtsProvider::new().expect("固定能力声明应有效");
    assert_eq!(
        provider.capability().platform(),
        TtsPlatformCondition::MacOs
    );

    let request = TtsRequest::new("你好", TtsAudioFormat::Aiff)
        .unwrap()
        .with_voice("Tingting")
        .with_speed(1.2)
        .unwrap();
    provider
        .capability()
        .validate_request(&request)
        .expect("say 应支持音色和语速");

    let unsupported = TtsRequest::new("你好", TtsAudioFormat::Aiff)
        .unwrap()
        .with_pitch(0.2)
        .unwrap();
    assert!(provider
        .capability()
        .validate_request(&unsupported)
        .expect_err("say 不应静默忽略音高")
        .to_string()
        .contains("pitch control"));
}

#[test]
#[cfg(target_os = "macos")]
fn macos_say_creates_auditable_aiff_artifact() {
    if Command::new("say").arg("-v").arg("?").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!("jianying-say-tts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let output = root.join("voice.aiff");
    let provider = MacOsSayTtsProvider::new().unwrap();
    let request = TtsRequest::new("剪映语音测试", TtsAudioFormat::Aiff).unwrap();

    let artifact = provider.synthesize(&request, &output).unwrap();

    assert_eq!(artifact.provider_id(), "system-macos");
    assert!(artifact.byte_length() > 0);
    assert!(output.is_file());
    let _ = std::fs::remove_dir_all(root);
}

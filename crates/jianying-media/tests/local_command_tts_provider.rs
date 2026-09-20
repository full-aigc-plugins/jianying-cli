use jianying_media::{LocalCommandTtsProvider, TtsAudioFormat};
#[cfg(unix)]
use jianying_media::{TtsProvider, TtsRequest};
#[cfg(unix)]
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn command_template_requires_output_placeholder_and_balanced_quotes() {
    assert!(
        LocalCommandTtsProvider::new("tts --text {text}", TtsAudioFormat::Wav)
            .expect_err("缺少输出占位符必须失败")
            .to_string()
            .contains("{out}")
    );
    assert!(
        LocalCommandTtsProvider::new("tts '{out}", TtsAudioFormat::Wav)
            .expect_err("不配对引号必须失败")
            .to_string()
            .contains("unbalanced")
    );
}

#[test]
#[cfg(unix)]
fn provider_delivers_text_as_one_argv_without_a_shell() {
    let root =
        std::env::temp_dir().join(format!("jianying-local-command-tts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("fixture.sh");
    std::fs::write(&script, "#!/bin/sh\nprintf '%s' \"$2\" > \"$1\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = root.join("voice.wav");
    let provider = LocalCommandTtsProvider::new(
        format!("{} {{out}} {{text}}", script.display()),
        TtsAudioFormat::Wav,
    )
    .unwrap();
    let request = TtsRequest::new("hello; touch must-not-run", TtsAudioFormat::Wav).unwrap();

    let artifact = provider.synthesize(&request, &output).unwrap();

    assert_eq!(provider.text_delivery(), "argv");
    assert_eq!(std::fs::read_to_string(&output).unwrap(), request.text());
    assert_eq!(artifact.byte_length(), request.text().len() as u64);
    assert_eq!(artifact.provider_id(), "local-command");
    assert!(!Path::new("must-not-run").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
#[cfg(unix)]
fn provider_uses_stdin_when_text_placeholder_is_absent() {
    let root = std::env::temp_dir().join(format!(
        "jianying-local-command-tts-stdin-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("fixture.sh");
    std::fs::write(&script, "#!/bin/sh\ncat > \"$1\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = root.join("voice.wav");
    let provider =
        LocalCommandTtsProvider::new(format!("{} {{out}}", script.display()), TtsAudioFormat::Wav)
            .unwrap();
    let request = TtsRequest::new("stdin text", TtsAudioFormat::Wav).unwrap();

    provider.synthesize(&request, &output).unwrap();

    assert_eq!(provider.text_delivery(), "stdin");
    assert_eq!(std::fs::read_to_string(output).unwrap(), request.text());
    let _ = std::fs::remove_dir_all(root);
}

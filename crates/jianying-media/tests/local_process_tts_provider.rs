use jianying_media::{LocalProcessTtsProvider, TtsAudioFormat};

#[cfg(unix)]
use jianying_media::{TtsProvider, TtsRequest};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[test]
fn local_process_requires_absolute_executable_and_output_placeholder() {
    assert!(LocalProcessTtsProvider::new(
        "relative-tts",
        vec!["{out}".to_owned()],
        TtsAudioFormat::Wav,
    )
    .expect_err("可执行文件必须是绝对路径")
    .to_string()
    .contains("absolute"));
    assert!(LocalProcessTtsProvider::new(
        std::env::temp_dir().join("tts"),
        vec!["--voice".to_owned()],
        TtsAudioFormat::Wav,
    )
    .expect_err("参数必须声明输出文件")
    .to_string()
    .contains("{out}"));
}

#[test]
#[cfg(unix)]
fn local_process_uses_structured_argv_and_stdin() {
    let root = std::env::temp_dir().join(format!("jianying-process-tts-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let script = root.join("fixture.sh");
    std::fs::write(&script, "#!/bin/sh\ncat > \"$1\"\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = root.join("voice.wav");
    let provider =
        LocalProcessTtsProvider::new(script, vec!["{out}".to_owned()], TtsAudioFormat::Wav)
            .unwrap();
    let request = TtsRequest::new("process input", TtsAudioFormat::Wav).unwrap();

    let artifact = provider.synthesize(&request, &output).unwrap();

    assert_eq!(artifact.provider_id(), "local-process");
    assert_eq!(std::fs::read_to_string(output).unwrap(), "process input");
    let _ = std::fs::remove_dir_all(root);
}

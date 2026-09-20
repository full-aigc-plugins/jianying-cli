#![cfg(unix)]

use jianying_media::{
    AsrOutputFormat, AsrProvider, AsrRequest, AsrTimestampGranularity, WhisperCppAsrProvider,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "jianying-whisper-cpp-{name}-{}",
        std::process::id()
    ))
}

fn fixture_provider(root: &std::path::Path) -> WhisperCppAsrProvider {
    let executable = root.join("fake whisper;touch SHOULD_NOT_EXIST");
    let model = root.join("ggml-fixture.bin");
    std::fs::write(
        &executable,
        r#"#!/bin/sh
script_dir=$(dirname "$0")
printf '%s\n' "$@" > "$script_dir/args.log"
prefix=""
extension=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-file) prefix="$2"; shift 2 ;;
    --output-txt) extension="txt"; shift ;;
    --output-srt) extension="srt"; shift ;;
    --output-vtt) extension="vtt"; shift ;;
    --output-json|--output-json-full) extension="json"; shift ;;
    *) shift ;;
  esac
done
[ -n "$prefix" ] && [ -n "$extension" ] || exit 9
printf '%s' '{"text":"fixture transcript"}' > "${prefix}.${extension}"
"#,
    )
    .unwrap();
    std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&model, b"fixture model").unwrap();
    WhisperCppAsrProvider::new(executable, model, "ggml-fixture").unwrap()
}

#[test]
fn whisper_cpp_uses_structured_argv_and_atomically_commits_the_expected_format() {
    let root = root("success");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let provider = fixture_provider(&root);
    let source = root.join("audio ; touch injected.wav");
    let output = root.join("transcript.srt");
    let args_log = root.join("args.log");
    std::fs::write(&source, b"synthetic audio").unwrap();
    let request = AsrRequest::new(
        AsrRequest::hash_source(&source).unwrap(),
        AsrOutputFormat::Srt,
    )
    .unwrap()
    .with_model("ggml-fixture")
    .with_language("zh")
    .with_translate_to_english(true);

    let artifact = provider.transcribe(&request, &source, &output).unwrap();

    assert_eq!(artifact.provider_id(), "whisper-cpp");
    assert_eq!(artifact.path(), output);
    assert_eq!(
        std::fs::read_to_string(&output).unwrap(),
        r#"{"text":"fixture transcript"}"#
    );
    let argv = std::fs::read_to_string(args_log).unwrap();
    assert!(argv.contains("--model\n"));
    assert!(argv.contains("--file\n"));
    assert!(argv.contains("--output-file\n"));
    assert!(argv.contains("--output-srt\n"));
    assert!(argv.contains("--language\nzh\n"));
    assert!(argv.contains("--translate\n"));
    assert!(argv.contains(source.to_str().unwrap()));
    assert!(!root.join("injected.wav").exists());
    assert!(provider
        .capability()
        .executor_identity()
        .starts_with("whisper.cpp@sha256:"));
    assert!(!root.read_dir().unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("partial")));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn whisper_cpp_rejects_digest_model_and_unsupported_word_timestamp_before_execution() {
    let root = root("fail-closed");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let provider = fixture_provider(&root);
    let source = root.join("audio.wav");
    let output = root.join("transcript.json");
    let args_log = root.join("args.log");
    std::fs::write(&source, b"synthetic audio").unwrap();

    let wrong_digest = AsrRequest::new("a".repeat(64), AsrOutputFormat::Json).unwrap();
    assert!(provider
        .transcribe(&wrong_digest, &source, &output)
        .is_err());
    assert!(!args_log.exists());

    let wrong_model = AsrRequest::new(
        AsrRequest::hash_source(&source).unwrap(),
        AsrOutputFormat::Json,
    )
    .unwrap()
    .with_model("different-model");
    assert!(provider.transcribe(&wrong_model, &source, &output).is_err());
    assert!(!args_log.exists());

    let word_timestamps = AsrRequest::new(
        AsrRequest::hash_source(&source).unwrap(),
        AsrOutputFormat::VerboseJson,
    )
    .unwrap()
    .with_model("ggml-fixture")
    .with_timestamps([AsrTimestampGranularity::Word]);
    assert!(provider
        .transcribe(&word_timestamps, &source, &output)
        .is_err());
    assert!(!args_log.exists());
    std::fs::remove_dir_all(root).unwrap();
}

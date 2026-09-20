#![cfg(unix)]

use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-asr-cli-{name}-{}-{nonce}",
        std::process::id()
    ))
}

fn fake_whisper(root: &Path) -> PathBuf {
    let executable = root.join("fake whisper;touch injected");
    fs::write(
        &executable,
        r#"#!/bin/sh
script_dir=$(dirname "$0")
printf 'run\n' >> "$script_dir/invocations.log"
printf '%s\n' "$@" > "$script_dir/args.log"
if [ -f "$script_dir/fail" ]; then
  printf 'synthetic whisper failure' >&2
  exit 17
fi
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
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    executable
}

fn run(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .current_dir(root)
        .args(arguments)
        .output()
        .expect("run jianying")
}

fn envelope(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON envelope: {error}; stdout={}; stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn arguments<'a>(
    source: &'a Path,
    target: &'a Path,
    executable: &'a Path,
    model: &'a Path,
    state_root: &'a Path,
) -> Vec<&'a str> {
    vec![
        "--json",
        "media",
        "transcribe",
        source.to_str().unwrap(),
        "--out",
        target.to_str().unwrap(),
        "--executable",
        executable.to_str().unwrap(),
        "--model",
        model.to_str().unwrap(),
        "--model-id",
        "ggml-fixture",
        "--format",
        "srt",
        "--language",
        "zh",
        "--translate",
        "--task-id",
        "asr-cli-contract",
        "--state-root",
        state_root.to_str().unwrap(),
    ]
}

#[test]
fn plan_executes_nothing_then_success_is_reused_by_the_persistent_ledger() {
    let root = fixture_root("reuse");
    fs::create_dir_all(&root).unwrap();
    let executable = fake_whisper(&root);
    let model = root.join("ggml-fixture.bin");
    let source = root.join("audio ; touch injected.wav");
    let target = root.join("transcript.srt");
    let state_root = root.join("asr-state");
    fs::write(&model, b"fixture model").unwrap();
    fs::write(&source, b"synthetic audio").unwrap();
    let base = arguments(&source, &target, &executable, &model, &state_root);

    let mut plan_args = base.clone();
    plan_args.push("--plan");
    let planned = run(&root, &plan_args);
    assert!(planned.status.success());
    let planned_json = envelope(&planned);
    assert_eq!(planned_json["data"]["state"], "ready");
    assert_eq!(planned_json["data"]["network"], false);
    assert!(!target.exists());
    assert!(!root.join("invocations.log").exists());
    assert!(!state_root.exists());

    let first = run(&root, &base);
    assert!(
        first.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&first.stdout),
        String::from_utf8_lossy(&first.stderr)
    );
    let first_json = envelope(&first);
    assert_eq!(first_json["data"]["state"], "succeeded");
    assert_eq!(first_json["data"]["attempts"], 1);
    assert_eq!(first_json["data"]["reused"], false);
    assert_eq!(
        fs::read_to_string(&target).unwrap(),
        r#"{"text":"fixture transcript"}"#
    );

    let second = run(&root, &base);
    assert!(second.status.success());
    let second_json = envelope(&second);
    assert_eq!(second_json["data"]["reused"], true);
    assert_eq!(second_json["data"]["attempts"], 1);
    assert_eq!(
        first_json["data"]["idempotency_key"],
        second_json["data"]["idempotency_key"]
    );
    assert_eq!(
        fs::read_to_string(root.join("invocations.log")).unwrap(),
        "run\n"
    );

    let argv = fs::read_to_string(root.join("args.log")).unwrap();
    assert!(argv.contains("--model\n"));
    assert!(argv.contains("--file\n"));
    assert!(argv.contains("--output-srt\n"));
    assert!(argv.contains("--language\nzh\n"));
    assert!(argv.contains("--translate\n"));
    assert!(argv.contains(source.to_str().unwrap()));
    assert!(!root.join("injected").exists());
    assert!(!root.read_dir().unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("partial")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn definite_failure_requires_explicit_retry_and_increments_attempts_only_on_execution() {
    let root = fixture_root("retry");
    fs::create_dir_all(&root).unwrap();
    let executable = fake_whisper(&root);
    let model = root.join("ggml-fixture.bin");
    let source = root.join("audio.wav");
    let target = root.join("transcript.json");
    let state_root = root.join("asr-state");
    fs::write(&model, b"fixture model").unwrap();
    fs::write(&source, b"synthetic audio").unwrap();
    fs::write(root.join("fail"), b"fail").unwrap();
    let mut base = arguments(&source, &target, &executable, &model, &state_root);
    let format_index = base.iter().position(|value| *value == "srt").unwrap();
    base[format_index] = "json";

    let first = run(&root, &base);
    assert!(!first.status.success());
    assert_eq!(envelope(&first)["error"]["type"], "execution_failed");
    assert!(!target.exists());

    let duplicate = run(&root, &base);
    assert!(!duplicate.status.success());
    assert!(!target.exists());
    assert_eq!(
        fs::read_to_string(root.join("invocations.log")).unwrap(),
        "run\n"
    );

    fs::remove_file(root.join("fail")).unwrap();
    let mut retry_args = base.clone();
    retry_args.push("--retry");
    let retried = run(&root, &retry_args);
    assert!(
        retried.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&retried.stdout),
        String::from_utf8_lossy(&retried.stderr)
    );
    let retried_json = envelope(&retried);
    assert_eq!(retried_json["data"]["state"], "succeeded");
    assert_eq!(retried_json["data"]["attempts"], 2);
    assert_eq!(retried_json["data"]["reused"], false);
    assert_eq!(
        fs::read_to_string(root.join("invocations.log")).unwrap(),
        "run\nrun\n"
    );
    fs::remove_dir_all(root).unwrap();
}

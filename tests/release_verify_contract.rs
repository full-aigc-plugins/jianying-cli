//! 发布包完整性合同：离线复核 manifest 的 sha256/大小，篡改必须被发现。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn temp_bundle(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("jianying-release-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

/// 空文件的 SHA-256，作为已知正确值写在清单里。
const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn write_bundle(root: &Path, notes: &str) {
    fs::write(root.join("notes.txt"), notes).unwrap();
    let size = fs::metadata(root.join("notes.txt")).unwrap().len();
    let manifest = json!({
        "schema": "jianying-cli-release/v1",
        "version": "9.9.9",
        "platform": "win32-x64",
        "capabilityManifest": "provenance/CAPABILITIES.json",
        "files": [
            {"path": "notes.txt", "sha256": EMPTY_SHA256, "size": size}
        ]
    });
    fs::write(
        root.join("manifest.json"),
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();
}

#[test]
fn verify_accepts_an_intact_bundle_and_rejects_a_tampered_file() {
    let root = temp_bundle("ok");
    write_bundle(&root, "");

    let report = run_ok(&["release", "verify", &root.to_string_lossy(), "--json"]);
    assert_eq!(report["ok"], json!(true));
    assert_eq!(report["version"], json!("9.9.9"));
    assert_eq!(report["platform"], json!("win32-x64"));
    assert_eq!(report["files_checked"], json!(1));
    assert_eq!(report["files_failed"], json!(0));

    // 改一个字节：hash 与 size 都会变
    fs::write(root.join("notes.txt"), "x").unwrap();
    let output = run(&["release", "verify", &root.to_string_lossy(), "--json"]);
    let tampered: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tampered["data"]["ok"], json!(false), "篡改必须被发现");
    assert_eq!(tampered["data"]["files_failed"], json!(1));
    assert_eq!(tampered["data"]["files"][0]["ok"], json!(false));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn verify_reports_a_missing_file_and_rejects_unknown_manifest_schema() {
    let root = temp_bundle("missing");
    write_bundle(&root, "");
    fs::remove_file(root.join("notes.txt")).unwrap();

    let output = run(&["release", "verify", &root.to_string_lossy(), "--json"]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["ok"], json!(false));
    assert_eq!(report["data"]["files_failed"], json!(1));
    assert_eq!(report["data"]["files"][0]["reason"], json!("missing"));

    fs::write(
        root.join("manifest.json"),
        r#"{"schema":"something-else/v1","files":[]}"#,
    )
    .unwrap();
    let bad = run(&["release", "verify", &root.to_string_lossy(), "--json"]);
    assert_eq!(bad.status.code(), Some(1), "未知 schema 必须拒绝");
    assert!(String::from_utf8_lossy(&bad.stdout).contains("unsupported manifest schema"));
    let _ = fs::remove_dir_all(root);
}

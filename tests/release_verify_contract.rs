//! 发布包完整性合同：离线复核 manifest 的 sha256/大小，篡改必须被发现。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    test_command().args(args).output().unwrap()
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

#[test]
fn hash_matches_the_known_empty_file_vector() {
    let root = temp_bundle("hash");
    fs::write(root.join("empty.bin"), "").unwrap();
    let report = run_ok(&[
        "release",
        "hash",
        &root.join("empty.bin").to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["sha256"], json!(EMPTY_SHA256));
    assert_eq!(report["size"], json!(0));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn capability_drift_between_the_binary_and_the_bundle_is_detected() {
    let root = temp_bundle("caps");
    let binary = std::path::PathBuf::from(env!("CARGO_BIN_EXE_jianying"));
    let caps_dir = root.join("provenance");
    fs::create_dir_all(&caps_dir).unwrap();

    // 用二进制自报能力生成包内清单，二者此时应当一致
    let output = Command::new(&binary)
        .args(["capabilities", "--json"])
        .output()
        .unwrap();
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    let mut declared = envelope["data"].clone();
    fs::write(
        caps_dir.join("CAPABILITIES.json"),
        serde_json::to_string_pretty(&declared).unwrap(),
    )
    .unwrap();

    let write_manifest = |declared: &Value| {
        let hash = run_ok(&[
            "release",
            "hash",
            &caps_dir.join("CAPABILITIES.json").to_string_lossy(),
            "--json",
        ]);
        let manifest = json!({
            "schema": "jianying-cli-release/v1",
            "version": "9.9.9",
            "platform": "win32-x64",
            "files": [
                {"path": "provenance/CAPABILITIES.json",
                 "sha256": hash["sha256"], "size": hash["size"]}
            ]
        });
        let _ = declared;
        fs::write(
            root.join("manifest.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    };

    write_manifest(&declared);
    let report = run_ok(&[
        "release",
        "verify",
        &root.to_string_lossy(),
        "--binary",
        &binary.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["ok"], json!(true), "{report}");
    assert_eq!(report["capabilities"]["ok"], json!(true));
    assert_eq!(
        report["capabilities"]["declared_entries"],
        report["capabilities"]["embedded_entries"]
    );

    // 改一个能力状态：哈希重算后文件本身是"完整"的，但契约漂移必须被发现
    let caps = declared["capabilities"].as_array_mut().unwrap();
    let target = caps
        .iter_mut()
        .find(|item| item["status"] == json!("supported"))
        .expect("至少有一个 supported 能力");
    target["status"] = json!("partial");
    fs::write(
        caps_dir.join("CAPABILITIES.json"),
        serde_json::to_string_pretty(&declared).unwrap(),
    )
    .unwrap();
    write_manifest(&declared);
    let output = run(&[
        "release",
        "verify",
        &root.to_string_lossy(),
        "--binary",
        &binary.to_string_lossy(),
        "--json",
    ]);
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["data"]["ok"], json!(false), "契约漂移必须判失败");
    assert_eq!(report["data"]["capabilities"]["ok"], json!(false));
    assert!(!report["data"]["capabilities"]["mismatches"]
        .as_array()
        .unwrap()
        .is_empty());
    let _ = fs::remove_dir_all(root);
}

/// Builds the CLI command used by these tests.
///
/// `JIANYING_CLI_PROCESS_LIST` replaces the host process listing with a fixed file: the
/// production path shells out to `tasklist`, which can wedge (it does on the machine these
/// tests were stabilised on) and every write path now refuses to run when it cannot tell
/// whether the editor is open. The listing source is all this replaces - the "editor is
/// running -> refuse" rule itself is still asserted with a synthetic listing.
fn test_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command.env(
        "JIANYING_CLI_PROCESS_LIST",
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/empty-processes.txt"
        ),
    );
    command
}

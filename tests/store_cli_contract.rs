use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 5_000_000,
        width: 1280,
        height: 720,
        has_video: false,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "jianying-store-cli-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"source",
        "canvas":{"width":1280,"height":720,"fps":30},
        "tracks":[{"type":"text","name":"字幕","segments":[{
            "start_us":0,"duration_us":1_000_000,"text":"原文"
        }]}]
    }))
    .unwrap();
    let source = root.join("source");
    draft::build(&plan, &root, &source, None, &probe_stub).unwrap();
    (root, source)
}

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

#[test]
fn store_register_rename_sync_backup_and_restore_are_operational() {
    let (root, source) = fixture();
    let store_root = root.join("store");
    std::fs::create_dir_all(&store_root).unwrap();
    run_ok(&[
        "store",
        "register",
        &source.to_string_lossy(),
        "--root",
        &store_root.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        run_ok(&[
            "store",
            "list",
            "--root",
            &store_root.to_string_lossy(),
            "--json",
        ])["drafts"],
        json!(["source"])
    );

    let renamed = run_ok(&[
        "store",
        "rename",
        "source",
        "renamed",
        "--root",
        &store_root.to_string_lossy(),
        "--json",
    ]);
    let draft_path = store_root.join("renamed");
    assert_eq!(renamed["status"], "renamed");
    assert!(!store_root.join("source").exists());
    let root_meta: Value = serde_json::from_str(
        &std::fs::read_to_string(store_root.join("root_meta_info.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(root_meta["all_draft_store"][0]["draft_name"], "renamed");

    std::fs::write(draft_path.join("draft_info.json"), "{\"tracks\":[]}").unwrap();
    let plan = run_ok(&["store", "sync", &draft_path.to_string_lossy(), "--json"]);
    assert_eq!(plan["reconciled"].as_array().unwrap().len(), 0);
    assert_eq!(plan["targets"][0]["drifted"], true);
    let applied = run_ok(&[
        "store",
        "sync",
        &draft_path.to_string_lossy(),
        "--apply",
        "--force-newer",
        "--json",
    ]);
    assert_eq!(applied["reconciled"].as_array().unwrap().len(), 1);
    assert_eq!(
        std::fs::read(draft_path.join("draft_content.json")).unwrap(),
        std::fs::read(draft_path.join("draft_info.json")).unwrap()
    );

    let backup = root.join("backup");
    run_ok(&[
        "store",
        "backup",
        &draft_path.to_string_lossy(),
        "--out",
        &backup.to_string_lossy(),
        "--json",
    ]);
    let caption_id = run_ok(&["captions", "list", &draft_path.to_string_lossy(), "--json"])[0]
        ["segment_id"]
        .as_str()
        .unwrap()
        .to_owned();
    run_ok(&[
        "captions",
        "set",
        &draft_path.to_string_lossy(),
        &caption_id,
        "已修改",
        "--json",
    ]);
    run_ok(&[
        "store",
        "restore",
        &backup.to_string_lossy(),
        "--target",
        &draft_path.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        run_ok(&["captions", "list", &draft_path.to_string_lossy(), "--json",])[0]["text"],
        "原文"
    );
    assert_eq!(
        run_ok(&["project", "verify", &draft_path.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn store_decrypt_is_detection_only_and_directories_are_machine_readable() {
    let (root, source) = fixture();
    let plaintext = run_ok(&["store", "decrypt", &source.to_string_lossy(), "--json"]);
    assert_eq!(plaintext["encrypted"], false);
    assert_eq!(plaintext["classification"], "plaintext");
    assert_eq!(plaintext["decrypt_supported"], false);

    let encrypted = root.join("encrypted.bin");
    std::fs::write(&encrypted, [0x89, 0x41, 0x45, 0x53, 0, 1, 2]).unwrap();
    let detected = run_ok(&["store", "decrypt", &encrypted.to_string_lossy(), "--json"]);
    assert_eq!(detected["encrypted"], true);
    assert_eq!(detected["classification"], "encrypted");

    let corrupted = root.join("corrupted.json");
    std::fs::write(&corrupted, b"{bad json").unwrap();
    let classified = run_ok(&["store", "decrypt", &corrupted.to_string_lossy(), "--json"]);
    assert_eq!(classified["encrypted"], false);
    assert_eq!(classified["classification"], "corrupted");

    assert!(run_ok(&["store", "directories", "--json"]).is_array());
    let _ = std::fs::remove_dir_all(root);
}

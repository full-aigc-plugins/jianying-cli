#[cfg(unix)]
use jianying_runtime::{
    LocalRuntimeAdapter, RuntimeAdapter, RuntimeControlStatus, RuntimeFileIdentity,
    RuntimePlatform, RuntimeProfile,
};
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
fn fixture_root() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-runtime-control-{}-{nonce}",
        std::process::id()
    ))
}

#[cfg(unix)]
#[test]
fn adapter_starts_reports_and_stops_only_its_owned_process() {
    use std::os::unix::fs::PermissionsExt;

    let root = fixture_root();
    fs::create_dir_all(&root).unwrap();
    let executable = root.join("fixture-editor");
    fs::write(&executable, b"#!/bin/sh\nwhile true; do sleep 1; done\n").unwrap();
    let mut permissions = fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&executable, permissions).unwrap();

    let profile = RuntimeProfile::new(
        "fixture-editor",
        "Fixture Editor",
        "1.0",
        RuntimePlatform::MacOs,
        ["editor.control"],
    )
    .unwrap()
    .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap());
    let adapter = LocalRuntimeAdapter::new(profile, executable, Vec::<String>::new()).unwrap();

    assert_eq!(adapter.status().unwrap(), RuntimeControlStatus::Stopped);
    assert_eq!(adapter.start().unwrap(), RuntimeControlStatus::Running);
    assert_eq!(adapter.status().unwrap(), RuntimeControlStatus::Running);
    assert!(adapter
        .start()
        .unwrap_err()
        .to_string()
        .contains("already running"));
    assert_eq!(adapter.stop().unwrap(), RuntimeControlStatus::Stopped);
    assert_eq!(adapter.status().unwrap(), RuntimeControlStatus::Stopped);
    assert!(adapter
        .stop()
        .unwrap_err()
        .to_string()
        .contains("not running"));

    fs::remove_dir_all(root).unwrap();
}

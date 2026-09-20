use jianying_runtime::{
    RuntimeFileIdentity, RuntimePlatform, RuntimeProbeInput, RuntimeProfile, RuntimeWriteGuard,
};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixture_root(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "jianying-runtime-{name}-{}-{nonce}",
        std::process::id()
    ))
}

#[test]
fn runtime_profile_records_identity_and_probe_evidence() {
    let root = fixture_root("probe");
    let executable = root.join("JianyingPro");
    let draft_root = root.join("drafts");
    let material = root.join("clip.mp4");
    fs::create_dir_all(&draft_root).unwrap();
    fs::write(&executable, b"fixture executable").unwrap();
    fs::write(&material, b"fixture media").unwrap();

    let identity = RuntimeFileIdentity::from_path(&executable).unwrap();
    let profile = RuntimeProfile::new(
        "jianying-macos-11.4",
        "JianYing Pro",
        "11.4",
        RuntimePlatform::MacOs,
        ["project.edit_isolated", "editor.control"],
    )
    .unwrap()
    .with_file_identity(identity.clone())
    .with_process_names(["JianyingPro"])
    .with_draft_roots([draft_root.clone()]);

    assert_eq!(profile.file_identity(), Some(&identity));
    assert!(profile.process_names().contains("JianyingPro"));
    assert_eq!(profile.draft_roots(), std::slice::from_ref(&draft_root));

    let report = profile
        .probe(
            RuntimeProbeInput::new(
                "JianYing Pro",
                "11.4",
                RuntimePlatform::MacOs,
                executable,
                draft_root,
            )
            .with_running_processes(["JianyingPro"])
            .with_material_paths([material])
            .with_requested_capabilities(["project.edit_isolated"]),
        )
        .unwrap();

    assert!(report.supported());
    assert!(report.editor_running());
    assert!(report.permissions().draft_root_readable());
    assert!(report.permissions().draft_root_writable());
    assert!(report.materials_available());
    assert_eq!(report.executable_identity(), &identity);

    let blocked = RuntimeWriteGuard::check(&profile, &report).unwrap_err();
    assert_eq!(
        blocked.recovery_argv(),
        &[
            "jianying",
            "runtime",
            "stop",
            "--profile",
            "jianying-macos-11.4"
        ]
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn unknown_version_and_changed_executable_fail_closed() {
    let root = fixture_root("fail-closed");
    let executable = root.join("JianyingPro");
    let draft_root = root.join("drafts");
    fs::create_dir_all(&draft_root).unwrap();
    fs::write(&executable, b"known executable").unwrap();

    let profile = RuntimeProfile::new(
        "jianying-macos-11.4",
        "JianYing Pro",
        "11.4",
        RuntimePlatform::MacOs,
        ["project.edit_isolated"],
    )
    .unwrap()
    .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap())
    .with_draft_roots([draft_root.clone()]);

    let unknown = profile.probe(
        RuntimeProbeInput::new(
            "JianYing Pro",
            "12.0",
            RuntimePlatform::MacOs,
            executable.clone(),
            draft_root.clone(),
        )
        .with_requested_capabilities(["project.edit_isolated"]),
    );
    assert!(unknown
        .unwrap_err()
        .to_string()
        .contains("unsupported runtime version"));

    fs::write(&executable, b"tampered executable").unwrap();
    let changed = profile.probe(
        RuntimeProbeInput::new(
            "JianYing Pro",
            "11.4",
            RuntimePlatform::MacOs,
            executable,
            draft_root,
        )
        .with_requested_capabilities(["project.edit_isolated"]),
    );
    assert!(changed
        .unwrap_err()
        .to_string()
        .contains("file identity mismatch"));

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn missing_material_or_capability_fails_closed() {
    let root = fixture_root("missing");
    let executable = root.join("JianyingPro");
    let draft_root = root.join("drafts");
    fs::create_dir_all(&draft_root).unwrap();
    fs::write(&executable, b"known executable").unwrap();

    let profile = RuntimeProfile::new(
        "jianying-macos-11.4",
        "JianYing Pro",
        "11.4",
        RuntimePlatform::MacOs,
        ["project.edit_isolated"],
    )
    .unwrap()
    .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap())
    .with_draft_roots([draft_root.clone()]);

    let missing_material = profile.probe(
        RuntimeProbeInput::new(
            "JianYing Pro",
            "11.4",
            RuntimePlatform::MacOs,
            executable.clone(),
            draft_root.clone(),
        )
        .with_material_paths([root.join("missing.mp4")]),
    );
    assert!(missing_material
        .unwrap_err()
        .to_string()
        .contains("material unavailable"));

    let missing_capability = profile.probe(
        RuntimeProbeInput::new(
            "JianYing Pro",
            "11.4",
            RuntimePlatform::MacOs,
            executable,
            draft_root,
        )
        .with_requested_capabilities(["render.native"]),
    );
    assert!(missing_capability
        .unwrap_err()
        .to_string()
        .contains("unsupported runtime capability"));

    fs::remove_dir_all(root).unwrap();
}

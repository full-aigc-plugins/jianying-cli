use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn data(args: &[&str]) -> Value {
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

fn setup() -> (PathBuf, PathBuf, Vec<u8>) {
    let root = std::env::temp_dir().join(format!(
        "jianying-fixture-contract-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let draft = root.join("home/secretuser/draft");
    std::fs::create_dir_all(&draft).unwrap();
    let mut timeline = json!({
        "id":"fixture-draft",
        "name":"/home/secretuser/video.mov secret@example.com",
        "duration":1_000_000,
        "fps":30,
        "canvas_config":{"width":1080,"height":1920,"ratio":"9:16"},
        "platform":{"app_source":"cc","app_version":"8.7.0","os":"windows",
            "device_id":"a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
            "mac_address":"0f1e2d3c4b5a69788796a5b4c3d2e1f0","hard_disk_id":""},
        "tracks":[{"id":"track","type":"video","segments":[{
            "id":"segment","extra_material_refs":["mask-1"],
            "common_keyframes":[{"property_type":"KFTypeSyntheticMaskCenterX","keyframe_list":[]}]
        }]}],
        "materials":{"videos":[],"audios":[],"texts":[],"speeds":[],
            "common_mask":[{"id":"mask-1","type":"mask","name":"Rectangle",
                "resource_type":"rectangle","config":{"width":0.5},
                "mask_keyframes":[{"time_offset":0,"values":[0.1]}]}]}
    });
    timeline["last_modified_platform"] = timeline["platform"].clone();
    let encoded = serde_json::to_vec_pretty(&timeline).unwrap();
    for name in ["draft_content.json", "draft_info.json"] {
        std::fs::write(draft.join(name), &encoded).unwrap();
    }
    std::fs::write(
        draft.join("template-2.tmp"),
        serde_json::to_vec(&serde_json::to_string(&timeline).unwrap()).unwrap(),
    )
    .unwrap();
    std::fs::create_dir_all(draft.join("assets/video")).unwrap();
    std::fs::write(draft.join("assets/video/private.mp4"), b"private-media").unwrap();
    std::fs::create_dir_all(draft.join("Timelines/guid")).unwrap();
    std::fs::write(
        draft.join("Timelines/project.json"),
        b"{\"active\":\"guid\"}",
    )
    .unwrap();
    std::fs::write(draft.join("Timelines/guid/draft_info.json"), &encoded).unwrap();
    (root, draft, encoded)
}

fn text(path: impl AsRef<Path>) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn fixture_builds_redacted_media_free_read_only_bundle_and_rechecks_it() {
    let (root, draft, source_before) = setup();
    let bundle = root.join("bundle");
    let report = data(&[
        "project",
        "fixture",
        &draft.to_string_lossy(),
        "--out",
        &bundle.to_string_lossy(),
        "--check",
        "--json",
    ]);
    assert_eq!(report["ok"], true);
    assert_eq!(report["media_excluded"], true);
    assert_eq!(report["redaction_check"]["ok"], true);
    assert!(report["redaction_check"]["files_scanned"].as_u64().unwrap() >= 6);
    assert_eq!(
        report["mask_keyframe_evidence"]["verdict"],
        "mask-keyframe-evidence-found"
    );
    assert!(report["redaction_kinds"]["linux_user"].as_u64().unwrap() >= 1);
    assert!(report["redaction_kinds"]["email"].as_u64().unwrap() >= 1);
    assert!(report["redaction_kinds"]["device_ids"].as_u64().unwrap() >= 4);

    let bundled = text(bundle.join("draft_content.json"));
    assert!(!bundled.contains("secretuser"));
    assert!(!bundled.contains("a1b2c3d4"));
    assert!(bundled.contains("/home/USER/"));
    assert!(bundled.contains("redacted@example.com"));
    assert!(bundle.join("Timelines/project.json").is_file());
    assert!(bundle.join("Timelines/guid/draft_info.json").is_file());
    assert!(!bundle.join("assets").exists());
    assert!(bundle.join("README.md").is_file());
    assert!(bundle.join("diagnose.json").is_file());
    assert!(bundle.join("mask-keyframe-report.json").is_file());
    assert!(bundle.join("SANITIZE_REPORT.json").is_file());
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        source_before
    );

    let in_place = run(&[
        "project",
        "fixture",
        &draft.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(in_place.status.code(), Some(1));
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        source_before,
        "an in-place fixture request must fail before the first source write"
    );

    let verify = data(&[
        "project",
        "fixture",
        &bundle.to_string_lossy(),
        "--check",
        "--json",
    ]);
    assert_eq!(verify["ok"], true);

    std::fs::write(
        bundle.join("leak.json"),
        b"{\"path\":\"/Users/hansmustermann/private.mov\"}",
    )
    .unwrap();
    let leaked = run(&[
        "project",
        "fixture",
        &bundle.to_string_lossy(),
        "--check",
        "--json",
    ]);
    assert_eq!(leaked.status.code(), Some(1));
    let failure: Value = serde_json::from_slice(&leaked.stdout).unwrap();
    assert_eq!(failure["error"]["type"], "fixture_redaction_failed");
    assert_eq!(
        failure["error"]["details"]["findings"][0]["file"],
        "leak.json"
    );
    assert_eq!(
        failure["error"]["details"]["findings"][0]["kind"],
        "home-path"
    );
    assert!(!String::from_utf8_lossy(&leaked.stderr).contains("hansmustermann"));
    let _ = std::fs::remove_dir_all(root);
}

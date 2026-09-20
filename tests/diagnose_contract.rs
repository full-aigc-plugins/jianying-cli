use serde_json::Value;
use std::path::PathBuf;
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
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "jianying-diagnose-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let draft = root.join("draft");
    let result = run(&[
        "project",
        "init",
        "diagnose",
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(result.status.code(), Some(0));
    (root, draft)
}

#[test]
fn diagnose_reports_candidates_divergence_bundle_and_human_view() {
    let (root, draft) = fixture();
    let bundle = root.join("diagnose.json");
    let report = data(&[
        "project",
        "diagnose",
        &draft.to_string_lossy(),
        "--bundle",
        &bundle.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["ok"], true);
    assert_eq!(report["canonical"], "draft_content.json");
    assert_eq!(report["layout"], "content-primary");
    assert_eq!(report["diverged"], false);
    assert_eq!(report["candidates"].as_array().unwrap().len(), 4);
    assert_eq!(report["candidates"][0]["parseable_timeline"], true);
    assert_eq!(report["candidates"][2]["parseable_timeline"], false);
    let stored: Value = serde_json::from_slice(&std::fs::read(&bundle).unwrap()).unwrap();
    assert!(stored.get("bundle").is_none());
    assert_eq!(stored["canonical"], report["canonical"]);

    let human = run(&["project", "diagnose", &draft.to_string_lossy(), "--human"]);
    assert_eq!(human.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&human.stdout).contains("Canonical: draft_content.json"));

    let info = draft.join("draft_info.json");
    let mut changed: Value = serde_json::from_slice(&std::fs::read(&info).unwrap()).unwrap();
    changed["name"] = Value::String("diverged".to_owned());
    std::fs::write(&info, serde_json::to_vec_pretty(&changed).unwrap()).unwrap();
    let diverged = data(&["project", "diagnose", &draft.to_string_lossy(), "--json"]);
    assert_eq!(diverged["ok"], false);
    assert_eq!(diverged["diverged"], true);
    assert!(diverged["next_actions"][0]
        .as_str()
        .unwrap()
        .contains("diverge"));

    let missing = run(&["project", "diagnose", "/missing/jianying-draft", "--json"]);
    assert_eq!(missing.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(root);
}

//! 计划编排合同：确定性组合、覆盖保护、快照与回滚。

use serde_json::Value;
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

fn temp_root(label: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("jianying-planops-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn text_plan(path: &Path, name: &str, fps: u64, duration_us: i64, text: &str) {
    let value = serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":name,
        "canvas":{"width":320,"height":180,"fps":fps},
        "tracks":[{"type":"text","segments":[
            {"start_us":0,"duration_us":duration_us,"text":text}
        ]}]
    });
    fs::write(path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
}

#[test]
fn compose_offsets_each_part_and_the_result_builds() {
    let root = temp_root("compose");
    let a = root.join("a.json");
    let b = root.join("b.json");
    text_plan(&a, "a", 30, 1_000_000, "A1");
    text_plan(&b, "b", 30, 2_000_000, "B1");
    let out = root.join("out.json");

    let report = run_ok(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        &b.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["duration_us"], serde_json::json!(3_000_000));

    let composed: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let starts: Vec<i64> = composed["tracks"][0]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| segment["start_us"].as_i64().unwrap())
        .collect();
    assert_eq!(starts, vec![0, 1_000_000], "第二份必须整体后移");

    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &out.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn overwrite_needs_force_and_leaves_a_snapshot() {
    let root = temp_root("overwrite");
    let a = root.join("a.json");
    text_plan(&a, "a", 30, 1_000_000, "A1");
    let out = root.join("out.json");
    run_ok(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        "--json",
    ]);

    let refused = run(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("already exists"));

    run_ok(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        "--force",
        "--name",
        "second",
        "--json",
    ]);
    let history = run_ok(&["plan", "history", &out.to_string_lossy(), "--json"]);
    assert_eq!(history["total"], serde_json::json!(1));
    let snapshot = history["snapshots"][0]["path"].as_str().unwrap();
    assert!(Path::new(snapshot).is_file());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn append_then_revert_restores_the_previous_bytes() {
    let root = temp_root("revert");
    let a = root.join("a.json");
    let b = root.join("b.json");
    text_plan(&a, "a", 30, 1_000_000, "A1");
    text_plan(&b, "b", 30, 1_000_000, "B1");
    let out = root.join("out.json");
    run_ok(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        "--json",
    ]);
    let before = fs::read_to_string(&out).unwrap();

    let appended = run_ok(&[
        "plan",
        "append",
        &out.to_string_lossy(),
        "--from",
        &b.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(appended["duration_us"], serde_json::json!(2_000_000));
    let after = fs::read_to_string(&out).unwrap();
    assert_ne!(before, after, "append 必须改变计划");

    let reverted = run_ok(&["plan", "revert", &out.to_string_lossy(), "--json"]);
    assert_eq!(reverted["duration_us"], serde_json::json!(1_000_000));
    assert_eq!(
        before,
        fs::read_to_string(&out).unwrap(),
        "回滚必须逐字节还原"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn compose_rejects_canvas_mismatch() {
    let root = temp_root("canvas");
    let a = root.join("a.json");
    let b = root.join("b.json");
    text_plan(&a, "a", 30, 1_000_000, "A1");
    text_plan(&b, "b", 60, 1_000_000, "B1");
    let out = root.join("out.json");
    let failed = run(&[
        "plan",
        "compose",
        "--out",
        &out.to_string_lossy(),
        "--from",
        &a.to_string_lossy(),
        &b.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(failed.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&failed.stdout).contains("canvas mismatch"));
    assert!(!out.exists(), "失败时不得写出输出文件");
    let _ = fs::remove_dir_all(root);
}

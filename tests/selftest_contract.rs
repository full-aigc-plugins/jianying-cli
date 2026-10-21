//! 自检合同：一条命令给出 PASS/FAIL/SKIP 矩阵，缺依赖时如实 SKIP 而不是假通过。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/empty-processes.txt"
            )),
        )
        .output()
        .unwrap()
}

fn temp_dir(label: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("jianying-selftest-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn status_of(report: &Value, id: &str) -> String {
    report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("缺少检查项 {id}"))["status"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn selftest_reports_a_coherent_matrix() {
    let output = run(&["selftest", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    let report = &envelope["data"];
    assert_eq!(report["schema"], json!("jianying-selftest/v1"));
    assert_eq!(report["ok"], json!(true));

    let summary = &report["summary"];
    let total = summary["total"].as_i64().unwrap();
    assert!(total >= 7, "检查项太少: {total}");
    assert_eq!(summary["fail"], json!(0));
    assert_eq!(
        summary["pass"].as_i64().unwrap() + summary["skip"].as_i64().unwrap(),
        total
    );

    // 纯本机检查必须通过，不允许 SKIP 掩盖问题
    for id in ["capabilities", "skills", "mcp", "plan", "draft"] {
        assert_eq!(status_of(report, id), "pass", "{id} 应为 pass");
    }
    // 依赖 ffmpeg 的项：有则 pass，无则 skip，不能 fail
    for id in ["media", "render"] {
        let status = status_of(report, id);
        assert!(
            status == "pass" || status == "skip",
            "{id} 状态异常: {status}"
        );
    }
}

#[test]
fn selftest_keeps_or_cleans_its_workdir() {
    let kept = temp_dir("keep");
    let output = run(&[
        "selftest",
        "--workdir",
        &kept.to_string_lossy(),
        "--keep",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(kept.is_dir(), "--keep 应保留工作目录");
    let _ = fs::remove_dir_all(&kept);

    let cleaned = temp_dir("clean");
    let output = run(&[
        "selftest",
        "--workdir",
        &cleaned.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(!cleaned.exists(), "默认应清理工作目录");
}

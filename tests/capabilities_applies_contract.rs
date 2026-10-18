//! 能力适用性合同：`applies_to` 可声明草稿线格式，非法声明必须失败。

use serde_json::{json, Value};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            std::env::temp_dir().join("no-editor.txt"),
        )
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

fn capability<'a>(manifest: &'a Value, id: &str) -> &'a Value {
    manifest["capabilities"]
        .as_array()
        .expect("capabilities")
        .iter()
        .find(|entry| entry["id"] == json!(id))
        .unwrap_or_else(|| panic!("缺少能力 {id}"))
}

#[test]
fn plain_capabilities_output_is_unchanged() {
    let manifest = run_ok(&["capabilities", "--json"]);
    let entries = manifest["capabilities"].as_array().unwrap();
    assert!(entries.len() >= 150, "能力条目太少: {}", entries.len());
    assert!(
        manifest.get("applicability").is_none(),
        "不带 --draft-schema 时不应出现适用性汇总"
    );
    for entry in entries {
        assert!(
            entry.get("applicable").is_none(),
            "不带 --draft-schema 时不应逐条标注 applicable"
        );
    }
}

#[test]
fn draft_schema_flag_annotates_applicability() {
    let draft_info = run_ok(&[
        "capabilities",
        "--draft-schema",
        "draft_info.json",
        "--json",
    ]);
    assert_eq!(draft_info["draft_schema"], json!("draft_info.json"));
    let total = draft_info["capabilities"].as_array().unwrap().len() as i64;
    let applicable = draft_info["applicability"]["applicable"].as_i64().unwrap();
    let not_applicable = draft_info["applicability"]["not_applicable"]
        .as_i64()
        .unwrap();
    assert_eq!(applicable + not_applicable, total);
    assert_eq!(
        capability(&draft_info, "protocol.pyjyd_wire")["applicable"],
        json!(true),
        "pyJYD 线格式应适用于 draft_info.json"
    );

    let draft_content = run_ok(&[
        "capabilities",
        "--draft-schema",
        "draft_content.json",
        "--json",
    ]);
    assert_eq!(
        capability(&draft_content, "protocol.pyjyd_wire")["applicable"],
        json!(false),
        "pyJYD 线格式不应适用于 draft_content.json"
    );
    assert!(
        draft_content["applicability"]["not_applicable"]
            .as_i64()
            .unwrap()
            >= 1
    );
}

#[test]
fn unknown_draft_schema_is_rejected() {
    let output = run(&["capabilities", "--draft-schema", "draft_v9.json", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("unknown draft schema"),
        "错误信息应说明未知格式: {text}"
    );
}

//! 技能注册表合同：状态自洽，且引用的命令必须真实存在（防文档漂移）。

use serde_json::Value;
use std::collections::BTreeSet;
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

/// 命令目录里的全部路径，例如 "timeline speed"。
fn command_paths() -> BTreeSet<String> {
    let catalog = run_ok(&["commands", "--json"]);
    catalog["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn every_skill_is_self_consistent() {
    let registry = run_ok(&["skills", "list", "--json"]);
    assert_eq!(registry["schema"], serde_json::json!("jianying-skills/v1"));
    let skills = registry["skills"].as_array().unwrap();
    assert_eq!(registry["total"], serde_json::json!(skills.len()));
    assert!(skills.len() >= 12, "技能表过小: {}", skills.len());

    let mut ids = BTreeSet::new();
    for skill in skills {
        let id = skill["id"].as_str().unwrap();
        assert!(ids.insert(id.to_owned()), "重复技能 id: {id}");
        assert!(!skill["intent"].as_str().unwrap().is_empty());
        assert!(!skill["evidence"].as_str().unwrap().is_empty());
        let status = skill["status"].as_str().unwrap();
        let commands = skill["commands"].as_array().unwrap();
        match status {
            "supported" => {
                assert!(!commands.is_empty(), "{id} supported 但没有命令");
                assert_eq!(
                    skill["gap"],
                    serde_json::json!(""),
                    "{id} supported 却写了缺口"
                );
            }
            "partial" | "missing" => {
                assert!(
                    !skill["gap"].as_str().unwrap().is_empty(),
                    "{id} 是 {status} 必须写缺口"
                );
            }
            other => panic!("未知状态 {other}"),
        }
    }
}

#[test]
fn skill_commands_all_exist_in_the_command_catalog() {
    let paths = command_paths();
    let registry = run_ok(&["skills", "list", "--json"]);
    let mut missing = Vec::new();
    for skill in registry["skills"].as_array().unwrap() {
        for command in skill["commands"].as_array().unwrap() {
            let path = command.as_str().unwrap();
            if !paths.contains(path) {
                missing.push(format!("{} -> {path}", skill["id"].as_str().unwrap()));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "技能引用了不存在的命令（文档与实现漂移）: {missing:?}"
    );
}

#[test]
fn skills_show_and_status_filter_work() {
    let review = run_ok(&["skills", "show", "review", "--json"]);
    assert_eq!(review["id"], serde_json::json!("review"));
    assert!(review["commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|command| command == "loop"));

    let missing = run_ok(&["skills", "list", "--status", "missing", "--json"]);
    assert_eq!(missing["status"], serde_json::json!("missing"));
    let ids: Vec<&str> = missing["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|skill| skill["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&"beat-sync"), "ids={ids:?}");

    let unknown = run(&["skills", "show", "nope", "--json"]);
    assert_eq!(unknown.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&unknown.stdout).contains("unknown skill"));
}

//! editor-skill 整合合同：映射里的命令必须真实存在，覆盖率报告的统计必须自洽。

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::Command;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn catalog_paths() -> BTreeSet<String> {
    let output = test_command()
        .args(["commands", "--json"])
        .output()
        .expect("run commands --json");
    assert!(output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    envelope["data"]["commands"]
        .as_array()
        .expect("data.commands")
        .iter()
        .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
        .collect()
}

fn mapping() -> Value {
    let path = root().join("provenance/EDITOR_SKILL_MAPPING.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("mapping exists")).expect("mapping json")
}

fn coverage() -> Value {
    let path = root().join("provenance/EDITOR_SKILL_COVERAGE.json");
    serde_json::from_str(&fs::read_to_string(&path).expect("coverage exists"))
        .expect("coverage json")
}

/// 把 `a b + c d` 拆成单条命令路径（去掉括号里的说明）。
fn command_paths(spec: &str) -> Vec<String> {
    spec.split('+')
        .map(|chunk| {
            let mut text = chunk.trim().to_string();
            if let Some(position) = text.find(['（', '(']) {
                text.truncate(position);
            }
            text.trim().to_string()
        })
        .filter(|text| !text.is_empty())
        .collect()
}

fn entries<'a>(node: &'a Value, key: &str) -> Vec<(&'a str, &'a Value)> {
    node[key]
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(name, value)| (name.as_str(), value))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_mapping_entry_points_at_a_real_command() {
    let commands = catalog_paths();
    let mapping = mapping();
    let mut checked = 0_usize;

    for class in ["JyProject", "scripts"] {
        let node = &mapping["classes"][class];
        for group in ["methods", "items"] {
            for (name, entry) in entries(node, group) {
                let status = entry["status"].as_str().unwrap_or_default();
                assert!(
                    ["covered", "partial", "out_of_scope"].contains(&status),
                    "{class}/{name} 状态非法: {status}"
                );
                if status == "partial" {
                    assert!(
                        entry["missing"]
                            .as_array()
                            .map(|list| !list.is_empty())
                            .unwrap_or(false),
                        "{class}/{name} partial 必须写明缺什么"
                    );
                }
                if status == "out_of_scope" {
                    assert!(
                        entry["note"]
                            .as_str()
                            .map(|note| !note.is_empty())
                            .unwrap_or(false),
                        "{class}/{name} out_of_scope 必须写原因"
                    );
                }
                let Some(ours) = entry["ours"].as_str() else {
                    assert_eq!(status, "out_of_scope", "{class}/{name} 缺 ours");
                    continue;
                };
                for candidate in command_paths(ours) {
                    checked += 1;
                    let matched = commands.contains(&candidate)
                        || commands
                            .iter()
                            .any(|path| path.starts_with(&format!("{candidate} ")));
                    assert!(matched, "{class}/{name} 引用了不存在的命令: {candidate}");
                }
            }
        }
    }
    assert!(
        checked >= 15,
        "只校验了 {checked} 条命令引用，映射可能被清空"
    );
}

#[test]
fn coverage_report_matches_the_mapping_and_has_no_problems() {
    let mapping = mapping();
    let report = coverage();
    assert_eq!(
        report["schema"],
        serde_json::json!("jianying-editor-skill-coverage/v1")
    );
    assert_eq!(
        report["problems"].as_array().map(Vec::len).unwrap_or(1),
        0,
        "覆盖率报告仍有问题: {}",
        report["problems"]
    );

    let mut counted = std::collections::BTreeMap::new();
    for class in ["JyProject", "scripts"] {
        let node = &mapping["classes"][class];
        for group in ["methods", "items"] {
            for (_, entry) in entries(node, group) {
                let status = entry["status"].as_str().unwrap().to_owned();
                *counted.entry(status).or_insert(0_i64) += 1;
            }
        }
    }
    let by_status = &report["summary"]["by_status"];
    for (status, expected) in counted {
        assert_eq!(
            by_status[&status].as_i64().unwrap_or(0),
            expected,
            "{status} 计数与映射不一致"
        );
    }
    let methods = report["summary"]["methods"].as_i64().unwrap();
    let scripts = report["summary"]["scripts"].as_i64().unwrap();
    assert!(methods >= 10, "advertised methods 太少: {methods}");
    assert!(scripts >= 10, "scripts 太少: {scripts}");
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

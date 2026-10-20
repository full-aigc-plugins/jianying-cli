//! 插件交接契约的**自证**测试。
//!
//! `docs/integration/plugin-handoff.json` 声明了 jianying-edit-plugin 将要依赖的
//! 能力与命令。插件侧用 `requireCapabilities` 强制「每个 id 必须 status=supported」，
//! 所以本仓库必须先自证：声明的能力都是 supported，声明的命令都真实存在。

use serde_json::Value;
use std::collections::BTreeSet;
use std::process::Command;

fn handoff() -> Value {
    serde_json::from_str(include_str!("../docs/integration/plugin-handoff.json")).unwrap()
}

fn capabilities() -> Value {
    serde_json::from_str(include_str!("../provenance/CAPABILITIES.json")).unwrap()
}

fn command_paths() -> BTreeSet<String> {
    let output = test_command()
        .args(["commands", "--json"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    envelope["data"]["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn every_required_capability_is_supported() {
    let manifest = capabilities();
    let supported: BTreeSet<&str> = manifest["capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|capability| capability["status"] == "supported")
        .filter_map(|capability| capability["id"].as_str())
        .collect();

    let required = handoff()["requiredCapabilities"]
        .as_array()
        .unwrap()
        .clone();
    assert!(!required.is_empty());
    let mut missing = Vec::new();
    for capability in required {
        let id = capability.as_str().unwrap();
        if !supported.contains(id) {
            missing.push(id.to_owned());
        }
    }
    assert!(
        missing.is_empty(),
        "交接契约声明了未支持的能力（插件会在握手时直接拒绝）: {missing:?}"
    );
}

#[test]
fn every_handed_off_command_exists() {
    let paths = command_paths();
    let commands = handoff()["commands"].as_array().unwrap().clone();
    assert!(!commands.is_empty());
    let mut missing = Vec::new();
    for command in commands {
        let path = command.as_str().unwrap();
        if !paths.contains(path) {
            missing.push(path.to_owned());
        }
    }
    assert!(
        missing.is_empty(),
        "交接契约引用了不存在的命令: {missing:?}"
    );
}

#[test]
fn handoff_declares_owner_and_gate_for_each_step() {
    let handoff = handoff();
    assert_eq!(
        handoff["schema"],
        serde_json::json!("jianying-plugin-handoff/v1")
    );
    let steps = handoff["pluginSteps"].as_array().unwrap();
    assert!(steps.len() >= 3, "交接步骤过少: {}", steps.len());
    for step in steps {
        assert!(!step["owner"].as_str().unwrap().is_empty());
        assert!(
            matches!(
                step["gate"].as_str().unwrap(),
                "release" | "plugin-release" | "none"
            ),
            "未知 gate: {:?}",
            step["gate"]
        );
        assert!(!step["action"].as_str().unwrap().is_empty());
    }
    // 约束必须写明插件侧的发布强制与上游技能锁定
    let constraints = handoff["constraints"].as_array().unwrap();
    let joined = constraints
        .iter()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(joined.contains("bump"), "必须写明插件侧的版本强制");
    assert!(
        joined.contains("skills.lock.json"),
        "必须写明技能来自上游锁定"
    );
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

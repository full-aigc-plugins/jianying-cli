//! 文档命令合同：README/文档里出现的 `jianying <命令>` 必须真实存在。
//!
//! 文档落后于实现是最常见的交付缺陷；这条测试用机器目录做交叉验证，
//! 只要有人改了命令名而没改文档，就会红。

use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn catalog_paths() -> BTreeSet<String> {
    let output = test_command()
        .args(["commands", "--json"])
        .output()
        .expect("run commands --json");
    assert!(output.status.success(), "commands --json failed");
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    envelope["data"]["commands"]
        .as_array()
        .expect("data.commands")
        .iter()
        .filter_map(|entry| entry["path"].as_str().map(str::to_owned))
        .collect()
}

fn docs(root: &Path) -> Vec<PathBuf> {
    let mut files = vec![root.join("README.md")];
    let mut stack = vec![root.join("docs")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// 会吃掉一个值的全局参数；解析文档时要把它们连同值一起跳过。
const VALUE_FLAGS: &[&str] = &[
    "--profile",
    "--state-root",
    "--config",
    "--root",
    "--token-env",
    "--bind",
    "--allowed-host",
    "--allowed-origin",
];

/// 从 `jianying ` 之后抽出真正的命令片段（跳过全局参数与占位符）。
fn command_after(candidate: &str) -> Option<String> {
    let rest = candidate.trim_start().strip_prefix("jianying ")?;
    let stop = rest
        .find(['|', '#', '<', '`', ']', ')', '\\'])
        .unwrap_or(rest.len());
    let raw = &rest[..stop];
    if raw.contains('/') {
        // 文档里 "job run/batch/list" 这类简写不当作具体命令
        return None;
    }
    let tokens: Vec<&str> = raw.split_whitespace().collect();
    let mut index = 0;
    while index < tokens.len() {
        let token = tokens[index];
        if token.starts_with("--") {
            let takes_value = VALUE_FLAGS.contains(&token.split('=').next().unwrap_or(token));
            index += if takes_value && !token.contains('=') {
                2
            } else {
                1
            };
            continue;
        }
        break;
    }
    if index >= tokens.len() {
        return None;
    }
    let first = tokens[index];
    let looks_like_command = first
        .chars()
        .next()
        .map(|c| c.is_ascii_lowercase())
        .unwrap_or(false)
        && first
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !looks_like_command {
        return None;
    }
    let mut path: Vec<&str> = Vec::new();
    for token in &tokens[index..] {
        if token.starts_with('-') || token.contains('=') || token.starts_with('[') {
            break;
        }
        path.push(token);
    }
    if path.is_empty() {
        None
    } else {
        Some(path.join(" "))
    }
}

#[test]
fn every_documented_command_exists_in_the_catalog() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let paths = catalog_paths();
    let mut missing: Vec<String> = Vec::new();
    let mut checked = 0_usize;

    for file in docs(root) {
        let text = fs::read_to_string(&file).unwrap();
        for (index, line) in text.lines().enumerate() {
            // 支持 `jianying ...` 出现在行首、注释后或代码块里的任意位置
            let mut rest = line;
            while let Some(position) = rest.find("jianying ") {
                let candidate = &rest[position..];
                if let Some(command) = command_after(candidate) {
                    // 贪心取目录里存在的最长前缀
                    let tokens: Vec<&str> = command.split_whitespace().collect();
                    checked += 1;
                    let mut matched = false;
                    for take in (1..=tokens.len()).rev() {
                        let candidate = tokens[..take].join(" ");
                        if paths.contains(&candidate) {
                            matched = true;
                            break;
                        }
                        // 只写组名（如 `jianying timeline`）也算存在
                        let prefix = format!("{candidate} ");
                        if paths.iter().any(|path| path.starts_with(&prefix)) {
                            matched = true;
                            break;
                        }
                    }
                    if !matched {
                        let relative = file.strip_prefix(root).unwrap_or(&file);
                        missing.push(format!(
                            "{}:{}: jianying {}",
                            relative.display(),
                            index + 1,
                            command
                        ));
                    }
                }
                rest = &rest[position + "jianying".len()..];
            }
        }
    }

    // 防止解析器静默失效导致"空检查也算通过"
    assert!(
        checked >= 20,
        "只解析出 {checked} 条文档命令，解析规则可能已失效"
    );
    assert!(
        missing.is_empty(),
        "文档引用了不存在的命令：\n{}",
        missing.join("\n")
    );
}

#[test]
fn parser_handles_global_flags_and_rejects_prose() {
    // 保留位置参数，靠"最长前缀命中目录"来判定，无需猜测哪个是参数
    assert_eq!(
        command_after("jianying project build plan.json --out d1").as_deref(),
        Some("project build plan.json")
    );
    assert_eq!(
        command_after("jianying --profile studio config get").as_deref(),
        Some("config get")
    );
    assert_eq!(
        command_after("jianying --json media transcribe a.wav").as_deref(),
        Some("media transcribe a.wav")
    );
    assert_eq!(command_after("jianying --help"), None);
    assert_eq!(command_after("jianying 智能体接口契约"), None);
    assert_eq!(command_after("jianying job run/batch/list"), None);
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

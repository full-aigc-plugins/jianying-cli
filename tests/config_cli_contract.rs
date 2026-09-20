use serde_json::Value;
use std::path::PathBuf;
use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
}

fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("jyc-config-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn run(root: &PathBuf, args: &[&str]) -> Output {
    bin()
        .env("JIANYING_CONFIG_ROOT", root)
        .args(args)
        .output()
        .unwrap()
}

fn json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn profiles_are_isolated_and_read_only_host_still_allows_reads() {
    let root = temp_root();
    let set = run(
        &root,
        &[
            "--profile",
            "alpha",
            "config",
            "set",
            "render.quality",
            "\"high\"",
            "--json",
        ],
    );
    assert!(
        set.status.success(),
        "{}",
        String::from_utf8_lossy(&set.stderr)
    );

    let patch = run(
        &root,
        &[
            "--profile",
            "alpha",
            "config",
            "patch",
            r#"{"render":{"threads":4},"enabled":true}"#,
            "--json",
        ],
    );
    assert!(patch.status.success());
    let alpha = run(
        &root,
        &["--profile", "alpha", "config", "get", "render", "--json"],
    );
    assert_eq!(json(&alpha)["data"]["quality"], "high");
    assert_eq!(json(&alpha)["data"]["threads"], 4);

    let beta = run(&root, &["--profile", "beta", "config", "get", "--json"]);
    assert_eq!(json(&beta)["data"]["values"], serde_json::json!({}));
    assert!(root.join("profiles/alpha.json").is_file());
    assert!(!root.join("profiles/beta.json").exists());

    let read_only = bin()
        .env("JIANYING_CONFIG_ROOT", &root)
        .args([
            "--profile",
            "alpha",
            "--host-read-only",
            "config",
            "unset",
            "render.quality",
            "--json",
        ])
        .output()
        .unwrap();
    assert_eq!(read_only.status.code(), Some(1));
    assert_eq!(json(&read_only)["error"]["type"], "config_read_only");

    let validate = bin()
        .env("JIANYING_CONFIG_ROOT", &root)
        .args([
            "--profile",
            "alpha",
            "--host-read-only",
            "config",
            "validate",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(validate.status.success());
    assert_eq!(json(&validate)["data"]["valid"], true);
}

#[test]
fn config_schema_is_machine_readable() {
    let root = temp_root();
    let schema = run(&root, &["config", "schema", "--json"]);
    assert!(schema.status.success());
    assert_eq!(
        json(&schema)["data"]["$id"],
        "https://partme.ai/schemas/jianying-config-v1.schema.json"
    );
}

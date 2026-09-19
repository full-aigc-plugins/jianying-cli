//! Agent contract tests — exercise the real binary the way an agent would:
//! exit codes, single-JSON stdout, error chains on stderr, --help surface.

use std::process::{Command, Output};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().unwrap()
}

fn stdout_json(o: &Output) -> serde_json::Value {
    let text = String::from_utf8_lossy(&o.stdout);
    serde_json::from_str(text.trim())
        .unwrap_or_else(|e| panic!("stdout is not a single JSON object: {e}\n{text}"))
}

#[test]
fn help_surface_is_complete() {
    for args in [
        vec!["--help"],
        vec!["help"],
        vec!["doctor", "--help"],
        vec!["probe", "--help"],
        vec!["build", "--help"],
        vec!["verify", "--help"],
        vec!["inspect", "--help"],
        vec!["publish", "--help"],
        vec!["render", "--help"],
        vec!["catalog", "--help"],
        vec!["template", "--help"],
        vec!["store", "--help"],
        vec!["template", "inspect", "--help"],
        vec!["template", "duplicate", "--help"],
        vec!["template", "replace-text", "--help"],
        vec!["template", "replace-material", "--help"],
        vec!["template", "import-track", "--help"],
        vec!["store", "list", "--help"],
        vec!["store", "has", "--help"],
        vec!["store", "remove", "--help"],
    ] {
        let out = run(&args);
        assert_eq!(out.status.code(), Some(0), "--help must exit 0: {args:?}");
        let help = String::from_utf8_lossy(&out.stdout);
        assert!(
            help.contains("Usage:") || help.contains("usage:"),
            "{args:?}"
        );
    }
    // root help carries the agent I/O contract and version
    let root = run(&["--help"]).stdout;
    let root = String::from_utf8_lossy(&root);
    assert!(root.contains("I/O 契约"));
    assert!(root.contains("退出码"));
    assert!(root.contains("tim()"));
    assert!(run(&["--version"]).status.success());
}

#[test]
fn doctor_emits_one_json_object_and_exits_zero() {
    let out = run(&["doctor"]);
    assert_eq!(out.status.code(), Some(0));
    let v = stdout_json(&out);
    for key in [
        "version",
        "plan_schema",
        "draft_roots",
        "editors_running",
        "ffprobe",
        "ffmpeg",
    ] {
        assert!(v.get(key).is_some(), "doctor output missing {key}");
    }
}

#[test]
fn runtime_errors_exit_1_with_stderr_error_chain() {
    // unreadable plan file
    let out = run(&["build", "/nonexistent/plan.json", "--out", "/tmp/x"]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.starts_with("error:"),
        "stderr must lead with `error:`: {err}"
    );
    // invalid plan JSON
    let tmp = std::env::temp_dir().join("jyc-agent-bad-plan.json");
    std::fs::write(&tmp, "{ not json").unwrap();
    let out = run(&["build", tmp.to_str().unwrap(), "--out", "/tmp/x2"]);
    assert_eq!(out.status.code(), Some(1));
    // unknown catalog domain
    let out = run(&["catalog", "--domain", "nope"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unknown domain"));
    // remove without --yes refuses
    let root = std::env::temp_dir().join("jyc-agent-store");
    std::fs::create_dir_all(&root).unwrap();
    let out = run(&[
        "store",
        "remove",
        "whatever",
        "--root",
        root.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(1));
}

#[test]
fn usage_errors_exit_2() {
    // missing required positional
    let out = run(&["build"]);
    assert_eq!(out.status.code(), Some(2));
    // unknown flag
    let out = run(&["doctor", "--nope"]);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn catalog_listing_and_search_contract() {
    let out = run(&["catalog"]);
    assert_eq!(out.status.code(), Some(0));
    let v = stdout_json(&out);
    let domains = v["domains"].as_array().unwrap();
    assert_eq!(domains.len(), 16);
    assert!(domains
        .iter()
        .all(|d| d["entries"].as_u64().unwrap_or(0) > 0));

    let out = run(&["catalog", "--domain", "transitions", "--search", "叠化"]);
    let v = stdout_json(&out);
    assert_eq!(v["count"], 1);
    assert_eq!(v["results"][0]["name"], "叠化");
    assert_eq!(v["results"][0]["effect_id"], "322577");

    // VIP entries are filtered by default and appear with --include-vip
    let plain = stdout_json(&run(&["catalog", "--domain", "transitions"]));
    let with_vip = stdout_json(&run(&[
        "catalog",
        "--domain",
        "transitions",
        "--include-vip",
    ]));
    assert_eq!(
        plain["count"].as_u64().unwrap() + 323,
        with_vip["count"].as_u64().unwrap()
    );
}

#[test]
fn build_with_tim_strings_produces_verifiable_draft() {
    let tmp = std::env::temp_dir().join(format!("jyc-agent-build-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    // ffmpeg-synthesized media keeps this self-contained
    let media = tmp.join("a.mp4");
    let ok = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:d=2",
            "-y",
        ])
        .arg(&media)
        .output()
        .unwrap();
    if !ok.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }
    let plan = tmp.join("plan.json");
    std::fs::write(
        &plan,
        serde_json::json!({
            "schema": "jianying-cli-plan/v1", "name": "agent-contract",
            "canvas": {"width": 640, "height": 360, "fps": 30},
            "tracks": [{"type": "video", "segments": [
                {"start_us": "0s", "duration_us": "2s", "source": "a.mp4"}]}]
        })
        .to_string(),
    )
    .unwrap();
    let out = tmp.join("draft");
    let o = run(&[
        "build",
        plan.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let report = stdout_json(&o);
    assert_eq!(report["duration_us"], 2_000_000);
    assert!(out.join("draft_content.json").is_file());
    assert!(out.join("draft_info.json").is_file());
    assert!(out.join("draft_meta_info.json").is_file());
    // verify + inspect also honor the single-JSON contract
    let v = stdout_json(&run(&["verify", out.to_str().unwrap()]));
    assert_eq!(v["ok"], serde_json::json!(true));
    let ins = stdout_json(&run(&["inspect", out.to_str().unwrap()]));
    assert_eq!(ins["duration_us"], 2_000_000);
    // refusing to overwrite the same output dir
    let again = run(&[
        "build",
        plan.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(again.status.code(), Some(1));
}

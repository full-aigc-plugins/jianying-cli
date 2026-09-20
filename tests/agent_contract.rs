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
        vec!["status", "--help"],
        vec!["audit", "--help"],
        vec!["completion", "--help"],
        vec!["capabilities", "--help"],
        vec!["commands", "--help"],
        vec!["probe", "--help"],
        vec!["build", "--help"],
        vec!["verify", "--help"],
        vec!["inspect", "--help"],
        vec!["publish", "--help"],
        vec!["render", "--help"],
        vec!["render", "proxy", "--help"],
        vec!["render", "status", "--help"],
        vec!["project", "--help"],
        vec!["project", "status", "--help"],
        vec!["project", "init", "--help"],
        vec!["project", "quickstart", "--help"],
        vec!["project", "build", "--help"],
        vec!["project", "verify", "--help"],
        vec!["project", "inspect", "--help"],
        vec!["project", "info", "--help"],
        vec!["project", "version", "--help"],
        vec!["project", "describe", "--help"],
        vec!["project", "diff", "--help"],
        vec!["project", "migrate", "--help"],
        vec!["project", "concat", "--help"],
        vec!["timeline", "--help"],
        vec!["timeline", "status", "--help"],
        vec!["media", "--help"],
        vec!["media", "status", "--help"],
        vec!["media", "probe", "--help"],
        vec!["media", "catalog", "--help"],
        vec!["captions", "--help"],
        vec!["captions", "status", "--help"],
        vec!["runtime", "--help"],
        vec!["runtime", "status", "--help"],
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
        vec!["store", "publish", "--help"],
        vec!["job", "--help"],
        vec!["job", "run", "--help"],
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
    assert!(root.contains("--profile"));
    assert!(root.contains("--no-color"));
    assert!(root.contains("--log-level"));
    assert!(run(&["--version"]).status.success());
}

#[test]
fn machine_command_catalog_covers_every_domain_with_schemas_and_access_levels() {
    let output = run(&["commands", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let envelope = stdout_json(&output);
    let catalog = &envelope["data"];
    assert_eq!(catalog["schema"], "jianying-command-catalog/v1");
    let commands = catalog["commands"].as_array().unwrap();
    let expected_groups = [
        "project",
        "timeline",
        "media",
        "captions",
        "template",
        "store",
        "render",
        "job",
        "runtime",
        "approvals",
        "config",
        "mcp",
    ];
    for group in expected_groups {
        assert!(
            commands.iter().any(|entry| entry["group"] == group),
            "{group}"
        );
    }
    for command in commands {
        assert!(command["path"]
            .as_str()
            .is_some_and(|path| !path.is_empty()));
        assert_eq!(command["input_schema"]["type"], "object");
        assert!(matches!(
            command["access"].as_str(),
            Some("read") | Some("write") | Some("service")
        ));
        assert!(command["platforms"]
            .as_array()
            .is_some_and(|items| !items.is_empty()));
        assert!(command["output_schema"]["$ref"].is_string());
        assert!(matches!(
            command["status"].as_str(),
            Some("supported") | Some("partial") | Some("external_dependency")
        ));
    }
    for global in ["json", "profile", "no-color", "log-level"] {
        assert!(catalog["global_options"]["properties"]
            .get(global)
            .is_some());
    }
    let legacy_build = commands
        .iter()
        .find(|entry| entry["path"] == "build")
        .unwrap();
    assert_eq!(legacy_build["deprecated"], true);
    assert_eq!(legacy_build["replacement"], "project build");
    let grouped_build = commands
        .iter()
        .find(|entry| entry["path"] == "project build")
        .unwrap();
    assert_eq!(grouped_build["deprecated"], false);
    assert!(grouped_build["replacement"].is_null());

    let capabilities_output = run(&["capabilities", "--json"]);
    let capabilities_envelope = stdout_json(&capabilities_output);
    let capabilities = &capabilities_envelope["data"];
    assert_eq!(
        capabilities["command_catalog"]["schema"],
        "jianying-command-catalog/v1"
    );
}

#[test]
fn domain_group_skeletons_are_machine_readable_and_legacy_render_still_works() {
    for group in ["project", "timeline", "media", "captions", "runtime"] {
        let output = run(&[group, "status", "--json"]);
        assert_eq!(output.status.code(), Some(0), "{group}");
        let envelope = stdout_json(&output);
        assert_eq!(envelope["ok"], true);
        assert_eq!(envelope["data"]["group"], group);
        assert!(envelope["data"]["status"].is_string());
    }

    let render = run(&["render", "status", "--json"]);
    assert_eq!(render.status.code(), Some(0));
    assert_eq!(stdout_json(&render)["data"]["group"], "render");

    let legacy = run(&["render", "/missing/draft", "--out", "/tmp/missing.mp4"]);
    assert_eq!(legacy.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&legacy.stderr).starts_with("warning: deprecated"));
}

#[test]
fn global_machine_flags_are_accepted_before_or_after_subcommands() {
    for args in [
        vec!["--no-color", "--log-level", "debug", "doctor", "--json"],
        vec!["doctor", "--json", "--no-color", "--log-level", "warn"],
    ] {
        let output = run(&args);
        assert_eq!(output.status.code(), Some(0), "{args:?}");
        assert_eq!(stdout_json(&output)["ok"], true);
    }
}

#[test]
fn status_audit_and_completion_are_operational_and_redacted() {
    let state_root = std::env::temp_dir().join(format!(
        "jianying-sensitive-customer-state-{}",
        std::process::id()
    ));
    let state_root_text = state_root.to_string_lossy().into_owned();
    let status = bin()
        .args(["status", "--state-root", &state_root_text, "--json"])
        .env("JIANYING_MCP_TOKEN", "do-not-leak-this-token")
        .output()
        .unwrap();
    assert_eq!(status.status.code(), Some(0));
    let status_text = String::from_utf8_lossy(&status.stdout);
    assert!(!status_text.contains("do-not-leak-this-token"));
    assert!(!status_text.contains(&state_root_text));
    let status_json = stdout_json(&status);
    assert!(matches!(
        status_json["data"]["status"].as_str(),
        Some("ready") | Some("degraded")
    ));
    assert_eq!(status_json["data"]["security"]["remote_auth"], "configured");

    let audit = run(&["audit", "--state-root", &state_root_text, "--json"]);
    assert_eq!(audit.status.code(), Some(0));
    let audit_text = String::from_utf8_lossy(&audit.stdout);
    assert!(!audit_text.contains(&state_root_text));
    let audit_json = stdout_json(&audit);
    assert_eq!(audit_json["data"]["append_only"], true);
    assert!(audit_json["data"]["records"].is_array());

    for (shell, marker) in [
        ("bash", "complete"),
        ("zsh", "#compdef"),
        ("fish", "function"),
    ] {
        let completion = run(&["completion", shell]);
        assert_eq!(completion.status.code(), Some(0));
        let completion = String::from_utf8_lossy(&completion.stdout);
        assert!(completion.contains("jianying"));
        assert!(completion.contains(marker));
    }

    let _ = std::fs::remove_dir_all(state_root);
}

#[test]
fn global_json_mode_uses_one_success_or_failure_envelope() {
    let success = run(&["doctor", "--json"]);
    assert_eq!(success.status.code(), Some(0));
    assert!(success.stderr.is_empty());
    let success = stdout_json(&success);
    assert_eq!(success["ok"], true);
    assert!(success["data"]["version"].is_string());

    let failure = run(&[
        "project",
        "build",
        "/nonexistent/plan.json",
        "--out",
        "/tmp/x",
        "--json",
    ]);
    assert_eq!(failure.status.code(), Some(1));
    assert!(failure.stderr.is_empty());
    let failure = stdout_json(&failure);
    assert_eq!(failure["ok"], false);
    assert_eq!(failure["error"]["type"], "execution_failed");
}

#[test]
fn capabilities_exposes_the_runtime_handshake_contract() {
    let output = run(&["capabilities", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let envelope = stdout_json(&output);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["schema"], "jianying-capabilities/v1");
    assert_eq!(envelope["data"]["cli_version"], env!("CARGO_PKG_VERSION"));
    for required in [
        "schema.job_v2",
        "schema.compile_v1_compat",
        "project.compile",
        "job.run",
        "media.audio",
        "timeline.quantization_report",
    ] {
        assert!(envelope["data"]["capabilities"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["id"] == required && item["status"] == "supported"));
    }
    let capabilities = envelope["data"]["capabilities"].as_array().unwrap();
    let ids = capabilities
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        ids.len(),
        capabilities.len(),
        "capability ids must be unique"
    );
    let windows = capabilities
        .iter()
        .find(|item| item["id"] == "runtime.profile.windows")
        .unwrap();
    assert_eq!(windows["availability"], "unsupported");
    assert_ne!(windows["status"], "supported");
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
    let out = run(&[
        "project",
        "build",
        "/nonexistent/plan.json",
        "--out",
        "/tmp/x",
    ]);
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.starts_with("error:"),
        "stderr must lead with `error:`: {err}"
    );
    // invalid plan JSON
    let tmp = std::env::temp_dir().join("jyc-agent-bad-plan.json");
    std::fs::write(&tmp, "{ not json").unwrap();
    let out = run(&[
        "project",
        "build",
        tmp.to_str().unwrap(),
        "--out",
        "/tmp/x2",
    ]);
    assert_eq!(out.status.code(), Some(1));
    // unknown catalog domain
    let out = run(&["media", "catalog", "--domain", "nope"]);
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
fn enums_expose_fixed_capcut_and_jianying_namespaces() {
    let capcut = stdout_json(&run(&[
        "media",
        "enums",
        "transitions",
        "--namespace",
        "capcut",
        "--json",
    ]));
    assert_eq!(capcut["ok"], true);
    assert_eq!(capcut["data"]["category"], "transitions");
    assert_eq!(capcut["data"]["namespace"], "capcut");
    assert_eq!(capcut["data"]["count"], 116);
    assert_eq!(capcut["data"]["entries"][0]["member"], "Montage_Snippets");

    let jianying = stdout_json(&run(&[
        "media",
        "enums",
        "filters",
        "--namespace",
        "jianying",
        "--json",
    ]));
    assert_eq!(jianying["data"]["count"], 468);
    assert_eq!(
        jianying["data"]["entries"][0]["effect_id"],
        "7127828208690433311"
    );

    let bubbles = stdout_json(&run(&[
        "media",
        "enums",
        "bubbles",
        "--namespace",
        "jianying",
        "--json",
    ]));
    assert_eq!(bubbles["data"]["count"], 7);

    let human = run(&[
        "media",
        "enums",
        "masks",
        "--namespace",
        "capcut",
        "--human",
    ]);
    assert_eq!(human.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&human.stdout).contains("Slug"));
    assert!(String::from_utf8_lossy(&human.stderr).contains("9 masks (capcut)"));

    let invalid = run(&["media", "enums", "unknown", "--json"]);
    assert_eq!(invalid.status.code(), Some(2));
}

#[test]
fn harvest_enums_manual_add_is_plan_first_atomic_and_fail_closed() {
    let root = std::env::temp_dir().join(format!(
        "jianying-harvest-enums-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let catalogue = root.join("user-enums.json");
    let catalogue_text = catalogue.to_string_lossy();
    let plan = stdout_json(&run(&[
        "media",
        "harvest-enums",
        "add",
        "video_effects",
        "snow-fly",
        "resource-1",
        "--effect-id",
        "effect-1",
        "--catalogue",
        &catalogue_text,
        "--json",
    ]));
    assert_eq!(plan["data"]["applied"], false);
    assert!(!catalogue.exists());

    let applied = stdout_json(&run(&[
        "media",
        "harvest-enums",
        "add",
        "video_effects",
        "snow-fly",
        "resource-1",
        "--effect-id",
        "effect-1",
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]));
    assert_eq!(applied["data"]["added"], 1);
    let stored: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&catalogue).unwrap()).unwrap();
    assert_eq!(stored["version"], 1);
    assert_eq!(stored["entries"][0]["slug"], "snow-fly");

    let duplicate = run(&[
        "media",
        "harvest-enums",
        "add",
        "filters",
        "other",
        "resource-1",
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(duplicate.status.code(), Some(1));
    let id_only = run(&[
        "media",
        "harvest-enums",
        "add",
        "animations",
        "unsafe",
        "resource-2",
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(id_only.status.code(), Some(1));
    let bad_slug = run(&[
        "media",
        "harvest-enums",
        "add",
        "filters",
        "Bad Slug",
        "resource-2",
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(bad_slug.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn deprecated_top_level_aliases_share_domain_handlers_without_output_drift() {
    let legacy = run(&[
        "catalog",
        "--domain",
        "transitions",
        "--search",
        "叠化",
        "--json",
    ]);
    let grouped = run(&[
        "media",
        "catalog",
        "--domain",
        "transitions",
        "--search",
        "叠化",
        "--json",
    ]);
    assert_eq!(legacy.status.code(), grouped.status.code());
    assert_eq!(legacy.stdout, grouped.stdout);
    assert!(String::from_utf8_lossy(&legacy.stderr).starts_with("warning: deprecated"));
    assert!(grouped.stderr.is_empty());

    let missing_legacy = run(&["inspect", "/missing/draft", "--json"]);
    let missing_grouped = run(&["project", "inspect", "/missing/draft", "--json"]);
    assert_eq!(missing_legacy.status.code(), Some(1));
    assert_eq!(missing_legacy.status.code(), missing_grouped.status.code());
    assert_eq!(missing_legacy.stdout, missing_grouped.stdout);
    assert!(String::from_utf8_lossy(&missing_legacy.stderr).starts_with("warning: deprecated"));
    assert!(missing_grouped.stderr.is_empty());
}

#[test]
fn project_read_commands_report_versions_composition_and_pointer_diffs() {
    let root = std::env::temp_dir().join(format!("jianying-project-read-{}", std::process::id()));
    let left = root.join("left");
    let right = root.join("right");
    std::fs::create_dir_all(&left).unwrap();
    std::fs::create_dir_all(&right).unwrap();
    let base = serde_json::json!({
        "name":"read-fixture",
        "duration":1_000_000,
        "fps":30.0,
        "platform":{"os":"mac"},
        "version": 360000,
        "canvas_config":{"width":1920,"height":1080,"ratio":"16:9"},
        "materials":{"videos":[{"id":"m1","path":"/tmp/a.mp4"}],"audios":[]},
        "tracks":[{"type":"video","name":"main","segments":[{
            "id":"s1","material_id":"m1","target_timerange":{"start":0,"duration":1_000_000},
            "speed":1.0,"volume":1.0
        }]}]
    });
    for draft in [&left, &right] {
        std::fs::write(
            draft.join("draft_content.json"),
            serde_json::to_vec_pretty(&base).unwrap(),
        )
        .unwrap();
        std::fs::write(
            draft.join("draft_info.json"),
            serde_json::to_vec_pretty(&base).unwrap(),
        )
        .unwrap();
        std::fs::write(
            draft.join("draft_meta_info.json"),
            br#"{"draft_version":360000,"app_version":"9.9.0","platform":"mac"}"#,
        )
        .unwrap();
    }
    let left_text = left.to_string_lossy();
    let right_text = right.to_string_lossy();

    let info = stdout_json(&run(&["project", "info", &left_text, "--json"]));
    assert_eq!(info["data"]["tracks"], 1);
    assert_eq!(info["data"]["segments"], 1);
    assert_eq!(info["data"]["material_types"], 2);
    assert_eq!(info["data"]["material_summary"][0]["count"], 1);
    let version = stdout_json(&run(&["project", "version", &left_text, "--json"]));
    assert_eq!(version["data"]["schema"]["schema_int"], 360000);
    assert_eq!(version["data"]["support"]["write_guard"], "block");
    let describe = stdout_json(&run(&["project", "describe", "--json"]));
    assert_eq!(describe["data"]["schema"], "jianying-command-catalog/v1");

    let equal = stdout_json(&run(&[
        "project",
        "diff",
        &left_text,
        &right_text,
        "--json",
    ]));
    assert_eq!(equal["data"]["changed"], false);
    let mut changed = base;
    changed["tracks"][0]["segments"][0]["target_timerange"]["start"] = serde_json::json!(500_000);
    std::fs::write(
        right.join("draft_content.json"),
        serde_json::to_vec_pretty(&changed).unwrap(),
    )
    .unwrap();
    let changed = stdout_json(&run(&[
        "project",
        "diff",
        &left_text,
        &right_text,
        "--json",
    ]));
    assert_eq!(changed["data"]["changed"], true);
    assert!(changed["data"]["segments"]["changed"]
        .as_array()
        .unwrap()
        .iter()
        .any(|change| change["id"] == "s1" && change["fields"] == serde_json::json!(["start"])));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_init_quickstart_migrate_and_concat_are_black_box_operational() {
    let root = std::env::temp_dir().join(format!("jianying-project-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let empty = root.join("empty");
    let empty_text = empty.to_string_lossy().into_owned();
    let init = run(&[
        "project",
        "init",
        "empty",
        "--out",
        &empty_text,
        "--width",
        "1080",
        "--height",
        "1920",
        "--json",
    ]);
    assert_eq!(
        init.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert_eq!(stdout_json(&init)["data"]["canvas"]["width"], 1080);
    assert!(empty.join("draft_content.json").is_file());

    let srt_a = root.join("a.srt");
    let srt_b = root.join("b.srt");
    std::fs::write(&srt_a, "1\n00:00:00,000 --> 00:00:01,000\nA\n").unwrap();
    std::fs::write(&srt_b, "1\n00:00:00,000 --> 00:00:02,000\nB\n").unwrap();
    let draft_a = root.join("a");
    let draft_b = root.join("b");
    for (name, out, srt) in [("a", &draft_a, &srt_a), ("b", &draft_b, &srt_b)] {
        let output = run(&[
            "project",
            "quickstart",
            name,
            "--out",
            &out.to_string_lossy(),
            "--srt",
            &srt.to_string_lossy(),
            "--json",
        ]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(stdout_json(&output)["data"]["added"]["captions"], 1);
    }

    let mut migratable: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(draft_a.join("draft_content.json")).unwrap())
            .unwrap();
    migratable["materials"]["masks"] = serde_json::json!([{"id":"mask-legacy"}]);
    let encoded = serde_json::to_string_pretty(&migratable).unwrap();
    std::fs::write(draft_a.join("draft_content.json"), &encoded).unwrap();
    std::fs::write(draft_a.join("draft_info.json"), &encoded).unwrap();
    let migrated = run(&[
        "project",
        "migrate",
        &draft_a.to_string_lossy(),
        "--from",
        "5.9",
        "--to",
        "9.6",
        "--json",
    ]);
    assert_eq!(
        migrated.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&migrated.stderr)
    );
    let migrated_timeline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(draft_a.join("draft_content.json")).unwrap())
            .unwrap();
    assert!(migrated_timeline["materials"]["masks"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        migrated_timeline["materials"]["common_masks"][0]["id"],
        "mask-legacy"
    );
    let transaction_root = root.join(".jianying-transactions");
    let transaction = std::fs::read_dir(&transaction_root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(transaction.join("snapshot/draft_content.json").is_file());
    let audit = std::fs::read_to_string(transaction.join("audit.json")).unwrap();
    assert!(audit.contains("atomic-commit"));
    assert!(audit.contains("jianying store restore-snapshot"));
    let snapshot = transaction.join("snapshot");
    let restored = run(&[
        "store",
        "restore-snapshot",
        "--snapshot",
        &snapshot.to_string_lossy(),
        "--target",
        &draft_a.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        restored.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&restored.stdout),
        String::from_utf8_lossy(&restored.stderr)
    );
    let restored_timeline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(draft_a.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(
        restored_timeline["materials"]["masks"][0]["id"],
        "mask-legacy"
    );
    let restored_verify = run(&["project", "verify", &draft_a.to_string_lossy(), "--json"]);
    assert_eq!(
        restored_verify.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&restored_verify.stdout),
        String::from_utf8_lossy(&restored_verify.stderr)
    );
    assert_eq!(stdout_json(&restored_verify)["data"]["ok"], true);

    let combined = root.join("combined");
    let concat = run(&[
        "project",
        "concat",
        &draft_a.to_string_lossy(),
        &draft_b.to_string_lossy(),
        "--out",
        &combined.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        concat.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&concat.stderr)
    );
    assert_eq!(stdout_json(&concat)["data"]["duration_us"], 3_000_000);
    let combined_timeline: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(combined.join("draft_content.json")).unwrap(),
    )
    .unwrap();
    let starts: Vec<i64> = combined_timeline["tracks"][0]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| segment["target_timerange"]["start"].as_i64().unwrap())
        .collect();
    assert_eq!(starts, vec![0, 1_000_000]);
    let _ = std::fs::remove_dir_all(root);
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

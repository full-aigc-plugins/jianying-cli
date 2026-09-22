use serde_json::Value;
use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .expect("run jianying")
}

#[test]
fn runtime_help_exposes_probe_and_owned_process_controls() {
    let output = run(&["runtime", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    for operation in [
        "discover",
        "acceptance",
        "controls",
        "entitlement",
        "status",
        "probe",
        "start",
        "stop",
        "export-capabilities",
    ] {
        assert!(help.contains(operation), "missing {operation} in {help}");
    }
}

#[test]
fn semantic_control_catalog_prefers_direct_draft_routes_and_contains_no_coordinates() {
    let output = run(&["--json", "runtime", "controls", "list"]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    let controls = envelope["data"]["controls"].as_array().unwrap();
    assert!(controls.len() >= 25);
    let split = controls
        .iter()
        .find(|control| control["semantic_id"] == "timeline.segment.split")
        .unwrap();
    assert_eq!(split["route"], "draft_protocol");
    assert_eq!(split["status"], "supported");
    assert_eq!(split["capability"], "timeline.edit");
    let undo = controls
        .iter()
        .find(|control| control["semantic_id"] == "timeline.session.undo")
        .unwrap();
    assert_eq!(undo["route"], "runtime_native");
    assert_eq!(undo["status"], "partial");
    let mute = controls
        .iter()
        .find(|control| control["semantic_id"] == "track.audio.mute")
        .unwrap();
    assert_eq!(mute["route"], "draft_protocol");
    assert_eq!(mute["status"], "partial");
    assert_eq!(mute["capability"], "timeline.track_mute");
    assert!(mute["command"]
        .as_str()
        .unwrap()
        .contains("timeline track-mute"));
    let rename = controls
        .iter()
        .find(|control| control["semantic_id"] == "track.rename")
        .unwrap();
    assert_eq!(rename["route"], "draft_protocol");
    assert_eq!(rename["status"], "partial");
    assert_eq!(rename["capability"], "timeline.track_rename");
    assert!(rename["command"]
        .as_str()
        .unwrap()
        .contains("timeline track-rename"));
    assert!(!serde_json::to_string(&envelope)
        .unwrap()
        .contains("screen_coordinate"));

    let direct = run(&[
        "--json",
        "runtime",
        "controls",
        "list",
        "--route",
        "draft_protocol",
        "--status",
        "supported",
    ]);
    assert!(direct.status.success());
    let direct: Value = serde_json::from_slice(&direct.stdout).unwrap();
    assert!(direct["data"]["controls"]
        .as_array()
        .unwrap()
        .iter()
        .all(|control| control["route"] == "draft_protocol" && control["status"] == "supported"));

    let get = run(&[
        "--json",
        "runtime",
        "controls",
        "get",
        "timeline.segment.delete",
    ]);
    assert!(get.status.success());
    let get: Value = serde_json::from_slice(&get.stdout).unwrap();
    assert_eq!(get["data"]["capability"], "timeline.remove");
}

#[test]
fn screenshot_surface_inventory_accounts_for_every_visible_entry_without_coordinates() {
    let output = run(&["--json", "runtime", "controls", "surfaces"]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["coverage"]["expected_surfaces"], 50);
    assert_eq!(envelope["data"]["coverage"]["expandable_parents"], 7);
    assert_eq!(envelope["data"]["coverage"]["enumerated_parents"], 0);
    let surfaces = envelope["data"]["surfaces"].as_array().unwrap();
    assert_eq!(surfaces.len(), 50);
    for (region, expected) in [
        ("timeline_tabs", 4),
        ("edit_toolbar", 12),
        ("view_toolbar", 12),
        ("view_settings_popover", 4),
        ("track_headers", 9),
        ("timeline_canvas", 9),
    ] {
        assert_eq!(
            surfaces
                .iter()
                .filter(|surface| surface["region"] == region)
                .count(),
            expected,
            "region {region}"
        );
    }
    let serialized = serde_json::to_string(&envelope).unwrap();
    assert!(!serialized.contains("coordinates"));
    assert!(!serialized.contains("screen_coordinate"));
    assert!(surfaces.iter().any(|surface| {
        surface["surface_id"] == "settings.track_height"
            && surface["semantic_id"] == "timeline.view.track_height"
    }));
    assert!(surfaces.iter().any(|surface| {
        surface["surface_id"] == "track.video.cover" && surface["semantic_id"] == "track.cover.edit"
    }));

    let unresolved = run(&[
        "--json",
        "runtime",
        "controls",
        "surfaces",
        "--status",
        "unresolved",
    ]);
    assert!(unresolved.status.success());
    let unresolved: Value = serde_json::from_slice(&unresolved.stdout).unwrap();
    assert_eq!(unresolved["data"]["surfaces"].as_array().unwrap().len(), 5);

    let expandable = surfaces
        .iter()
        .filter(|surface| surface.get("expansion").is_some())
        .collect::<Vec<_>>();
    assert_eq!(expandable.len(), 7);
    assert!(expandable.iter().all(|surface| {
        matches!(
            surface["expansion"]["status"].as_str(),
            Some("pending_accessibility" | "partial")
        )
    }));
    let settings = expandable
        .iter()
        .find(|surface| surface["surface_id"] == "view.more")
        .unwrap();
    assert_eq!(settings["expansion"]["status"], "partial");
    assert_eq!(
        settings["expansion"]["child_surface_ids"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}

#[test]
fn semantic_control_catalog_binds_existing_atomic_edit_commands_without_gui_coordinates() {
    let output = run(&[
        "--json",
        "runtime",
        "controls",
        "list",
        "--route",
        "draft_protocol",
        "--status",
        "supported",
    ]);
    assert!(output.status.success());
    let output: Value = serde_json::from_slice(&output.stdout).expect("JSON envelope");
    let controls = output["data"]["controls"]
        .as_array()
        .expect("controls array");
    let expected = [
        ("timeline.segment.split", "timeline.edit", "timeline split"),
        (
            "timeline.segment.trim_left",
            "timeline.edit",
            "timeline trim",
        ),
        ("timeline.segment.move", "timeline.edit", "timeline move"),
        (
            "timeline.segment.delete",
            "timeline.remove",
            "timeline remove",
        ),
        (
            "timeline.segment.duplicate",
            "timeline.duplicate",
            "timeline duplicate",
        ),
        ("timeline.track.add", "timeline.edit", "timeline add-track"),
        (
            "timeline.keyframe.add",
            "timeline.keyframe",
            "timeline keyframe",
        ),
        (
            "timeline.filter.add",
            "timeline.filter",
            "timeline add-filter",
        ),
        (
            "timeline.effect.add",
            "timeline.effect",
            "timeline add-effect",
        ),
        (
            "timeline.transition.set",
            "timeline.transition",
            "timeline transition",
        ),
    ];

    for (semantic_id, capability, command_fragment) in expected {
        let control = controls
            .iter()
            .find(|control| control["semantic_id"] == semantic_id)
            .unwrap_or_else(|| panic!("missing supported direct control {semantic_id}"));
        assert_eq!(control["capability"], capability);
        assert!(control["command"]
            .as_str()
            .expect("command")
            .contains(command_fragment));
        assert!(control["parameters"].is_array());
        assert!(control.get("coordinates").is_none());
        assert!(control.get("screen_coordinate").is_none());
    }
}

#[test]
fn session_controls_require_state_readback_and_remain_partial() {
    let expected = [
        ("timeline.session.undo", "invoke"),
        ("timeline.session.redo", "invoke"),
        ("timeline.audio.record", "invoke"),
        ("timeline.session.main_track_magnet", "set"),
        ("timeline.session.snap", "set"),
        ("timeline.session.linkage", "set"),
        ("timeline.view.fit", "invoke"),
        ("timeline.view.zoom_out", "invoke"),
        ("timeline.view.zoom", "set"),
        ("timeline.view.zoom_in", "invoke"),
    ];

    for (semantic_id, operation) in expected {
        let output = run(&["--json", "runtime", "controls", "get", semantic_id]);
        assert!(output.status.success(), "failed to get {semantic_id}");
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        let control = &envelope["data"];
        assert_eq!(control["status"], "partial");
        assert_eq!(control["state_contract"]["operation"], operation);
        assert_eq!(control["state_contract"]["readback_required"], true);
        assert!(control["state_contract"]["readback"].is_string());
    }
}

#[test]
fn runtime_control_mutations_fail_closed_with_version_bound_structured_errors() {
    let help = run(&["runtime", "controls", "--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    for operation in ["list", "get", "set", "invoke"] {
        assert!(help.contains(operation), "missing {operation} in {help}");
    }

    let mismatch = run(&[
        "--json",
        "runtime",
        "controls",
        "set",
        "timeline.session.snap",
        "--value",
        "true",
        "--bundle-id",
        "com.lemon.lvpro",
        "--version",
        "0.0.0",
        "--build",
        "11.6.0-beta2",
    ]);
    assert_eq!(mismatch.status.code(), Some(1));
    let mismatch: Value = serde_json::from_slice(&mismatch.stdout).unwrap();
    assert_eq!(mismatch["error"]["type"], "control_profile_mismatch");
    assert_eq!(mismatch["error"]["details"]["field"], "version");
    assert_eq!(mismatch["error"]["details"]["expected"], "11.5.13243");

    let unavailable = run(&[
        "--json",
        "runtime",
        "controls",
        "invoke",
        "timeline.session.undo",
        "--bundle-id",
        "com.lemon.lvpro",
        "--version",
        "11.5.13243",
        "--build",
        "11.6.0-beta2",
    ]);
    assert_eq!(unavailable.status.code(), Some(1));
    let unavailable: Value = serde_json::from_slice(&unavailable.stdout).unwrap();
    assert_eq!(unavailable["error"]["type"], "control_unavailable");
    assert_eq!(
        unavailable["error"]["details"]["control"],
        "timeline.session.undo"
    );
    assert!(unavailable["error"]["message"]
        .as_str()
        .unwrap()
        .contains("undo_depth readback is not implemented"));

    let wrong_operation = run(&[
        "--json",
        "runtime",
        "controls",
        "invoke",
        "timeline.view.zoom",
        "--bundle-id",
        "com.lemon.lvpro",
        "--version",
        "11.5.13243",
        "--build",
        "11.6.0-beta2",
    ]);
    assert_eq!(wrong_operation.status.code(), Some(1));
    let wrong_operation: Value = serde_json::from_slice(&wrong_operation.stdout).unwrap();
    assert_eq!(
        wrong_operation["error"]["type"],
        "control_operation_mismatch"
    );
}

#[test]
fn runtime_discover_reports_unverified_installations_and_orphan_draft_roots() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "jianying-runtime-cli-discovery-{}-{nonce}",
        std::process::id()
    ));
    let home = root.join("home");
    let applications = root.join("Applications");
    let bundle = applications.join("剪映专业版.app");
    let executable = bundle.join("Contents/MacOS/JianyingPro");
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, b"synthetic editor identity").unwrap();
    fs::write(
        bundle.join("Contents/Info.plist"),
        br#"<?xml version="1.0"?><plist><dict><key>CFBundleShortVersionString</key><string>9.9.1</string></dict></plist>"#,
    )
    .unwrap();
    let draft_root = home.join("Movies/JianyingPro/User Data/Projects/com.lveditor.draft");
    fs::create_dir_all(&draft_root).unwrap();

    let discovered = Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args([
            "--json",
            "runtime",
            "discover",
            "--platform",
            "macos",
            "--home",
            home.to_str().unwrap(),
            "--search-root",
            applications.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        discovered.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&discovered.stdout),
        String::from_utf8_lossy(&discovered.stderr)
    );
    let envelope: Value = serde_json::from_slice(&discovered.stdout).unwrap();
    assert_eq!(envelope["data"]["state"], "found_unverified");
    assert_eq!(envelope["data"]["automatic_routing"], false);
    assert_eq!(envelope["data"]["installations"][0]["version"], "9.9.1");
    assert_eq!(
        envelope["data"]["installations"][0]["support_status"],
        "unverified"
    );
    assert_eq!(envelope["data"]["draft_roots"][0]["exists"], true);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn render_help_exposes_native_as_a_distinct_operation() {
    let output = run(&["render", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("proxy"));
    assert!(help.contains("native"));
    assert!(help.contains("native-task"));
}

#[test]
fn unsupported_native_export_fails_with_a_structured_capability_error() {
    let output = run(&[
        "--json",
        "render",
        "native",
        "missing-draft",
        "--out",
        "missing.mp4",
    ]);
    assert!(!output.status.success());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["type"], "incompatible_capability");
    assert_eq!(envelope["error"]["details"]["capability"], "render.native");
}

#[test]
fn native_export_plan_emits_the_exact_approval_binding_without_starting_an_editor() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "jianying-native-cli-plan-{}-{nonce}",
        std::process::id()
    ));
    let draft = root.join("draft");
    fs::create_dir_all(&draft).unwrap();
    fs::write(draft.join("draft_content.json"), b"{\"tracks\":[]}").unwrap();
    let executable = root.join("synthetic-editor");
    fs::write(&executable, b"synthetic editor identity").unwrap();
    let profile = jianying_runtime::RuntimeProfile::new(
        "synthetic-native-cli",
        "Synthetic Editor",
        "1.0",
        jianying_runtime::RuntimePlatform::MacOs,
        ["render.native"],
    )
    .unwrap()
    .with_file_identity(jianying_runtime::RuntimeFileIdentity::from_path(&executable).unwrap())
    .with_draft_roots([draft.clone()]);
    let profile_path = root.join("profile.json");
    fs::write(&profile_path, serde_json::to_vec_pretty(&profile).unwrap()).unwrap();
    let output_path = root.join("native.mp4");

    let output = Command::new(env!("CARGO_BIN_EXE_jianying"))
        .current_dir(&root)
        .args([
            "--json",
            "render",
            "native",
            draft.to_str().unwrap(),
            "--out",
            output_path.to_str().unwrap(),
            "--runtime-profile",
            profile_path.to_str().unwrap(),
            "--executable",
            executable.to_str().unwrap(),
            "--task-id",
            "native-cli-task",
            "--plan",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["data"]["state"], "approval_required");
    assert_eq!(
        envelope["data"]["approval_binding"]["command"],
        "render.native"
    );
    assert_eq!(
        envelope["data"]["approval_binding"]["task_id"],
        "native-cli-task"
    );
    assert!(!output_path.exists());

    let binding: jianying_jobs::ApprovalBinding =
        serde_json::from_value(envelope["data"]["approval_binding"].clone()).unwrap();
    let approval_root = root.join("approvals");
    let state_root = root.join("exports");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    jianying_jobs::ApprovalStore::new(&approval_root)
        .save(
            &jianying_jobs::ApprovalRecord::grant(
                "native-cli-approval".to_owned(),
                binding.clone(),
                300,
                now,
            )
            .unwrap(),
        )
        .unwrap();
    let submit_native = |approval_id: &str| {
        Command::new(env!("CARGO_BIN_EXE_jianying"))
            .current_dir(&root)
            .args([
                "--json",
                "render",
                "native",
                draft.to_str().unwrap(),
                "--out",
                output_path.to_str().unwrap(),
                "--runtime-profile",
                profile_path.to_str().unwrap(),
                "--executable",
                executable.to_str().unwrap(),
                "--task-id",
                "native-cli-task",
                "--approval-id",
                approval_id,
                "--approval-root",
                approval_root.to_str().unwrap(),
                "--state-root",
                state_root.to_str().unwrap(),
            ])
            .output()
            .unwrap()
    };
    let submitted = submit_native("native-cli-approval");
    assert!(
        submitted.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&submitted.stdout),
        String::from_utf8_lossy(&submitted.stderr)
    );
    let submitted: Value = serde_json::from_slice(&submitted.stdout).unwrap();
    assert_eq!(submitted["data"]["state"], "queued");
    assert_eq!(submitted["data"]["approval_id"], "native-cli-approval");
    assert!(!output_path.exists());

    let run_native_task = |operation: &[&str]| -> (std::process::ExitStatus, Value) {
        let mut arguments = vec!["--json", "render", "native-task"];
        arguments.extend_from_slice(operation);
        arguments.extend_from_slice(&["--state-root", state_root.to_str().unwrap()]);
        let output = Command::new(env!("CARGO_BIN_EXE_jianying"))
            .current_dir(&root)
            .args(arguments)
            .output()
            .unwrap();
        let envelope = serde_json::from_slice(&output.stdout).unwrap();
        (output.status, envelope)
    };

    let (status, shown) = run_native_task(&["show", "native-cli-task"]);
    assert!(status.success());
    assert_eq!(shown["data"]["state"], "queued");

    let (status, running) = run_native_task(&["start", "native-cli-task"]);
    assert!(status.success());
    assert_eq!(running["data"]["state"], "running");

    let (status, progress) = run_native_task(&["progress", "native-cli-task", "60"]);
    assert!(status.success());
    assert_eq!(progress["data"]["progress_percent"], 60);

    let (status, failed) = run_native_task(&[
        "fail",
        "native-cli-task",
        "--reason",
        "synthetic adapter failure",
    ]);
    assert!(status.success());
    assert_eq!(failed["data"]["state"], "failed");
    assert_eq!(
        failed["data"]["recovery_argv"],
        serde_json::json!([
            "jianying",
            "render",
            "native-task",
            "show",
            "native-cli-task"
        ])
    );

    jianying_jobs::ApprovalStore::new(&approval_root)
        .save(
            &jianying_jobs::ApprovalRecord::grant(
                "native-cli-reapproval".to_owned(),
                binding,
                300,
                now + 1,
            )
            .unwrap(),
        )
        .unwrap();
    let resubmitted = submit_native("native-cli-reapproval");
    assert!(
        resubmitted.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&resubmitted.stdout),
        String::from_utf8_lossy(&resubmitted.stderr)
    );
    let resubmitted: Value = serde_json::from_slice(&resubmitted.stdout).unwrap();
    assert_eq!(resubmitted["data"]["state"], "queued");
    assert_eq!(resubmitted["data"]["approval_id"], "native-cli-reapproval");

    let (status, running) = run_native_task(&["start", "native-cli-task"]);
    assert!(status.success());
    assert_eq!(running["data"]["state"], "running");

    fs::write(&output_path, b"synthetic native adapter output").unwrap();
    let (status, verified) = run_native_task(&["verify", "native-cli-task"]);
    assert!(status.success());
    assert_eq!(verified["data"]["state"], "succeeded");
    assert_eq!(verified["data"]["artifact"]["export_kind"], "native");
    assert_eq!(verified["data"]["artifact"]["byte_length"], 31);

    let (status, artifact) = run_native_task(&["result", "native-cli-task"]);
    assert!(status.success());
    assert_eq!(artifact["data"]["sha256"].as_str().unwrap().len(), 64);

    fs::write(&output_path, b"tampered").unwrap();
    let (status, drifted) = run_native_task(&["result", "native-cli-task"]);
    assert!(!status.success());
    assert_eq!(drifted["error"]["type"], "native_artifact_invalid");

    fs::remove_dir_all(root).unwrap();
}

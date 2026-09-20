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

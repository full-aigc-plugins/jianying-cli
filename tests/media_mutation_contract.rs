use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

type MockServer = (
    String,
    Arc<Mutex<Vec<Vec<u8>>>>,
    std::thread::JoinHandle<()>,
);

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> serde_json::Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["data"].clone()
}

fn run_ok_owned(args: &[String]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["data"].clone()
}

fn ffmpeg(args: &[&str]) {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn add_replace_and_relink_are_atomic_and_keep_the_bundle_valid() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!("jianying-media-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let red = root.join("red.mp4");
    let blue = root.join("blue.mp4");
    let audio = root.join("tone.wav");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=red:s=320x240:d=2",
        "-an",
        "-c:v",
        "libx264",
        &red.to_string_lossy(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=blue:s=640x360:d=1",
        "-an",
        "-c:v",
        "libx264",
        &blue.to_string_lossy(),
    ]);
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=2",
        &audio.to_string_lossy(),
    ]);

    let draft = root.join("draft");
    run_ok(&[
        "project",
        "init",
        "media",
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    let video = run_ok(&[
        "media",
        "add-video",
        &draft.to_string_lossy(),
        &red.to_string_lossy(),
        "0us",
        "--json",
    ]);
    let video_id = video["segment_id"].as_str().unwrap().to_owned();
    let audio_result = run_ok(&[
        "media",
        "add-audio",
        &draft.to_string_lossy(),
        &audio.to_string_lossy(),
        "0us",
        "1s",
        "--volume",
        "0.5",
        "--json",
    ]);
    assert!(Path::new(audio_result["path"].as_str().unwrap()).is_file());

    let replaced = run_ok(&[
        "media",
        "replace",
        &draft.to_string_lossy(),
        &video_id,
        &blue.to_string_lossy(),
        "--retime",
        "--json",
    ]);
    assert_eq!(replaced["new_duration_us"], 1_000_000);
    assert_eq!(replaced["retimed"], true);

    let mut timeline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(draft.join("draft_content.json")).unwrap())
            .unwrap();
    let material = timeline["materials"]["videos"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|item| item["id"] == replaced["material_id"])
        .unwrap();
    material["path"] = serde_json::json!(root.join("missing/blue.mp4"));
    let encoded = serde_json::to_string_pretty(&timeline).unwrap();
    std::fs::write(draft.join("draft_content.json"), &encoded).unwrap();
    std::fs::write(draft.join("draft_info.json"), &encoded).unwrap();
    let relinked = run_ok(&[
        "media",
        "relink",
        &draft.to_string_lossy(),
        "--dir",
        &root.to_string_lossy(),
        "--stage",
        "--json",
    ]);
    assert_eq!(relinked["missing"], 0);
    assert_eq!(relinked["relinked_count"], 1);
    assert_eq!(relinked["staged"], 1);
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        true
    );
    assert!(root.join(".jianying-transactions").is_dir());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn tts_sfx_and_draft_retakes_use_the_rust_runtime_only() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let help = run(&["media", "tts", "--help"]);
    let help_text = String::from_utf8_lossy(&help.stdout);
    assert!(help.status.success());
    for provider in [
        "local-command",
        "system-macos",
        "system-windows",
        "local-http",
        "local-process",
        "edge-tts",
        "xiaomi-mimo",
        "volcengine",
        "aliyun-bailian",
        "baidu",
        "tencent-cloud",
        "minimax",
        "zhipu-glm",
    ] {
        assert!(help_text.contains(provider), "missing provider {provider}");
    }
    let root = std::env::temp_dir().join(format!("jianying-media-tools-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("takes.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:01,000\nthis is the intended sentence\n\n\
         2\n00:00:02,000 --> 00:00:03,000\nthis is the intended sentence\n",
    )
    .unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "media-tools",
        "--out",
        &draft.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);

    let retakes = run_ok(&["media", "retakes", &draft.to_string_lossy(), "--json"]);
    assert_eq!(retakes["retakes"].as_array().unwrap().len(), 1);
    assert_eq!(retakes["source"], "字幕");

    let generator = root.join("fake-tts.sh");
    std::fs::write(
        &generator,
        "#!/bin/sh\nffmpeg -hide_banner -loglevel error -y -f lavfi -i sine=frequency=600:duration=1 \"$1\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    std::fs::set_permissions(&generator, std::fs::Permissions::from_mode(0o755)).unwrap();
    let template = format!("{} {{out}} {{text}}", generator.display());
    let tts = run_ok(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "0us",
        "--text",
        "hello; this is one inert argv token",
        "--tts-cmd",
        &template,
        "--json",
    ]);
    assert_eq!(tts["text_delivery"], "argv");
    assert_eq!(tts["provider"], "local-command");
    assert!(tts["bytes"].as_u64().unwrap() > 0);
    assert!(Path::new(tts["path"].as_str().unwrap()).is_file());

    let process_tts = run_ok(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "1200ms",
        "--text",
        "structured local process",
        "--provider",
        "local-process",
        "--executable",
        &generator.to_string_lossy(),
        "--provider-arg",
        "{out}",
        "--provider-arg",
        "{text}",
        "--json",
    ]);
    assert_eq!(process_tts["provider"], "local-process");
    assert_eq!(process_tts["format"], "wav");
    assert!(process_tts["bytes"].as_u64().unwrap() > 0);

    #[cfg(target_os = "macos")]
    if Command::new("say")
        .args(["-v", "?"])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        let system_tts = run_ok(&[
            "media",
            "tts",
            &draft.to_string_lossy(),
            "2400ms",
            "--text",
            "system voice",
            "--provider",
            "system-macos",
            "--json",
        ]);
        assert_eq!(system_tts["provider"], "system-macos");
        assert_eq!(system_tts["format"], "aiff");
    }

    #[cfg(target_os = "windows")]
    if Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "$PSVersionTable.PSVersion.ToString()",
        ])
        .output()
        .is_ok_and(|output| output.status.success())
    {
        let system_tts = run_ok(&[
            "media",
            "tts",
            &draft.to_string_lossy(),
            "2400ms",
            "--text",
            "system voice",
            "--provider",
            "system-windows",
            "--json",
        ]);
        assert_eq!(system_tts["provider"], "system-windows");
        assert_eq!(system_tts["format"], "wav");
    }

    let before_invalid_provider = std::fs::read(draft.join("draft_content.json")).unwrap();
    let denied = run(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "--text",
        "must not leave this host",
        "--provider",
        "local-http",
        "--endpoint",
        "http://example.com/tts",
        "--json",
    ]);
    assert_eq!(denied.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&denied.stdout).contains("local endpoint host must be loopback")
    );
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        before_invalid_provider
    );

    let sfx = run_ok(&[
        "media",
        "sfx",
        &draft.to_string_lossy(),
        "echo",
        "1s",
        "500ms",
        "--volume",
        "0.4",
        "--json",
    ]);
    assert_eq!(sfx["slug"], "echo");
    assert_eq!(sfx["name"], "Echo");
    assert_eq!(sfx["duration_us"], 500_000);

    let timeline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(draft.join("draft_content.json")).unwrap())
            .unwrap();
    let material = timeline["materials"]["audio_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == sfx["materialId"])
        .unwrap();
    assert_eq!(material["resource_id"], "7021052523762946561");
    assert_eq!(material["type"], "sound_effect");
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cloud_tts_plan_binds_cost_and_content_without_network_or_secret_disclosure() {
    let root = std::env::temp_dir().join(format!("jianying-cloud-tts-plan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let draft = root.join("draft");
    std::fs::create_dir_all(&draft).unwrap();

    let planned = run_ok(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "--provider",
        "minimax",
        "--text",
        "这段原文只能参与哈希，不能进入审批输出",
        "--format",
        "mp3",
        "--model",
        "speech-2.8-hd",
        "--voice",
        "male-qn-qingse",
        "--credential-env",
        "MINIMAX_API_KEY",
        "--task-id",
        "tts-plan-1",
        "--max-cost-microunits",
        "50000",
        "--plan",
        "--json",
    ]);
    assert_eq!(planned["state"], "approval_required");
    assert_eq!(planned["provider"], "minimax");
    assert_eq!(planned["approval_binding"]["command"], "tts.cloud.submit");
    assert_eq!(planned["max_cost_microunits"], 50_000);
    assert_eq!(planned["credential_ref"], "MINIMAX_API_KEY");
    assert_eq!(planned["credential_source"], "environment");
    assert_eq!(planned["idempotency_key"].as_str().unwrap().len(), 64);
    let encoded = serde_json::to_string(&planned).unwrap();
    assert!(!encoded.contains("这段原文只能参与哈希"));
    assert!(!draft.join("draft_content.json").exists());

    let canary = run_ok(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "--provider",
        "minimax",
        "--text",
        "这段原文只能参与哈希，不能进入审批输出",
        "--format",
        "mp3",
        "--model",
        "speech-2.8-hd",
        "--voice",
        "male-qn-qingse",
        "--credential-env",
        "MINIMAX_API_KEY",
        "--task-id",
        "tts-plan-1",
        "--max-cost-microunits",
        "50000",
        "--live-canary",
        "--plan",
        "--json",
    ]);
    assert_eq!(
        canary["approval_binding"]["command"],
        "tts.cloud.live-canary"
    );
    assert_ne!(canary["idempotency_key"], planned["idempotency_key"]);

    let cloud_cases = [
        (
            "xiaomi-mimo",
            vec![
                "--format",
                "wav",
                "--model",
                "mimo-v2.5-tts",
                "--voice",
                "default",
            ],
        ),
        (
            "volcengine",
            vec![
                "--format",
                "wav",
                "--voice",
                "zh_female",
                "--resource-id",
                "seed-tts-2.0",
                "--uid",
                "fixture-user",
            ],
        ),
        (
            "aliyun-bailian",
            vec![
                "--format",
                "wav",
                "--model",
                "qwen3-tts-flash",
                "--voice",
                "Cherry",
                "--aliyun-family",
                "qwen-tts",
            ],
        ),
        (
            "baidu",
            vec![
                "--format",
                "mp3",
                "--voice",
                "0",
                "--cuid",
                "fixture-client",
            ],
        ),
        (
            "tencent-cloud",
            vec!["--format", "wav", "--model", "1", "--voice", "101001"],
        ),
        (
            "zhipu-glm",
            vec![
                "--format", "wav", "--model", "glm-tts", "--voice", "tongtong",
            ],
        ),
    ];
    for (index, (provider, extras)) in cloud_cases.into_iter().enumerate() {
        let mut args = vec![
            "media".to_owned(),
            "tts".to_owned(),
            draft.to_string_lossy().into_owned(),
            "--provider".to_owned(),
            provider.to_owned(),
            "--text".to_owned(),
            "离线计划验证".to_owned(),
            "--credential-env".to_owned(),
            "FIXTURE_CREDENTIAL".to_owned(),
            "--task-id".to_owned(),
            format!("tts-provider-plan-{index}"),
            "--max-cost-microunits".to_owned(),
            "50000".to_owned(),
            "--plan".to_owned(),
            "--json".to_owned(),
        ];
        args.extend(extras.into_iter().map(str::to_owned));
        let provider_plan = run_ok_owned(&args);
        assert_eq!(provider_plan["provider"], provider);
        assert_eq!(
            provider_plan["approval_binding"]["command"],
            "tts.cloud.submit"
        );
    }

    let legacy_volcengine = run(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "--provider",
        "volcengine",
        "--text",
        "拒绝旧协议",
        "--voice",
        "zh_female",
        "--app-id",
        "legacy-app",
        "--cluster",
        "legacy-cluster",
        "--uid",
        "fixture-user",
        "--credential-env",
        "FIXTURE_CREDENTIAL",
        "--task-id",
        "tts-legacy-volcengine",
        "--max-cost-microunits",
        "50000",
        "--plan",
        "--json",
    ]);
    assert_eq!(legacy_volcengine.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&legacy_volcengine.stdout)
        .contains("Volcengine V1 --app-id/--cluster configuration is unsupported"));

    let denied = run(&[
        "media",
        "tts",
        &draft.to_string_lossy(),
        "--provider",
        "minimax",
        "--text",
        "不能绕过 plan 直接计费",
        "--format",
        "mp3",
        "--model",
        "speech-2.8-hd",
        "--voice",
        "male-qn-qingse",
        "--credential-env",
        "MINIMAX_API_KEY",
        "--task-id",
        "tts-plan-2",
        "--max-cost-microunits",
        "50000",
        "--json",
    ]);
    assert_eq!(denied.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&denied.stdout).contains(
        "cloud TTS submission requires --approval-id; run the same command with --plan first"
    ));
    assert!(!draft.join("draft_content.json").exists());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn approved_cloud_tts_submits_once_reuses_artifact_and_quarantines_ambiguous_outcomes() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!("jianying-cloud-tts-run-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let source_audio = root.join("provider.wav");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=0.25",
        &source_audio.to_string_lossy(),
    ]);
    let audio_hex = hex_lower(&std::fs::read(&source_audio).unwrap());
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "init",
        "cloud-tts",
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    let approval_root = root.join("approvals");
    let tts_state_root = root.join("tts-state");
    let env_name = "JIANYING_MINIMAX_OFFLINE_EXECUTOR_TEST_KEY";
    let secret = "offline-minimax-secret";

    let planned = run_ok_owned(&cloud_tts_args(
        &draft,
        "审批后只提交一次",
        "cloud-run-success",
        env_name,
        &["--plan"],
    ));
    grant_plan(&approval_root, "cloud-success-approval", &planned);
    let response_body = format!(
        r#"{{"data":{{"audio":"{audio_hex}"}},"trace_id":"mock-minimax-42","base_resp":{{"status_code":0,"status_msg":"success"}}}}"#
    );
    let (endpoint, requests, server) = serve_once(Some(http_response(
        "200 OK",
        "application/json",
        response_body.as_bytes(),
    )));
    let execution_args = cloud_tts_args(
        &draft,
        "审批后只提交一次",
        "cloud-run-success",
        env_name,
        &[
            "--approval-id",
            "cloud-success-approval",
            "--approval-root",
            approval_root.to_str().unwrap(),
            "--tts-state-root",
            tts_state_root.to_str().unwrap(),
            "--cloud-endpoint-override",
            &endpoint,
        ],
    );
    let first = run_ok_owned_with_env(&execution_args, env_name, secret);
    server.join().unwrap();
    assert_eq!(first["ledger_state"], "succeeded");
    assert_eq!(first["reused"], false);
    assert_eq!(first["external_request_id"], "mock-minimax-42");
    assert_eq!(requests.lock().unwrap().len(), 1);
    let sent = String::from_utf8_lossy(&requests.lock().unwrap()[0]).to_ascii_lowercase();
    assert!(sent.contains("authorization: bearer offline-minimax-secret"));

    let second = run_ok_owned_with_env(&execution_args, env_name, secret);
    assert_eq!(second["ledger_state"], "succeeded");
    assert_eq!(second["reused"], true);
    let record = jianying_jobs::TtsLedgerStore::new(tts_state_root.join("ledger"))
        .load(planned["idempotency_key"].as_str().unwrap())
        .unwrap();
    assert_eq!(record.attempts(), 1);

    let ambiguous_plan = run_ok_owned(&cloud_tts_args(
        &draft,
        "连接中断必须先对账",
        "cloud-run-ambiguous",
        env_name,
        &["--plan"],
    ));
    grant_plan(&approval_root, "cloud-ambiguous-approval", &ambiguous_plan);
    let before = std::fs::read(draft.join("draft_content.json")).unwrap();
    let (endpoint, requests, server) = serve_once(None);
    let ambiguous_args = cloud_tts_args(
        &draft,
        "连接中断必须先对账",
        "cloud-run-ambiguous",
        env_name,
        &[
            "--approval-id",
            "cloud-ambiguous-approval",
            "--approval-root",
            approval_root.to_str().unwrap(),
            "--tts-state-root",
            tts_state_root.to_str().unwrap(),
            "--cloud-endpoint-override",
            &endpoint,
        ],
    );
    let failed = run_owned_with_env(&ambiguous_args, env_name, secret);
    server.join().unwrap();
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        before
    );
    let record = jianying_jobs::TtsLedgerStore::new(tts_state_root.join("ledger"))
        .load(ambiguous_plan["idempotency_key"].as_str().unwrap())
        .unwrap();
    assert_eq!(record.state(), jianying_jobs::TtsLedgerState::Ambiguous);
    assert_eq!(record.attempts(), 1);

    let retried = run_owned_with_env(&ambiguous_args, env_name, secret);
    assert_eq!(retried.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&retried.stdout).contains("requires reconciliation"));
    let record = jianying_jobs::TtsLedgerStore::new(tts_state_root.join("ledger"))
        .load(ambiguous_plan["idempotency_key"].as_str().unwrap())
        .unwrap();
    assert_eq!(record.attempts(), 1);

    let definite_plan = run_ok_owned(&cloud_tts_args(
        &draft,
        "四百响应属于明确拒绝",
        "cloud-run-definite",
        env_name,
        &["--plan"],
    ));
    grant_plan(&approval_root, "cloud-definite-approval", &definite_plan);
    let (endpoint, _, server) = serve_once(Some(http_response(
        "400 Bad Request",
        "application/json",
        br#"{"error":"fixture rejection"}"#,
    )));
    let definite_args = cloud_tts_args(
        &draft,
        "四百响应属于明确拒绝",
        "cloud-run-definite",
        env_name,
        &[
            "--approval-id",
            "cloud-definite-approval",
            "--approval-root",
            approval_root.to_str().unwrap(),
            "--tts-state-root",
            tts_state_root.to_str().unwrap(),
            "--cloud-endpoint-override",
            &endpoint,
        ],
    );
    let failed = run_owned_with_env(&definite_args, env_name, secret);
    server.join().unwrap();
    assert_eq!(failed.status.code(), Some(1));
    let record = jianying_jobs::TtsLedgerStore::new(tts_state_root.join("ledger"))
        .load(definite_plan["idempotency_key"].as_str().unwrap())
        .unwrap();
    assert_eq!(record.state(), jianying_jobs::TtsLedgerState::Failed);
    assert_eq!(record.attempts(), 1);
    let retried = run_owned_with_env(&definite_args, env_name, secret);
    assert_eq!(retried.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&retried.stdout).contains("explicit retry"));
    let _ = std::fs::remove_dir_all(root);
}

fn cloud_tts_args(
    draft: &Path,
    text: &str,
    task_id: &str,
    credential_env: &str,
    extras: &[&str],
) -> Vec<String> {
    let mut args = vec![
        "media".to_owned(),
        "tts".to_owned(),
        draft.to_string_lossy().into_owned(),
        "--provider".to_owned(),
        "minimax".to_owned(),
        "--text".to_owned(),
        text.to_owned(),
        "--format".to_owned(),
        "wav".to_owned(),
        "--model".to_owned(),
        "speech-2.8-hd".to_owned(),
        "--voice".to_owned(),
        "male-qn-qingse".to_owned(),
        "--credential-env".to_owned(),
        credential_env.to_owned(),
        "--task-id".to_owned(),
        task_id.to_owned(),
        "--max-cost-microunits".to_owned(),
        "50000".to_owned(),
        "--json".to_owned(),
    ];
    args.extend(extras.iter().map(|value| (*value).to_owned()));
    args
}

fn grant_plan(approval_root: &Path, approval_id: &str, plan: &serde_json::Value) {
    let binding: jianying_jobs::ApprovalBinding =
        serde_json::from_value(plan["approval_binding"].clone()).unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    jianying_jobs::ApprovalStore::new(approval_root)
        .save(
            &jianying_jobs::ApprovalRecord::grant(approval_id.to_owned(), binding, 300, now)
                .unwrap(),
        )
        .unwrap();
}

fn run_ok_owned_with_env(args: &[String], name: &str, value: &str) -> serde_json::Value {
    let output = run_owned_with_env(args, name, value);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["data"].clone()
}

fn run_owned_with_env(args: &[String], name: &str, value: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(name, value)
        .env("JIANYING_ALLOW_TTS_ENDPOINT_OVERRIDE", "1")
        .output()
        .unwrap()
}

fn serve_once(response: Option<Vec<u8>>) -> MockServer {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/tts", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let handle = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        captured
            .lock()
            .unwrap()
            .push(read_http_request(&mut stream));
        if let Some(response) = response {
            stream.write_all(&response).unwrap();
        }
    });
    (endpoint, requests, handle)
}

fn read_http_request(stream: &mut TcpStream) -> Vec<u8> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") {
            let header_end = header_end + 4;
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    line.strip_prefix("Content-Length: ")
                        .or_else(|| line.strip_prefix("content-length: "))
                })
                .and_then(|value| value.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if request.len() >= header_end + content_length {
                break;
            }
        }
    }
    request
}

fn http_response(status: &str, content_type: &str, body: &[u8]) -> Vec<u8> {
    let mut bytes = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

//! 评估策略合同：权重与阈值来自策略文件，非法策略被拒绝且不改动草稿。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            Path::new(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/empty-processes.txt"
            )),
        )
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

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-policy-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn build_text_draft(root: &Path) -> PathBuf {
    // 只有两条字幕、没有画面：主轨覆盖率 0，字幕覆盖率 1000‰
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"policy",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"text","segments":[
            {"start_us":0,"duration_us":1_000_000,"text":"前半"},
            {"start_us":1000000,"duration_us":1_000_000,"text":"后半"}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    draft
}

#[test]
fn weights_come_from_the_policy_and_the_report_echoes_it() {
    let root = temp_root("weights");
    let draft = build_text_draft(&root);
    let policy = root.join("caption-only.json");
    fs::write(
        &policy,
        serde_json::to_string(&json!({
            "schema":"jianying-eval-policy/v1","name":"caption-only",
            "weights":{"main":0,"continuity":0,"caption":20,"audio":0}
        }))
        .unwrap(),
    )
    .unwrap();

    let default = run_ok(&["eval", &draft.to_string_lossy(), "--json"]);
    let tuned = run_ok(&[
        "eval",
        &draft.to_string_lossy(),
        "--policy",
        &policy.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(default["policy"]["name"], json!("default"));
    assert_eq!(tuned["policy"]["name"], json!("caption-only"));
    assert_ne!(
        default["score"], tuned["score"],
        "权重变化必须影响分数: default={} tuned={}",
        default["score"], tuned["score"]
    );
    // caption-only 策略下只剩字幕维度：覆盖率 1000‰ × 权重 20 ÷ 20 = 100
    assert_eq!(tuned["score"], json!(100));
    assert_eq!(default["score"], json!(50), "默认策略把主轨缺失也算进去");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn invalid_policies_are_rejected_before_any_evaluation() {
    let root = temp_root("invalid");
    let draft = build_text_draft(&root);

    let inverted = root.join("inverted.json");
    fs::write(
        &inverted,
        serde_json::to_string(&json!({
            "schema":"jianying-eval-policy/v1",
            "thresholds":{"silence_ratio_permille":100}
        }))
        .unwrap(),
    )
    .unwrap();
    let output = run(&[
        "eval",
        &draft.to_string_lossy(),
        "--policy",
        &inverted.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout)
        .contains("silence_ratio_permille must be >= mostly_silent_permille"));

    let wrong_schema = root.join("wrong.json");
    fs::write(&wrong_schema, r#"{"schema":"other/v1"}"#).unwrap();
    let output = run(&[
        "eval",
        &draft.to_string_lossy(),
        "--policy",
        &wrong_schema.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("unsupported policy schema"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn a_strict_threshold_turns_a_passing_draft_into_a_failure() {
    if Command::new("ffmpeg").arg("-version").output().is_err()
        || Command::new("ffprobe").arg("-version").output().is_err()
    {
        return;
    }
    let root = temp_root("strict");
    let clip = root.join("clip.mp4");
    // 0.4s 黑帧 + 1.6s 画面 = 20% 黑帧
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x180:d=0.4",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=1.6",
            "-filter_complex",
            "[0:v][1:v]concat=n=2:v=1:a=0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(status.success());
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"strict",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2_000_000,"source":"clip.mp4"}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);

    let default = run_ok(&["eval", &draft.to_string_lossy(), "--deep", "--json"]);
    assert_eq!(default["score"], json!(100), "默认阈值放行 20% 黑帧");

    let strict = root.join("strict.json");
    fs::write(
        &strict,
        serde_json::to_string(&json!({
            "schema":"jianying-eval-policy/v1","name":"strict",
            "thresholds":{"black_ratio_permille":150}
        }))
        .unwrap(),
    )
    .unwrap();
    let tuned = run_ok(&[
        "eval",
        &draft.to_string_lossy(),
        "--deep",
        "--policy",
        &strict.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(tuned["score"], json!(75), "严格阈值 -25");
    assert_eq!(
        tuned["policy"]["thresholds"]["black_ratio_permille"],
        json!(150)
    );
    assert!(tuned["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "video-black"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn mcp_eval_accepts_a_policy_file() {
    let root = temp_root("mcp");
    let draft = build_text_draft(&root);
    let policy = root.join("caption-only.json");
    fs::write(
        &policy,
        serde_json::to_string(&json!({
            "schema":"jianying-eval-policy/v1","name":"caption-only",
            "weights":{"main":0,"continuity":0,"caption":20,"audio":0}
        }))
        .unwrap(),
    )
    .unwrap();

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
               "params":{"protocolVersion":"2024-11-05","capabilities":{},
                          "clientInfo":{"name":"policy","version":"1"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"jianying_eval",
            "arguments":{"draft":draft.to_string_lossy(),"policy":policy.to_string_lossy()}}}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(["mcp", "serve"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        for request in &requests {
            writeln!(stdin, "{request}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    let response = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .find(|message| message["id"] == json!(2))
        .expect("eval 响应");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let envelope: Value = serde_json::from_str(text).unwrap();
    assert_eq!(envelope["ok"], json!(true), "{envelope}");
    assert_eq!(envelope["data"]["policy"]["name"], json!("caption-only"));
    assert_eq!(envelope["data"]["score"], json!(100));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn shot_length_issues_are_structural_and_policy_tunable() {
    if Command::new("ffmpeg").arg("-version").output().is_err()
        || Command::new("ffprobe").arg("-version").output().is_err()
    {
        return;
    }
    let root = temp_root("shots");
    let clip = root.join("long.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=10",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(status.success());
    // 0.2s 碎切 + 9.8s 闷镜
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"shots",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":200_000,"source":"long.mp4","source_start_us":0},
            {"start_us":200_000,"duration_us":9_800_000,"source":"long.mp4","source_start_us":200_000}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);

    let report = run_ok(&["eval", &draft.to_string_lossy(), "--json"]);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"shot-too-short"), "issues={codes:?}");
    assert!(codes.contains(&"shot-too-long"), "issues={codes:?}");
    assert_eq!(report["score"], json!(90), "两个镜头问题各扣 5");
    assert_eq!(report["shot_stats"]["count"], json!(2));
    assert_eq!(report["shot_stats"]["min_us"], json!(200_000));

    let loose = root.join("loose.json");
    fs::write(
        &loose,
        serde_json::to_string(&json!({
            "schema":"jianying-eval-policy/v1","name":"loose",
            "thresholds":{"min_shot_us":100_000,"max_shot_us":20_000_000}
        }))
        .unwrap(),
    )
    .unwrap();
    let tuned = run_ok(&[
        "eval",
        &draft.to_string_lossy(),
        "--policy",
        &loose.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(tuned["score"], json!(100));
    assert!(tuned["issues"].as_array().unwrap().is_empty());
    let _ = fs::remove_dir_all(root);
}

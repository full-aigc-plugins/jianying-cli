//! 评估与修正循环合同：确定性评分、只读评估、安全修复。

use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(any(unix, windows))]
use std::sync::OnceLock;

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command.args(args);
    #[cfg(unix)]
    command.env("PATH", isolated_process_path());
    #[cfg(windows)]
    command.env("JIANYING_CLI_PROCESS_LIST", empty_process_listing());
    command.output().unwrap()
}

#[cfg(unix)]
fn isolated_process_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!("jianying-eval-bin-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let ps = root.join("ps");
        fs::write(&ps, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&ps, fs::Permissions::from_mode(0o700)).unwrap();
        root.to_string_lossy().into_owned()
    })
}

#[cfg(windows)]
fn empty_process_listing() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        let path = std::env::temp_dir().join(format!(
            "jianying-eval-processes-{}.txt",
            std::process::id()
        ));
        fs::write(&path, "Image Name,PID\r\n").unwrap();
        path.to_string_lossy().into_owned()
    })
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

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 4_000_000,
        width: 320,
        height: 180,
        has_video: true,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-eval-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();
    root
}

/// 建一份两段主轨连续的草稿。
fn build_contiguous(root: &Path) -> PathBuf {
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"eval",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2000000,"source":"clip.mp4"},
            {"start_us":2000000,"duration_us":2000000,"source":"clip.mp4"}
        ]}]
    }))
    .unwrap();
    let out = root.join("draft");
    draft::build(&plan, root, &out, None, &probe_stub).unwrap();
    out
}

#[test]
fn clean_draft_scores_full_and_eval_does_not_mutate_it() {
    let root = temp_root("clean");
    let dir = build_contiguous(&root);
    let before = fs::read_to_string(dir.join("draft_content.json")).unwrap();
    let report = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    assert_eq!(report["score"], json!(100));
    assert_eq!(report["issues"].as_array().unwrap().len(), 0);
    assert_eq!(report["main_track"]["coverage_permille"], json!(1000));
    assert_eq!(report["main_track"]["gap_us"], json!(0));
    let after = fs::read_to_string(dir.join("draft_content.json")).unwrap();
    assert_eq!(before, after, "eval 必须只读");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn gap_is_detected_and_the_loop_repairs_it() {
    let root = temp_root("gap");
    let dir = build_contiguous(&root);
    let segments = run_ok(&[
        "timeline",
        "segments",
        &dir.to_string_lossy(),
        "--track",
        "video",
        "--json",
    ]);
    let second = segments[1]["id"].as_str().unwrap().to_owned();
    run_ok(&[
        "timeline",
        "move",
        &dir.to_string_lossy(),
        &second,
        "2500ms",
        "--json",
    ]);

    let report = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    assert!(report["score"].as_i64().unwrap() < 100);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"main-track-gap"), "issues={codes:?}");
    assert!(report["main_track"]["gap_us"].as_i64().unwrap() > 0);

    let outcome = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(outcome["stopped"], json!("clean"));
    assert_eq!(outcome["final"]["score"], json!(100));
    assert_eq!(outcome["final"]["main_track"]["gap_us"], json!(0));
    let rounds = outcome["rounds"].as_array().unwrap();
    assert_eq!(rounds.len(), 1);
    assert_eq!(rounds[0]["repair"], json!("close-main-track-gaps"));
    assert!(rounds[0]["score_before"].as_i64().unwrap() < 100);
    assert_eq!(rounds[0]["score_after"], json!(100));
    assert_eq!(
        run_ok(&["project", "verify", &dir.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn empty_tracks_are_dropped_by_the_loop_and_loop_stops_when_nothing_is_safe() {
    let root = temp_root("empty-track");
    let dir = build_contiguous(&root);
    run_ok(&[
        "timeline",
        "add-track",
        &dir.to_string_lossy(),
        "effect",
        "empty",
        "--json",
    ]);

    let report = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"empty-track"), "issues={codes:?}");

    let outcome = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(outcome["rounds"][0]["repair"], json!("drop-empty-tracks"));
    assert_eq!(outcome["final"]["score"], json!(100));

    // 再跑一次：已无问题，应立即停且不做任何修复
    let again = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(again["rounds"].as_array().unwrap().len(), 0);
    assert_eq!(again["stopped"], json!("clean"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn caption_and_audio_coverage_participate_in_the_score() {
    let root = temp_root("coverage");
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"coverage",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[
            {"type":"video","segments":[
                {"start_us":0,"duration_us":2000000,"source":"clip.mp4"}
            ]},
            {"type":"text","segments":[
                {"start_us":0,"duration_us":1000000,"text":"半程字幕"}
            ]}
        ]
    }))
    .unwrap();
    let dir = root.join("draft");
    draft::build(&plan, &root, &dir, None, &probe_stub).unwrap();

    let report = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    // 字幕只覆盖一半时长：出现该维度，且拉低总分
    assert_eq!(report["caption_coverage_permille"], json!(500));
    assert!(report["score"].as_i64().unwrap() < 100, "report={report}");
    // 没有音轨时不参与评分，不应因此扣分
    assert_eq!(report["audio_coverage_permille"], Value::Null);
    let _ = fs::remove_dir_all(root);
}

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
}

fn write_clip(path: &Path) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x445566:s=320x180:d=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg 造素材失败");
}

#[test]
fn deep_eval_flags_missing_media_and_oversized_source_windows() {
    if !media_ready() {
        return;
    }
    let root = temp_root("deep");
    write_clip(&root.join("clip.mp4"));

    let build = |duration_us: i64, out: &str| -> PathBuf {
        let value = json!({
            "schema":"jianying-cli-plan/v1","name":out,
            "canvas":{"width":320,"height":180,"fps":30},
            "tracks":[{"type":"video","segments":[
                {"start_us":0,"duration_us":duration_us,"source":"clip.mp4"}
            ]}]
        });
        let plan_path = root.join(format!("{out}.json"));
        fs::write(&plan_path, serde_json::to_string(&value).unwrap()).unwrap();
        let draft_dir = root.join(out);
        run_ok(&[
            "project",
            "build",
            &plan_path.to_string_lossy(),
            "--out",
            &draft_dir.to_string_lossy(),
            "--json",
        ]);
        draft_dir
    };

    // 干净：素材存在且源窗口不越界
    let clean = build(2_000_000, "clean");
    let report = run_ok(&["eval", &clean.to_string_lossy(), "--deep", "--json"]);
    assert_eq!(report["deep"], json!(true));
    assert_eq!(report["score"], json!(100));
    assert_eq!(report["media_checked"], json!(1));
    assert_eq!(report["issues"].as_array().unwrap().len(), 0);

    // 源窗口超出素材真实时长（2s 素材要 5s）
    let over = build(5_000_000, "over");
    let report = run_ok(&["eval", &over.to_string_lossy(), "--deep", "--json"]);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"source-exceeds-media"), "issues={codes:?}");
    assert_eq!(report["score"], json!(85), "100 - 15");

    // 素材文件缺失（草稿自带的 assets 被删）
    fs::remove_dir_all(clean.join("assets")).unwrap();
    let report = run_ok(&["eval", &clean.to_string_lossy(), "--deep", "--json"]);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"media-missing"), "issues={codes:?}");
    assert_eq!(report["score"], json!(70), "100 - 30");
    assert_eq!(report["penalties"].as_array().unwrap().len(), 1);
    let _ = fs::remove_dir_all(root);
}

#[test]
fn media_checks_are_opt_in_and_absent_from_shallow_eval() {
    let root = temp_root("shallow");
    let dir = build_contiguous(&root);
    // 浅评估即便素材全丢也不该碰文件系统
    fs::remove_dir_all(dir.join("assets")).unwrap();
    let report = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    assert!(report["deep"].is_null(), "浅评估不应设置 deep");
    assert!(report["media_checked"].is_null());
    assert_eq!(report["score"], json!(100));
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .all(|issue| issue["code"] != "media-missing"));
    let _ = fs::remove_dir_all(root);
}

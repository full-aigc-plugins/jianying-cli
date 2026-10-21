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

#[test]
fn loop_repairs_an_oversized_source_window_by_clamping_it() {
    if !media_ready() {
        return;
    }
    let root = temp_root("clamp");
    write_clip(&root.join("clip.mp4"));
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"clamp",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":5000000,"source":"clip.mp4"}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let dir = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &dir.to_string_lossy(),
        "--json",
    ]);

    let before = run_ok(&["eval", &dir.to_string_lossy(), "--deep", "--json"]);
    assert_eq!(before["score"], json!(85));

    let outcome = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--deep",
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(outcome["rounds"][0]["repair"], json!("clamp-source-window"));
    assert_eq!(outcome["final"]["score"], json!(100));
    assert_eq!(
        outcome["final"]["duration_us"],
        json!(2000000),
        "应收窄到素材实际时长"
    );
    let codes: Vec<&str> = outcome["final"]["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(!codes.contains(&"source-exceeds-media"), "issues={codes:?}");
    assert_eq!(
        run_ok(&["project", "verify", &dir.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

fn ffmpeg_out(args: &[&str]) {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg 失败: {args:?}");
}

#[test]
fn deep_eval_flags_black_video_and_fully_silent_audio() {
    if !media_ready() {
        return;
    }
    let root = temp_root("quality");
    let black = root.join("black.mp4");
    let good = root.join("good.mp4");
    let silent = root.join("silent.m4a");
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=320x180:d=2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &black.to_string_lossy(),
    ]);
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &good.to_string_lossy(),
    ]);
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "anullsrc=r=44100:cl=mono",
        "-t",
        "2",
        "-c:a",
        "aac",
        "-y",
        &silent.to_string_lossy(),
    ]);

    let build = |name: &str, plan: Value| -> PathBuf {
        let plan_path = root.join(format!("{name}.json"));
        fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
        let draft_dir = root.join(format!("draft-{name}"));
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

    let black_draft = build(
        "black",
        json!({
            "schema":"jianying-cli-plan/v1","name":"black",
            "canvas":{"width":320,"height":180,"fps":30},
            "tracks":[{"type":"video","segments":[
                {"start_us":0,"duration_us":2000000,"source":"black.mp4"}
            ]}]
        }),
    );
    let report = run_ok(&["eval", &black_draft.to_string_lossy(), "--deep", "--json"]);
    assert_eq!(report["score_before_media"], json!(100));
    assert_eq!(report["score"], json!(75), "黑帧 -25");
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "video-black"));

    let silent_draft = build(
        "silent",
        json!({
            "schema":"jianying-cli-plan/v1","name":"silent",
            "canvas":{"width":320,"height":180,"fps":30},
            "tracks":[
                {"type":"video","segments":[
                    {"start_us":0,"duration_us":2000000,"source":"good.mp4"}
                ]},
                {"type":"audio","segments":[
                    {"start_us":0,"duration_us":2000000,"source":"silent.m4a"}
                ]}
            ]
        }),
    );
    let report = run_ok(&["eval", &silent_draft.to_string_lossy(), "--deep", "--json"]);
    assert_eq!(report["score_before_media"], json!(100));
    // 全静音同时命中静音(-20)与响度(-10)：100 - 30
    assert_eq!(report["score"], json!(70), "全静音 -30");
    assert!(report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "audio-silent"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn deep_eval_flags_unusable_loudness() {
    if !media_ready() {
        return;
    }
    let root = temp_root("loudness");
    let video = root.join("good.mp4");
    let quiet = root.join("quiet.m4a");
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &video.to_string_lossy(),
    ]);
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=2",
        "-af",
        "volume=0.008",
        "-c:a",
        "aac",
        "-y",
        &quiet.to_string_lossy(),
    ]);
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"loudness",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[
            {"type":"video","segments":[{"start_us":0,"duration_us":2000000,"source":"good.mp4"}]},
            {"type":"audio","segments":[{"start_us":0,"duration_us":2000000,"source":"quiet.m4a"}]}
        ]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let dir = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &dir.to_string_lossy(),
        "--json",
    ]);

    let report = run_ok(&["eval", &dir.to_string_lossy(), "--deep", "--json"]);
    let codes: Vec<&str> = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect();
    assert!(codes.contains(&"audio-loudness"), "issues={codes:?}");
    // 过轻的音频同时触发静音与响度两个问题：100 - 20 - 10
    assert_eq!(report["score"], json!(70), "report={report}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn loop_trims_a_black_lead_in_and_keeps_the_shot_in_place() {
    if !media_ready() {
        return;
    }
    let root = temp_root("black-lead");
    let clip = root.join("black8.mp4");
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=320x180:d=8",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=2",
        "-filter_complex",
        "[0:v][1:v]concat=n=2:v=1:a=0",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &clip.to_string_lossy(),
    ]);
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"black-lead",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":10000000,"source":"black8.mp4"}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let dir = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &dir.to_string_lossy(),
        "--json",
    ]);

    let before = run_ok(&["eval", &dir.to_string_lossy(), "--deep", "--json"]);
    // 10s 单镜头同时触发黑帧(-25)与闷镜(-5)
    assert_eq!(before["score"], json!(70), "80% 黑帧 -25，10s 长镜 -5");
    assert!(before["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|issue| issue["code"] == "shot-too-long"));

    let outcome = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--deep",
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(
        outcome["rounds"].as_array().unwrap().len(),
        1,
        "一轮即可修好"
    );
    assert_eq!(outcome["rounds"][0]["repair"], json!("trim-quality-edges"));
    assert_eq!(outcome["final"]["score"], json!(100));
    assert_eq!(outcome["final"]["duration_us"], json!(2000000));
    assert_eq!(
        outcome["final"]["main_track"]["gap_us"],
        json!(0),
        "不应制造空洞"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn loop_splits_a_long_shot_without_changing_the_footage() {
    if !media_ready() {
        return;
    }
    let root = temp_root("split-long");
    let clip = root.join("long.mp4");
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=10",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &clip.to_string_lossy(),
    ]);
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"split-long",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":10_000_000,"source":"long.mp4"}
        ]}]
    });
    let plan_path = root.join("plan.json");
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let dir = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &dir.to_string_lossy(),
        "--json",
    ]);

    let before = run_ok(&["eval", &dir.to_string_lossy(), "--json"]);
    assert_eq!(before["score"], json!(95), "单个 10s 镜头命中闷镜 -5");
    assert_eq!(before["shot_stats"]["count"], json!(1));

    let outcome = run_ok(&[
        "loop",
        &dir.to_string_lossy(),
        "--max-rounds",
        "3",
        "--min-score",
        "100",
        "--json",
    ]);
    assert_eq!(outcome["rounds"].as_array().unwrap().len(), 1);
    assert_eq!(outcome["rounds"][0]["repair"], json!("split-long-shots"));
    assert_eq!(outcome["final"]["score"], json!(100));
    assert_eq!(
        outcome["final"]["duration_us"],
        json!(10_000_000),
        "总时长不得变化"
    );
    assert_eq!(outcome["final"]["shot_stats"]["count"], json!(2));

    // 切分只改切点：源窗口必须连续，画面不增不减
    let timeline: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("draft_content.json")).unwrap()).unwrap();
    let segments = timeline["tracks"][0]["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2);
    assert_eq!(segments[0]["source_timerange"]["start"], json!(0));
    assert_eq!(
        segments[0]["source_timerange"]["duration"],
        json!(5_000_000)
    );
    assert_eq!(segments[1]["source_timerange"]["start"], json!(5_000_000));
    assert_eq!(segments[1]["target_timerange"]["start"], json!(5_000_000));
    assert_eq!(
        run_ok(&["project", "verify", &dir.to_string_lossy(), "--json"])["ok"],
        json!(true)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn the_loop_is_reachable_over_mcp() {
    if !media_ready() {
        return;
    }
    let root = temp_root("mcp-loop");
    let clip = root.join("long.mp4");
    ffmpeg_out(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=10",
        "-c:v",
        "libx264",
        "-pix_fmt",
        "yuv420p",
        "-y",
        &clip.to_string_lossy(),
    ]);
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"mcp-loop",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":10_000_000,"source":"long.mp4"}
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

    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
               "params":{"protocolVersion":"2024-11-05","capabilities":{},
                          "clientInfo":{"name":"loop","version":"1"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{
            "name":"jianying_loop",
            "arguments":{"draft":draft.to_string_lossy(),"max_rounds":2}}}),
    ];
    let mut child = Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(["mcp", "serve"])
        // 让进程探测走固定夹具：否则本机开着剪映时这条测试会因为“编辑器在运行”而失败，
        // 那是产品正确的失败关闭行为，不该被当作回归。
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/empty-processes.txt"
            ),
        )
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
    let responses: Vec<Value> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.trim_start().starts_with('{'))
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect();

    let tools: Vec<&str> = responses
        .iter()
        .find(|message| message["id"] == json!(2))
        .unwrap()["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect();
    assert!(tools.contains(&"jianying_loop"), "tools={tools:?}");

    let response = responses
        .iter()
        .find(|message| message["id"] == json!(3))
        .expect("loop 响应");
    let data: Value =
        serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    let data = data["data"].clone();
    assert_eq!(data["stopped"], json!("clean"));
    assert_eq!(data["rounds"][0]["repair"], json!("split-long-shots"));
    assert_eq!(data["final"]["score"], json!(100));
    let _ = fs::remove_dir_all(root);
}

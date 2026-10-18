//! 简报生成合同：确定性装配、平台画布、字幕接入、缺口如实上报。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
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

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-brief-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn clip(path: &Path, duration_s: u32) {
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc=s=320x180:d={duration_s}"),
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

fn brief(root: &Path, value: Value) -> PathBuf {
    let path = root.join("brief.json");
    fs::write(&path, serde_json::to_string_pretty(&value).unwrap()).unwrap();
    path
}

#[test]
fn generate_assembles_shots_up_to_the_target_and_builds() {
    if !media_ready() {
        return;
    }
    let root = temp_root("assemble");
    clip(&root.join("a.mp4"), 2);
    clip(&root.join("b.mp4"), 2);
    let brief_path = brief(
        &root,
        json!({
            "schema":"jianying-brief/v1","name":"assembled","platform":"douyin",
            "target_duration_us":3_000_000,"media":["a.mp4","b.mp4"]
        }),
    );
    let out = root.join("plan.json");
    let report = run_ok(&[
        "generate",
        &brief_path.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["duration_us"], json!(3_000_000));
    assert_eq!(report["shots"], json!(2));
    assert_eq!(report["canvas"]["width"], json!(1080));
    assert_eq!(report["canvas"]["height"], json!(1920));
    assert_eq!(report["warnings"].as_array().unwrap().len(), 0);

    let plan: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let starts: Vec<i64> = plan["tracks"][0]["segments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|segment| segment["start_us"].as_i64().unwrap())
        .collect();
    assert_eq!(starts, vec![0, 2_000_000], "第二段必须紧接第一段");

    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &out.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        json!(true)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generate_reports_short_media_instead_of_faking_length() {
    if !media_ready() {
        return;
    }
    let root = temp_root("short");
    clip(&root.join("a.mp4"), 1);
    let brief_path = brief(
        &root,
        json!({
            "schema":"jianying-brief/v1","name":"short","platform":"bilibili",
            "target_duration_us":5_000_000,"media":["a.mp4"]
        }),
    );
    let report = run_ok(&[
        "generate",
        &brief_path.to_string_lossy(),
        "--out",
        &root.join("plan.json").to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["duration_us"], json!(1_000_000), "只能用满素材");
    let warnings: Vec<&str> = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|warning| warning.as_str().unwrap())
        .collect();
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("short-of-target")),
        "warnings={warnings:?}"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn generate_adds_bom_tolerant_captions_and_refuses_silent_overwrite() {
    if !media_ready() {
        return;
    }
    let root = temp_root("captions");
    clip(&root.join("a.mp4"), 2);
    fs::write(
        root.join("subs.srt"),
        "\u{feff}1\n00:00:00,000 --> 00:00:01,000\n第一句\n",
    )
    .unwrap();
    let brief_path = brief(
        &root,
        json!({
            "schema":"jianying-brief/v1","name":"captioned","platform":"douyin",
            "target_duration_us":2_000_000,"media":["a.mp4"],"captions":"subs.srt"
        }),
    );
    let out = root.join("plan.json");
    run_ok(&[
        "generate",
        &brief_path.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let plan: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let text_track = plan["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "text")
        .expect("字幕轨缺失");
    let text = text_track["segments"][0]["text"].as_str().unwrap();
    assert_eq!(text, "第一句", "BOM 与序号都不得混进正文");
    assert_eq!(text_track["segments"][0]["duration_us"], json!(1_000_000));

    let refused = run(&[
        "generate",
        &brief_path.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(refused.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&refused.stdout).contains("already exists"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn max_shot_us_splits_one_clip_into_consecutive_shots() {
    if !media_ready() {
        return;
    }
    let root = temp_root("shots");
    clip(&root.join("long.mp4"), 6);
    let brief_path = brief(
        &root,
        json!({
            "schema":"jianying-brief/v1","name":"shots","platform":"bilibili",
            "target_duration_us":5_000_000,"max_shot_us":1_500_000,"media":["long.mp4"]
        }),
    );
    let out = root.join("plan.json");
    let report = run_ok(&[
        "generate",
        &brief_path.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(report["duration_us"], json!(5_000_000));
    assert_eq!(report["shots"], json!(4), "5s / 1.5s 上限应切成 4 段");

    let plan: Value = serde_json::from_str(&fs::read_to_string(&out).unwrap()).unwrap();
    let segments = plan["tracks"][0]["segments"].as_array().unwrap();
    let durations: Vec<i64> = segments
        .iter()
        .map(|segment| segment["duration_us"].as_i64().unwrap())
        .collect();
    assert_eq!(durations, vec![1_500_000, 1_500_000, 1_500_000, 500_000]);
    let offsets: Vec<i64> = segments
        .iter()
        .map(|segment| segment["source_start_us"].as_i64().unwrap_or(0))
        .collect();
    assert_eq!(
        offsets,
        vec![0, 1_500_000, 3_000_000, 4_500_000],
        "源窗口必须连续前移"
    );

    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &out.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        json!(true)
    );
    let _ = fs::remove_dir_all(root);
}

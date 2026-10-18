//! 停顿剪切合同：按静音段剪掉停顿、保留气口，并让其它轨道跟着前移。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            std::env::temp_dir().join("no-editor.txt"),
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

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-cutsil-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn ffmpeg(args: &[&str], out: &Path) {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error"])
        .args(args)
        .args(["-c:a", "pcm_s16le", "-y"])
        .arg(out)
        .status()
        .unwrap();
    assert!(status.success());
}

/// 2s 有声 + 1.5s 静音 + 2s 有声
fn talk_track(root: &Path) {
    for (name, spec) in [
        ("a.wav", "sine=frequency=440:duration=2"),
        ("b.wav", "sine=frequency=660:duration=2"),
    ] {
        ffmpeg(
            &["-f", "lavfi", "-i", spec, "-af", "volume=0.8"],
            &root.join(name),
        );
    }
    ffmpeg(
        &["-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono", "-t", "1.5"],
        &root.join("s.wav"),
    );
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-v", "error"])
        .args(["-i", &root.join("a.wav").to_string_lossy()])
        .args(["-i", &root.join("s.wav").to_string_lossy()])
        .args(["-i", &root.join("b.wav").to_string_lossy()])
        .args([
            "-filter_complex",
            "[0:a][1:a][2:a]concat=n=3:v=0:a=1",
            "-c:a",
            "pcm_s16le",
            "-y",
        ])
        .arg(root.join("talk.wav"))
        .status()
        .unwrap();
    assert!(status.success());
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"talk",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[
            {"type":"audio","segments":[
                {"start_us":0,"duration_us":5_500_000,"source":"talk.wav"}
            ]},
            {"type":"text","segments":[
                {"start_us":4_000_000,"duration_us":1_000_000,"text":"尾段字幕"}
            ]}
        ]
    });
    fs::write(
        root.join("plan.json"),
        serde_json::to_string(&plan).unwrap(),
    )
    .unwrap();
}

#[test]
fn cut_silences_removes_the_pause_and_ripples_other_tracks() {
    if !media_ready() {
        return;
    }
    let root = temp_root("ok");
    talk_track(&root);
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "build",
        &root.join("plan.json").to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);

    let report = run_ok(&[
        "timeline",
        "cut-silences",
        &draft.to_string_lossy(),
        "--track",
        "audio",
        "--min-silence",
        "600ms",
        "--keep-pad",
        "100ms",
        "--json",
    ]);
    // 1.5s 静音 - 两侧各 100ms 气口 = 1.3s
    let removed = report["removed_us"].as_i64().unwrap();
    assert!(
        (1_200_000..=1_400_000).contains(&removed),
        "应剪掉约 1.3s，实际 {removed}"
    );
    assert_eq!(report["kept_segments"], json!(2));

    let timeline: Value =
        serde_json::from_str(&fs::read_to_string(draft.join("draft_content.json")).unwrap())
            .unwrap();
    let audio = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "audio")
        .unwrap();
    let segments = audio["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2, "停顿应被切成两段");
    let starts: Vec<i64> = segments
        .iter()
        .map(|segment| segment["target_timerange"]["start"].as_i64().unwrap())
        .collect();
    assert_eq!(starts, vec![0, 2_099_955], "保留段应紧邻拼接");

    // 尾段字幕原本在 4s，剪掉 1.3s 后应前移
    let text = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "text")
        .unwrap();
    let text_start = text["segments"][0]["target_timerange"]["start"]
        .as_i64()
        .unwrap();
    assert!(
        text_start < 4_000_000,
        "其它轨道必须跟着前移，实际 {text_start}"
    );

    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        json!(true)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn cut_silences_rejects_invalid_arguments() {
    let root = temp_root("invalid");
    fs::create_dir_all(&root).unwrap();
    let output = run(&[
        "timeline",
        "cut-silences",
        &root.to_string_lossy(),
        "--min-silence",
        "0ms",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("min-silence must be positive"));
    let _ = fs::remove_dir_all(root);
}

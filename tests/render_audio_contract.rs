//! 代理渲染的音频合同：画面自带的音轨要进混音，静音素材不能凭空多出音轨。

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
    let root = std::env::temp_dir().join(format!(
        "jianying-render-audio-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn streams(path: &Path) -> Vec<String> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=codec_type",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

fn make_clip(path: &Path, with_audio: bool) {
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc=s=320x180:d=2",
    ]);
    if with_audio {
        command.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=2"]);
        command.args(["-c:a", "aac", "-shortest"]);
    }
    let status = command
        .args(["-c:v", "libx264", "-pix_fmt", "yuv420p", "-y"])
        .arg(path)
        .status()
        .unwrap();
    assert!(status.success());
}

fn render(root: &Path, name: &str, source: &str) -> Value {
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":name,
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2000000,"source":source}
        ]}]
    });
    let plan_path = root.join(format!("{name}.json"));
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join(format!("draft-{name}"));
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    let out = root.join(format!("{name}.mp4"));
    let report = run_ok(&[
        "render",
        "proxy",
        &draft.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let mut report = report;
    report["streams"] = json!(streams(&out));
    report
}

#[test]
fn embedded_video_audio_is_mixed_into_the_preview() {
    if !media_ready() {
        return;
    }
    let root = temp_root("embedded");
    make_clip(&root.join("with-sound.mp4"), true);
    let report = render(&root, "with-sound", "with-sound.mp4");
    assert_eq!(report["audio_segments_mixed"], json!(1));
    let streams: Vec<&str> = report["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stream| stream.as_str().unwrap())
        .collect();
    assert!(streams.contains(&"audio"), "预览必须带音轨: {streams:?}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn silent_video_does_not_gain_an_audio_track() {
    if !media_ready() {
        return;
    }
    let root = temp_root("silent");
    make_clip(&root.join("mute.mp4"), false);
    let report = render(&root, "mute", "mute.mp4");
    assert_eq!(report["audio_segments_mixed"], json!(0));
    let streams: Vec<&str> = report["streams"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stream| stream.as_str().unwrap())
        .collect();
    assert!(
        !streams.contains(&"audio"),
        "静音素材不得凭空多出音轨: {streams:?}"
    );
    let _ = fs::remove_dir_all(root);
}

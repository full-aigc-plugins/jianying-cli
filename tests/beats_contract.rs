//! 节拍检测合同：合成 120 BPM 点击轨应估出接近的 BPM，持续音不得报节拍。

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

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-beats-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn ffmpeg_to(path: &PathBuf, args: &[&str]) {
    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-v", "error"]).args(args);
    command.args(["-c:a", "pcm_s16le", "-y"]).arg(path);
    let status = command.status().unwrap();
    assert!(status.success(), "ffmpeg 合成失败");
}

#[test]
fn detects_a_120_bpm_click_track_and_ignores_a_steady_tone() {
    if !media_ready() {
        return;
    }
    let root = temp_root("click");
    let click = root.join("click.wav");
    // 每 0.5s 一个 20ms 的 1kHz 脉冲 = 120 BPM
    ffmpeg_to(
        &click,
        &[
            "-f",
            "lavfi",
            "-i",
            "aevalsrc='if(lt(mod(t\\,0.5)\\,0.02)\\,0.9*sin(2*PI*1000*t)\\,0)':s=44100:d=8",
        ],
    );
    let report = run_ok(&["media", "beats", &click.to_string_lossy(), "--json"]);
    let bpm = report["estimated_bpm"].as_f64().expect("应给出 BPM 估计");
    assert!(
        (bpm - 120.0).abs() <= 12.0,
        "BPM 估计应接近 120，实际 {bpm}"
    );
    assert!(
        report["beat_count"].as_i64().unwrap() >= 10,
        "8s/120BPM 至少应有 10 个起点，实际 {}",
        report["beat_count"]
    );
    assert_eq!(report["resolution_ms"], json!(23), "包络分辨率应为 23ms");
    let confidence = report["confidence"].as_str().unwrap();
    assert!(
        confidence == "high" || confidence == "medium",
        "置信度 {confidence} 过低"
    );

    // 持续正弦没有起点：不得凭空报节拍
    let drone = root.join("drone.wav");
    ffmpeg_to(
        &drone,
        &["-f", "lavfi", "-i", "sine=frequency=220:duration=8"],
    );
    let quiet = run_ok(&["media", "beats", &drone.to_string_lossy(), "--json"]);
    assert_eq!(quiet["beat_count"], json!(0), "持续音不应被识别为节拍");
    assert_eq!(quiet["confidence"], json!("insufficient-onsets"));
    assert!(quiet["estimated_bpm"].is_null());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn rejects_an_invalid_min_gap() {
    let root = temp_root("invalid");
    let wav = root.join("silence.wav");
    fs::write(&wav, b"not really audio").unwrap();
    let output = run(&[
        "media",
        "beats",
        &wav.to_string_lossy(),
        "--min-gap",
        "0ms",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains("min-gap must be positive"));
    let _ = fs::remove_dir_all(root);
}

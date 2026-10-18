//! 单帧导出合同：`render frame` 出真 PNG，尺寸跟随 scale，MCP 同名工具行为一致。

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
    let root = std::env::temp_dir().join(format!("jianying-frame-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

/// PNG magic: 89 50 4E 47 0D 0A 1A 0A
fn is_png(path: &Path) -> bool {
    fs::read(path)
        .map(|bytes| bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]))
        .unwrap_or(false)
}

fn build_draft(root: &Path) -> PathBuf {
    let clip = root.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=2",
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
        "schema":"jianying-cli-plan/v1","name":"frame",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2000000,"source":"clip.mp4"}
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
fn frame_export_writes_a_png_at_the_requested_time() {
    if !media_ready() {
        return;
    }
    let root = temp_root("cli");
    let draft = build_draft(&root);
    let png = root.join("frame.png");
    let report = run_ok(&[
        "render",
        "frame",
        &draft.to_string_lossy(),
        "--at",
        "1500ms",
        "--out",
        &png.to_string_lossy(),
        "--scale",
        "1",
        "--json",
    ]);
    assert_eq!(report["status"], json!("frame"));
    assert_eq!(report["at_us"], json!(1_500_000));
    assert!(is_png(&png), "必须写出真正的 PNG");

    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "stream=width,height",
            "-of",
            "csv=p=0",
        ])
        .arg(&png)
        .output()
        .unwrap();
    let size = String::from_utf8_lossy(&probe.stdout).trim().to_owned();
    assert_eq!(size, "320,180", "scale=1 时应与画布同尺寸，实际 {size}");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn frame_export_is_reachable_over_mcp_stdio() {
    if !media_ready() {
        return;
    }
    let root = temp_root("mcp");
    let draft = build_draft(&root);
    let png = root.join("mcp-frame.png");
    let requests = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize",
               "params":{"protocolVersion":"2024-11-05","capabilities":{},
                          "clientInfo":{"name":"contract","version":"1"}}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{
            "name":"jianying_render_frame",
            "arguments":{"draft":draft.to_string_lossy(),"at_us":1000000,
                          "out":png.to_string_lossy(),"scale":0.5}}}),
    ];
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command
        .args(["mcp", "serve"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
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
        .expect("render_frame 响应");
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    let envelope: Value = serde_json::from_str(text).unwrap();
    assert_eq!(envelope["ok"], json!(true), "{envelope}");
    assert_eq!(envelope["data"]["status"], json!("frame"));
    assert!(is_png(&png), "MCP 工具同样必须写出 PNG");
    let _ = fs::remove_dir_all(root);
}

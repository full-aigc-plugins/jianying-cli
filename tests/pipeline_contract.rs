//! 批量流水线合同：目录 → 构建 → 评分 → 渲染，失败隔离且可作 CI 门禁。

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

        let root = std::env::temp_dir().join(format!("jianying-pipe-bin-{}", std::process::id()));
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
            "jianying-pipe-processes-{}.txt",
            std::process::id()
        ));
        fs::write(&path, "Image Name,PID\r\n").unwrap();
        path.to_string_lossy().into_owned()
    })
}

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-pipe-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
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
            "color=c=0x228833:s=320x180:d=1",
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

fn plan(duration_us: i64) -> Value {
    json!({
        "schema":"jianying-cli-plan/v1","name":"pipe",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":duration_us,"source":"clip.mp4"}
        ]}]
    })
}

#[test]
fn pipeline_builds_scores_and_renders_each_plan_and_isolates_failures() {
    if !media_ready() {
        return;
    }
    let root = temp_root("mixed");
    let plans = root.join("plans");
    fs::create_dir_all(&plans).unwrap();
    write_clip(&plans.join("clip.mp4"));
    fs::write(
        plans.join("a.json"),
        serde_json::to_string(&plan(1_000_000)).unwrap(),
    )
    .unwrap();
    fs::write(
        plans.join("b.json"),
        serde_json::to_string(&plan(1_000_000)).unwrap(),
    )
    .unwrap();
    // 坏计划：schema 不匹配，必须只影响自己
    fs::write(plans.join("c.json"), r#"{"name":"broken"}"#).unwrap();

    let output = run(&[
        "pipeline",
        &plans.to_string_lossy(),
        "--out",
        &root.join("out").to_string_lossy(),
        "--fail-under",
        "90",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "有失败条目必须退出码 1");
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let data = &report["data"];
    assert_eq!(data["total"], json!(3));
    assert_eq!(data["succeeded"], json!(2));
    assert_eq!(data["failed"], json!(1));
    for item in data["items"].as_array().unwrap() {
        let name = item["name"].as_str().unwrap();
        if name == "c" {
            assert_eq!(item["ok"], json!(false));
            assert!(item["error"].as_str().unwrap().contains("plan"));
        } else {
            assert_eq!(item["ok"], json!(true), "{item}");
            assert_eq!(item["score"], json!(100));
            assert!(item["issues"].as_array().unwrap().is_empty());
            let render = item["render"].as_str().unwrap();
            assert!(Path::new(render).is_file(), "成片必须存在: {render}");
        }
    }
    let _ = fs::remove_dir_all(root);
}

#[test]
fn pipeline_fails_closed_when_a_score_is_below_the_gate() {
    if !media_ready() {
        return;
    }
    let root = temp_root("gate");
    let plans = root.join("plans");
    fs::create_dir_all(&plans).unwrap();
    write_clip(&plans.join("clip.mp4"));
    fs::write(
        plans.join("a.json"),
        serde_json::to_string(&plan(1_000_000)).unwrap(),
    )
    .unwrap();

    let ok = run(&[
        "pipeline",
        &plans.to_string_lossy(),
        "--out",
        &root.join("out-ok").to_string_lossy(),
        "--fail-under",
        "100",
        "--json",
    ]);
    assert_eq!(ok.status.code(), Some(0));

    // 门槛设到 101 不可达 → 必须判失败
    let strict = run(&[
        "pipeline",
        &plans.to_string_lossy(),
        "--out",
        &root.join("out-strict").to_string_lossy(),
        "--fail-under",
        "101",
        "--json",
    ]);
    assert_eq!(strict.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&strict.stdout).unwrap();
    assert_eq!(report["data"]["failed"], json!(1));
    assert_eq!(report["data"]["items"][0]["ok"], json!(false));
    let _ = fs::remove_dir_all(root);
}

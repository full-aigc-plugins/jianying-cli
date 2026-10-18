//! 两条写入路径的等价性回归。
//!
//! `jianying-cli` 目前有两条写草稿的路径：`project build`（plan → 新草稿）与
//! `timeline ...`（对已有草稿做事务化编辑）。同一语义必须写出同一份 wire，
//! 否则"新建时正确、编辑时偏 1µs"这类偏差会长期潜伏。
//!
//! 本文件用同一语义的两种到达方式对比：
//!   A. 计划里直接带该属性 → `draft::build`
//!   B. 计划里不带该属性 → build 后调用 `timeline` 命令设置
//! 两者归一化后的 wire 必须逐字段相等。

use anyhow::Result;
use jianying_cli::lossless_draft::DraftTimelineWire;
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
    // 与其它时间线合同一致：用空 `ps` 替身隔离开发机上正在运行的剪映。
    #[cfg(unix)]
    command.env("PATH", isolated_process_path());
    // Windows 的 tasklist 固定在 System32，只能用清单注入点隔离。
    #[cfg(windows)]
    command.env("JIANYING_CLI_PROCESS_LIST", empty_process_listing());
    command.output().unwrap()
}

#[cfg(unix)]
fn isolated_process_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "jianying-path-equivalence-bin-{}",
            std::process::id()
        ));
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
            "jianying-path-equivalence-processes-{}.txt",
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

fn video_probe(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 4_000_000,
        width: 1920,
        height: 1080,
        has_video: true,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn audio_probe(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 4_000_000,
        width: 0,
        height: 0,
        has_video: false,
        has_audio: true,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "jianying-path-equivalence-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

/// 用给定的 plan 值构建一份草稿，返回草稿目录。
fn build(
    root: &Path,
    name: &str,
    plan: Value,
    prober: &dyn Fn(&Path) -> Result<MediaInfo>,
) -> PathBuf {
    let plan: Plan = serde_json::from_value(plan).unwrap();
    let out = root.join(name);
    draft::build(&plan, root, &out, None, prober).unwrap();
    out
}

/// 归一化草稿 wire：抹掉随机 id、时间戳与路径，只保留语义字段。
fn normalized(dir: &Path) -> Value {
    let content: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("draft_content.json")).unwrap()).unwrap();
    DraftTimelineWire::from_value(content)
        .unwrap()
        .normalized_core()
}

fn video_plan(duration_us: i64, extra: Value) -> Value {
    let mut segment = json!({
        "start_us": 0,
        "duration_us": duration_us,
        "source": "clip.mp4"
    });
    if let Some(object) = extra.as_object() {
        for (key, value) in object {
            segment[key] = value.clone();
        }
    }
    json!({
        "schema": "jianying-cli-plan/v1",
        "name": "equivalence",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "video", "segments": [segment]}]
    })
}

fn audio_plan(extra: Value) -> Value {
    let mut segment = json!({
        "start_us": 0,
        "duration_us": 2_000_000,
        "source": "voice.mp3"
    });
    if let Some(object) = extra.as_object() {
        for (key, value) in object {
            segment[key] = value.clone();
        }
    }
    json!({
        "schema": "jianying-cli-plan/v1",
        "name": "equivalence",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "audio", "segments": [segment]}]
    })
}

/// 取草稿里第一条视频片段的 id。
fn first_segment_id(draft_dir: &Path, track: &str) -> String {
    let listed = run_ok(&[
        "timeline",
        "segments",
        &draft_dir.to_string_lossy(),
        "--track",
        track,
        "--json",
    ]);
    listed[0]["id"].as_str().unwrap().to_owned()
}

#[test]
fn speed_is_identical_whether_authored_or_applied_afterwards() {
    let root = temp_root("speed");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    // 3000001 × 0.5 = 1500000.5：正好落在 ties-to-even 的边界上。
    let authored = build(
        &root,
        "authored",
        video_plan(3_000_001, json!({"speed": 0.5})),
        &video_probe,
    );
    let edited = build(
        &root,
        "edited",
        video_plan(3_000_001, json!({})),
        &video_probe,
    );
    let id = first_segment_id(&edited, "video");
    run_ok(&[
        "timeline",
        "speed",
        &edited.to_string_lossy(),
        &id,
        "0.5",
        "--json",
    ]);

    assert_eq!(
        normalized(&authored),
        normalized(&edited),
        "计划内 speed 与事后 timeline speed 必须写出同一份 wire"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn speed_via_timeline_set_matches_the_authored_wire() {
    let root = temp_root("set-speed");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    let authored = build(
        &root,
        "authored",
        video_plan(3_000_001, json!({"speed": 0.5})),
        &video_probe,
    );
    let edited = build(
        &root,
        "edited",
        video_plan(3_000_001, json!({})),
        &video_probe,
    );
    let id = first_segment_id(&edited, "video");
    run_ok(&[
        "timeline",
        "set",
        &edited.to_string_lossy(),
        &id,
        "--speed",
        "0.5",
        "--json",
    ]);

    // 旧实现只改 `speed` 字段，源区间仍停在 3000001；这里同时钉住该修复。
    let content: Value =
        serde_json::from_str(&fs::read_to_string(edited.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(
        content["tracks"][0]["segments"][0]["source_timerange"]["duration"],
        json!(1_500_000),
        "设置速度必须同步 source_timerange.duration"
    );
    assert_eq!(
        normalized(&authored),
        normalized(&edited),
        "timeline set --speed 必须与计划内 speed 写出同一份 wire"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn volume_is_identical_whether_authored_or_applied_afterwards() {
    let root = temp_root("volume");
    fs::write(root.join("voice.mp3"), b"fixture").unwrap();

    let authored = build(
        &root,
        "authored",
        audio_plan(json!({"volume": 0.25})),
        &audio_probe,
    );
    let edited = build(&root, "edited", audio_plan(json!({})), &audio_probe);
    let id = first_segment_id(&edited, "audio");
    run_ok(&[
        "timeline",
        "volume",
        &edited.to_string_lossy(),
        &id,
        "0.25",
        "--json",
    ]);

    assert_eq!(
        normalized(&authored),
        normalized(&edited),
        "计划内 volume 与事后 timeline volume 必须写出同一份 wire"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn opacity_is_identical_whether_authored_or_applied_afterwards() {
    let root = temp_root("opacity");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    let authored = build(
        &root,
        "authored",
        video_plan(2_000_000, json!({"opacity": 0.4})),
        &video_probe,
    );
    let edited = build(
        &root,
        "edited",
        video_plan(2_000_000, json!({})),
        &video_probe,
    );
    let id = first_segment_id(&edited, "video");
    run_ok(&[
        "timeline",
        "opacity",
        &edited.to_string_lossy(),
        &id,
        "0.4",
        "--json",
    ]);

    assert_eq!(
        normalized(&authored),
        normalized(&edited),
        "计划内 opacity 与事后 timeline opacity 必须写出同一份 wire"
    );
    let _ = fs::remove_dir_all(root);
}

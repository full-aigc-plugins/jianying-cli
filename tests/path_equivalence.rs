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

/// 抽取关键帧通道，抹掉随机 id/materal_id，按通道名与时间排序，数值统一成 f64。
///
/// `normalized_core()` 不含 `common_keyframes`，关键帧必须单独比对。
fn keyframe_channels(dir: &Path) -> Value {
    let content: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("draft_content.json")).unwrap()).unwrap();
    let mut segments = Vec::new();
    for track in content["tracks"].as_array().into_iter().flatten() {
        for segment in track["segments"].as_array().into_iter().flatten() {
            let mut channels: Vec<Value> = segment["common_keyframes"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|channel| {
                    let mut points: Vec<Value> = channel["keyframe_list"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .map(|point| {
                            json!({
                                "time_offset": point["time_offset"].as_i64().unwrap_or(0),
                                "values": point["values"].as_array().into_iter().flatten()
                                    .map(|value| value.as_f64().unwrap_or(0.0))
                                    .collect::<Vec<_>>(),
                            })
                        })
                        .collect();
                    points.sort_by_key(|point| point["time_offset"].as_i64().unwrap_or(0));
                    json!({
                        "property_type": channel["property_type"].as_str().unwrap_or(""),
                        "points": points,
                    })
                })
                .collect();
            channels.sort_by(|left, right| {
                left["property_type"]
                    .as_str()
                    .cmp(&right["property_type"].as_str())
            });
            segments.push(json!(channels));
        }
    }
    json!(segments)
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

#[test]
fn keyframes_are_identical_whether_authored_or_applied_afterwards() {
    let root = temp_root("keyframes");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    let authored = build(
        &root,
        "authored",
        video_plan(
            2_000_000,
            json!({"keyframes": {"x": [
                {"at_us": 0, "value": 0.0},
                {"at_us": 1_000_000, "value": 0.5}
            ]}}),
        ),
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
        "keyframe",
        &edited.to_string_lossy(),
        &id,
        "x",
        "0us",
        "0",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "keyframe",
        &edited.to_string_lossy(),
        &id,
        "x",
        "1000000us",
        "0.5",
        "--json",
    ]);

    assert_eq!(
        keyframe_channels(&authored),
        keyframe_channels(&edited),
        "计划内 keyframes 与事后 timeline keyframe 必须写出同一组通道"
    );
    let _ = fs::remove_dir_all(root);
}
#[test]
fn scale_keyframe_alias_is_identical_whether_authored_or_applied_afterwards() {
    let root = temp_root("scale-kf");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    let authored = build(
        &root,
        "authored",
        video_plan(
            2_000_000,
            json!({"keyframes": {"scale": [
                {"at_us": 0, "value": 1.0},
                {"at_us": 1_000_000, "value": 2.0}
            ]}}),
        ),
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
        "keyframe",
        &edited.to_string_lossy(),
        &id,
        "scale",
        "0us",
        "1.0",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "keyframe",
        &edited.to_string_lossy(),
        &id,
        "scale",
        "1000000us",
        "2.0",
        "--json",
    ]);

    assert_eq!(
        keyframe_channels(&authored),
        keyframe_channels(&edited),
        "同一个 scale 别名在两条路径上必须落到同一通道"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn plan_expresses_every_keyframe_channel_the_cli_supports() {
    let root = temp_root("all-channels");
    fs::write(root.join("clip.mp4"), b"fixture").unwrap();

    // 与 `timeline keyframe` 的 11 个通道一一对应；短别名 scale/x/y/opacity
    // 与显式短名落到同一通道，这里全部用显式名，避免别名掩盖差异。
    let channels = [
        ("scale_x", "KFTypeScaleX"),
        ("scale_y", "KFTypeScaleY"),
        ("uniform_scale", "UNIFORM_SCALE"),
        ("position_x", "KFTypePositionX"),
        ("position_y", "KFTypePositionY"),
        ("rotation", "KFTypeRotation"),
        ("alpha", "KFTypeAlpha"),
        ("saturation", "KFTypeSaturation"),
        ("contrast", "KFTypeContrast"),
        ("brightness", "KFTypeBrightness"),
        ("volume", "KFTypeVolume"),
    ];
    let mut keyframes = serde_json::Map::new();
    for (channel, _) in channels {
        keyframes.insert(
            channel.to_owned(),
            json!([{"at_us": 0, "value": 1.0}, {"at_us": 1_000_000, "value": 1.0}]),
        );
    }
    let dir = build(
        &root,
        "all-channels",
        video_plan(2_000_000, json!({"keyframes": keyframes})),
        &video_probe,
    );

    let mut written: Vec<String> = keyframe_channels(&dir)[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|channel| channel["property_type"].as_str().unwrap().to_owned())
        .collect();
    written.sort();
    let mut expected: Vec<String> = channels.iter().map(|(_, wire)| wire.to_string()).collect();
    expected.sort();
    assert_eq!(
        written, expected,
        "计划必须能表达命令行支持的每一个关键帧通道"
    );
    assert_eq!(
        run_ok(&["project", "verify", &dir.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn text_typography_fields_follow_the_reference_conversion() {
    let root = temp_root("typography");
    let plan = json!({
        "schema": "jianying-cli-plan/v1",
        "name": "typography",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "text", "segments": [{
            "start_us": 0,
            "duration_us": 2_000_000,
            "text": "排版",
            "vertical": true,
            "letter_spacing": 4,
            "line_spacing": 6,
            "auto_wrapping": true,
            "max_line_width": 0.6,
            "text_alpha": 0.8
        }]}]
    });
    let dir = build(&root, "draft", plan, &video_probe);
    let content: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("draft_content.json")).unwrap()).unwrap();
    let material = &content["materials"]["texts"][0];
    // pyJianYingDraft: typesetting=int(vertical)、letter_spacing=值×0.05、
    // line_spacing=0.02+值×0.05、type 由 auto_wrapping 决定。
    assert_eq!(material["typesetting"], json!(1));
    // 与 Python 同样的 IEEE754 运算顺序，浮点尾数也一致。
    assert_eq!(material["letter_spacing"].as_f64().unwrap(), 4.0 * 0.05);
    assert_eq!(
        material["line_spacing"].as_f64().unwrap(),
        0.02 + 6.0 * 0.05
    );
    assert_eq!(material["line_max_width"], json!(0.6));
    assert_eq!(material["global_alpha"], json!(0.8));
    assert_eq!(material["type"], json!("subtitle"));
    assert_eq!(
        run_ok(&["project", "verify", &dir.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn text_typography_defaults_stay_byte_identical() {
    let root = temp_root("typography-default");
    let plan = json!({
        "schema": "jianying-cli-plan/v1",
        "name": "typography-default",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "text", "segments": [{
            "start_us": 0,
            "duration_us": 2_000_000,
            "text": "默认"
        }]}]
    });
    let dir = build(&root, "draft", plan, &video_probe);
    let content: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("draft_content.json")).unwrap()).unwrap();
    let material = &content["materials"]["texts"][0];
    assert_eq!(material["typesetting"], json!(0));
    assert_eq!(material["letter_spacing"], json!(0));
    assert_eq!(material["line_spacing"], json!(0.02));
    assert_eq!(material["line_max_width"], json!(0.82));
    assert_eq!(material["global_alpha"], json!(1.0));
    assert_eq!(material["type"], json!("text"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn maintrack_adsorb_is_carried_from_the_plan_and_defaults_to_true() {
    let root = temp_root("maintrack");
    let base = json!({
        "schema": "jianying-cli-plan/v1",
        "name": "maintrack",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "text", "segments": [{
            "start_us": 0, "duration_us": 1_000_000, "text": "吸附"
        }]}]
    });
    let default_dir = build(&root, "default", base.clone(), &video_probe);
    let default_content: Value =
        serde_json::from_str(&fs::read_to_string(default_dir.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(default_content["config"]["maintrack_adsorb"], json!(true));

    let mut off = base;
    off["maintrack_adsorb"] = json!(false);
    let off_dir = build(&root, "off", off, &video_probe);
    let off_content: Value =
        serde_json::from_str(&fs::read_to_string(off_dir.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(off_content["config"]["maintrack_adsorb"], json!(false));
    let _ = fs::remove_dir_all(root);
}

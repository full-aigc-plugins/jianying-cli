use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
#[cfg(unix)]
use std::sync::OnceLock;

fn run(args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_jianying"));
    command.args(args);
    // 时间线合同必须只观察合成草稿，不能被开发机上正在运行的剪映污染。
    // 用无输出的 `ps` 测试替身隔离宿主进程；生产二进制仍保持运行中拒写门禁。
    #[cfg(unix)]
    command.env("PATH", isolated_process_path());
    command.output().unwrap()
}

#[cfg(unix)]
fn isolated_process_path() -> &'static str {
    static PATH: OnceLock<String> = OnceLock::new();
    PATH.get_or_init(|| {
        use std::os::unix::fs::PermissionsExt;

        let root = std::env::temp_dir().join(format!(
            "jianying-timeline-contract-bin-{}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        let ps = root.join("ps");
        fs::write(&ps, "#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(&ps, fs::Permissions::from_mode(0o700)).unwrap();
        root.to_string_lossy().into_owned()
    })
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap()
}

fn run_ok(args: &[&str]) -> serde_json::Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    json(&output)["data"].clone()
}

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 2_000_000,
        width: 1920,
        height: 1080,
        has_video: true,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn audio_probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 3_000_000,
        width: 0,
        height: 0,
        has_video: false,
        has_audio: true,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

#[test]
fn track_mute_preserves_unknown_track_state_and_segment_volume() {
    let root = std::env::temp_dir().join(format!(
        "jianying-track-mute-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("music.wav"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"track-mute",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"audio","segments":[{
            "start_us":0,"duration_us":3000000,"source":"music.wav"
        }]}]
    }))
    .unwrap();
    let output = root.join("draft");
    draft::build(&plan, &root, &output, None, &audio_probe_stub).unwrap();
    let mut timeline = draft::load_timeline(&output).unwrap();
    let track_id = timeline["tracks"][0]["id"].as_str().unwrap().to_owned();
    timeline["tracks"][0]["attribute"] = serde_json::json!(8);
    timeline["tracks"][0]["unrecognized_track_field"] = serde_json::json!({"keep":true});
    timeline["tracks"][0]["segments"][0]["volume"] = serde_json::json!(0.37);
    jianying_cli::template::save_timeline(&output, &timeline).unwrap();

    let muted = run_ok(&[
        "timeline",
        "track-mute",
        &output.to_string_lossy(),
        &track_id,
        "true",
        "--json",
    ]);
    assert_eq!(muted["old_muted"], false);
    assert_eq!(muted["new_muted"], true);
    assert_eq!(muted["old_attribute"], 8);
    assert_eq!(muted["new_attribute"], 9);
    let after = draft::load_timeline(&output).unwrap();
    assert_eq!(after["tracks"][0]["attribute"], 9);
    assert_eq!(after["tracks"][0]["unrecognized_track_field"]["keep"], true);
    assert_eq!(after["tracks"][0]["segments"][0]["volume"], 0.37);
    assert_eq!(
        run_ok(&["timeline", "tracks", &output.to_string_lossy(), "--json"])[0]["muted"],
        true
    );

    let unmuted = run_ok(&[
        "timeline",
        "track-mute",
        &output.to_string_lossy(),
        &track_id,
        "false",
        "--json",
    ]);
    assert_eq!(unmuted["old_attribute"], 9);
    assert_eq!(unmuted["new_attribute"], 8);
    let before_invalid = std::fs::read(output.join("draft_content.json")).unwrap();
    let unknown = run(&[
        "timeline",
        "track-mute",
        &output.to_string_lossy(),
        "missing-track",
        "true",
        "--json",
    ]);
    assert_eq!(unknown.status.code(), Some(1));
    assert_eq!(
        std::fs::read(output.join("draft_content.json")).unwrap(),
        before_invalid
    );
    let mut invalid = draft::load_timeline(&output).unwrap();
    invalid["tracks"][0]["attribute"] = serde_json::json!("unknown");
    let malformed_wire = serde_json::to_vec_pretty(&invalid).unwrap();
    std::fs::write(output.join("draft_content.json"), &malformed_wire).unwrap();
    std::fs::write(output.join("draft_info.json"), &malformed_wire).unwrap();
    let before_invalid_attribute = std::fs::read(output.join("draft_content.json")).unwrap();
    let malformed = run(&[
        "timeline",
        "track-mute",
        &output.to_string_lossy(),
        &track_id,
        "true",
        "--json",
    ]);
    assert_eq!(malformed.status.code(), Some(1));
    assert_eq!(
        std::fs::read(output.join("draft_content.json")).unwrap(),
        before_invalid_attribute
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn quantization_report_is_exposed_as_a_read_only_cli_contract() {
    let report = run_ok(&[
        "timeline",
        "quantization-report",
        "10ms",
        "50ms",
        "--fps-numerator",
        "30",
        "--fps-denominator",
        "1",
        "--maximum-drift",
        "30ms",
        "--json",
    ]);
    assert_eq!(report["original"]["start_us"], 10_000);
    assert_eq!(report["original"]["duration_us"], 50_000);
    assert_eq!(report["quantized"]["start_us"], 0);
    assert_eq!(report["quantized"]["duration_us"], 66_666);
    assert_eq!(report["start_delta_us"], -10_000);
    assert_eq!(report["end_delta_us"], 6_666);

    let rejected = run(&[
        "timeline",
        "quantization-report",
        "10ms",
        "50ms",
        "--fps-numerator",
        "30",
        "--maximum-drift",
        "1ms",
        "--json",
    ]);
    assert_eq!(rejected.status.code(), Some(1));
    assert_eq!(json(&rejected)["error"]["type"], "execution_failed");
    assert!(json(&rejected)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("quantization drift"));
}

#[test]
fn timeline_queries_and_mutations_use_the_transactional_cli() {
    let root = std::env::temp_dir().join(format!("jianying-timeline-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:02,000\nhello\n").unwrap();
    let draft = root.join("draft");
    let created = run(&[
        "project",
        "quickstart",
        "timeline",
        "--out",
        &draft.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(created.status.code(), Some(0));

    let tracks = run(&["timeline", "tracks", &draft.to_string_lossy(), "--json"]);
    assert_eq!(
        tracks.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&tracks.stdout),
        String::from_utf8_lossy(&tracks.stderr)
    );
    assert_eq!(json(&tracks)["data"][0]["type"], "text");
    let layout = run_ok(&[
        "timeline",
        "show",
        &draft.to_string_lossy(),
        "--cols",
        "73",
        "--json",
    ]);
    assert_eq!(layout["span_us"], 2_000_000);
    assert_eq!(layout["cols"], 73);
    assert_eq!(layout["tracks"][0]["segments"][0]["col_end"], 73);

    let listed = run_ok(&[
        "timeline",
        "segments",
        &draft.to_string_lossy(),
        "--track",
        "text",
        "--json",
    ]);
    let id = listed[0]["id"].as_str().unwrap().to_owned();
    assert_eq!(listed[0]["duration_us"], 2_000_000);
    assert_eq!(
        run_ok(&["timeline", "get", &draft.to_string_lossy(), &id, "--json"])["_track_type"],
        "text"
    );

    run_ok(&[
        "timeline",
        "speed",
        &draft.to_string_lossy(),
        &id,
        "2",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "volume",
        &draft.to_string_lossy(),
        &id,
        "0.5",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "trim",
        &draft.to_string_lossy(),
        &id,
        "0us",
        "1s",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "move",
        &draft.to_string_lossy(),
        &id,
        "250ms",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "set",
        &draft.to_string_lossy(),
        &id,
        "--volume",
        "0.75",
        "--json",
    ]);
    let split = run_ok(&[
        "timeline",
        "split",
        &draft.to_string_lossy(),
        &id,
        "500ms",
        "--json",
    ]);
    let right_id = split["new_segment_id"].as_str().unwrap().to_owned();
    let right_detail = run_ok(&[
        "timeline",
        "get",
        &draft.to_string_lossy(),
        &right_id,
        "--json",
    ]);
    let duplicated = run_ok(&[
        "timeline",
        "duplicate",
        &draft.to_string_lossy(),
        &right_id,
        "--json",
    ]);
    assert_ne!(duplicated["material_id"], right_detail["material_id"]);
    assert!(duplicated["cloned_materials"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["source_id"] == right_detail["material_id"]));
    let duplicate_id = duplicated["new_segment_id"].as_str().unwrap().to_owned();
    let removed = run_ok(&[
        "timeline",
        "remove",
        &draft.to_string_lossy(),
        &duplicate_id,
        "--json",
    ]);
    assert!(removed["materials_removed"].as_u64().unwrap() >= 1);
    run_ok(&[
        "timeline",
        "add-track",
        &draft.to_string_lossy(),
        "audio",
        "music",
        "--json",
    ]);
    let added_track = run_ok(&[
        "timeline",
        "add-track",
        &draft.to_string_lossy(),
        "text",
        "secondary",
        "--json",
    ]);
    let track_id = added_track["track_id"].as_str().unwrap();
    let mut injected = run_ok(&[
        "timeline",
        "get",
        &draft.to_string_lossy(),
        &right_id,
        "--json",
    ]);
    for field in ["_track_type", "_track_name", "_track_id", "_material"] {
        injected.as_object_mut().unwrap().remove(field);
    }
    injected["id"] = serde_json::json!("injected-segment");
    let segment_file = root.join("segment.json");
    std::fs::write(&segment_file, serde_json::to_vec_pretty(&injected).unwrap()).unwrap();
    run_ok(&[
        "timeline",
        "add-segment",
        &draft.to_string_lossy(),
        track_id,
        &segment_file.to_string_lossy(),
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "move-all",
        &draft.to_string_lossy(),
        "0us",
        "--track",
        "text",
        "--json",
    ]);

    let verified = run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"]);
    assert_eq!(verified["ok"], true);
    assert!(root.join(".jianying-transactions").is_dir());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn visual_timeline_opacity_and_composite_remain_structurally_valid() {
    let root = std::env::temp_dir().join(format!("jianying-visual-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("clip.mp4"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"visual",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"video","segments":[{
            "start_us":0,"duration_us":2000000,"source":"clip.mp4"
        }]}]
    }))
    .unwrap();
    let output = root.join("draft");
    draft::build(&plan, &root, &output, None, &probe_stub).unwrap();
    let listed = run_ok(&[
        "timeline",
        "segments",
        &output.to_string_lossy(),
        "--track",
        "video",
        "--json",
    ]);
    let id = listed[0]["id"].as_str().unwrap();
    run_ok(&[
        "timeline",
        "opacity",
        &output.to_string_lossy(),
        id,
        "0.4",
        "--json",
    ]);
    run_ok(&[
        "timeline",
        "composite",
        &output.to_string_lossy(),
        id,
        "正片叠底",
        "--json",
    ]);
    let detail = run_ok(&["timeline", "get", &output.to_string_lossy(), id, "--json"]);
    assert_eq!(detail["clip"]["alpha"], 0.4);
    assert!(detail["extra_material_refs"].as_array().unwrap().len() >= 4);
    assert_eq!(
        run_ok(&["project", "verify", &output.to_string_lossy(), "--json"])["ok"],
        true
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_prune_is_conservative_and_dry_run_is_read_only() {
    let root = std::env::temp_dir().join(format!("jianying-prune-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:02,000\nhello\n").unwrap();
    let draft = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "prune",
        "--out",
        &draft.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);

    let mut timeline = draft::load_timeline(&draft).unwrap();
    let referenced = timeline["tracks"][0]["segments"][0]["material_id"]
        .as_str()
        .unwrap()
        .to_owned();
    timeline["materials"]["effects"] = serde_json::json!([
        {"id":"orphan-effect","name":"remove me"},
        {"name":"anonymous entry must survive"}
    ]);
    let encoded = serde_json::to_vec_pretty(&timeline).unwrap();
    std::fs::write(draft.join("draft_content.json"), &encoded).unwrap();
    std::fs::write(draft.join("draft_info.json"), &encoded).unwrap();
    let before = std::fs::read(draft.join("draft_content.json")).unwrap();

    let preview = run_ok(&[
        "project",
        "prune",
        &draft.to_string_lossy(),
        "--dry-run",
        "--json",
    ]);
    assert_eq!(preview["removed"], 1);
    assert_eq!(preview["by_type"]["effects"]["removed"], 1);
    assert_eq!(preview["by_type"]["effects"]["kept"], 1);
    assert_eq!(preview["dryRun"], true);
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        before
    );

    let pruned = run_ok(&["project", "prune", &draft.to_string_lossy(), "--json"]);
    assert_eq!(pruned["removed"], 1);
    let after = draft::load_timeline(&draft).unwrap();
    assert!(after["materials"]
        .as_object()
        .unwrap()
        .values()
        .any(|items| {
            items
                .as_array()
                .into_iter()
                .flatten()
                .any(|item| item["id"].as_str() == Some(referenced.as_str()))
        }));
    assert_eq!(after["materials"]["effects"].as_array().unwrap().len(), 1);
    assert_eq!(
        after["materials"]["effects"][0]["name"],
        "anonymous entry must survive"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn matting_is_material_scoped_and_preserves_app_cache_fields() {
    let root = std::env::temp_dir().join(format!("jianying-matting-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("clip.mp4"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"matting",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"video","segments":[{
            "start_us":0,"duration_us":2000000,"source":"clip.mp4"
        }]}]
    }))
    .unwrap();
    let output = root.join("draft");
    draft::build(&plan, &root, &output, None, &probe_stub).unwrap();
    let mut timeline = draft::load_timeline(&output).unwrap();
    let first_id = timeline["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let material_id = timeline["tracks"][0]["segments"][0]["material_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let mut shared = timeline["tracks"][0]["segments"][0].clone();
    shared["id"] = serde_json::json!("shared-segment");
    shared["target_timerange"]["start"] = serde_json::json!(2_000_000);
    timeline["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(shared);
    timeline["duration"] = serde_json::json!(4_000_000);
    let material = timeline["materials"]["videos"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|item| item["id"].as_str() == Some(material_id.as_str()))
        .unwrap();
    material["matting"] = serde_json::json!({
        "flag":0,"has_use_quick_brush":true,"interactiveTime":[1],
        "path":"app-cache.bin","strokes":[{"x":1}],"vendor_unknown":"keep"
    });
    let encoded = serde_json::to_vec_pretty(&timeline).unwrap();
    std::fs::write(output.join("draft_content.json"), &encoded).unwrap();
    std::fs::write(output.join("draft_info.json"), &encoded).unwrap();

    let enabled = run_ok(&[
        "timeline",
        "matting",
        &output.to_string_lossy(),
        &first_id,
        "--json",
    ]);
    assert_eq!(
        enabled,
        serde_json::json!({
            "ok":true,"segmentId":first_id,"materialId":material_id,
            "flag":3,"enabled":true,"shared_segments":["shared-segment"]
        })
    );
    let timeline = draft::load_timeline(&output).unwrap();
    let matting = &timeline["materials"]["videos"][0]["matting"];
    assert_eq!(matting["path"], "app-cache.bin");
    assert_eq!(matting["strokes"][0]["x"], 1);
    assert_eq!(matting["vendor_unknown"], "keep");

    let disabled = run_ok(&[
        "timeline",
        "matting",
        &output.to_string_lossy(),
        &first_id,
        "--off",
        "--json",
    ]);
    assert_eq!(disabled["flag"], 0);
    assert_eq!(disabled["enabled"], false);
    assert_eq!(
        draft::load_timeline(&output).unwrap()["materials"]["videos"][0]["matting"]["path"],
        "app-cache.bin"
    );

    let keyed = run_ok(&[
        "timeline",
        "chroma",
        &output.to_string_lossy(),
        &first_id,
        "--color",
        "#00FF00",
        "--intensity",
        "1.7",
        "--json",
    ]);
    assert_eq!(keyed["color"], "#00FF00");
    assert_eq!(keyed["intensity"], 1.0);
    assert_eq!(keyed["shadow"], 0.0);
    let chroma_id = keyed["materialId"].as_str().unwrap().to_owned();
    let timeline = draft::load_timeline(&output).unwrap();
    assert!(timeline["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str() == Some(chroma_id.as_str())));
    assert_eq!(
        timeline["materials"]["chromas"].as_array().unwrap().len(),
        1
    );
    let cleared = run_ok(&[
        "timeline",
        "chroma",
        &output.to_string_lossy(),
        &first_id,
        "--off",
        "--json",
    ]);
    assert_eq!(cleared["removed"], serde_json::json!([chroma_id]));
    assert!(
        draft::load_timeline(&output).unwrap()["materials"]["chromas"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let masked = run_ok(&[
        "timeline",
        "mask",
        &output.to_string_lossy(),
        &first_id,
        "rectangle",
        "--center-x",
        "0.2",
        "--center-y",
        "-0.1",
        "--size",
        "0.6",
        "--rotation",
        "15",
        "--feather",
        "20",
        "--invert",
        "--rect-width",
        "0.8",
        "--round-corner",
        "30",
        "--json",
    ]);
    assert_eq!(masked["name"], "Rectangle");
    assert_eq!(masked["field"], "masks");
    let mask_id = masked["mask_id"].as_str().unwrap().to_owned();
    let timeline = draft::load_timeline(&output).unwrap();
    let mask = timeline["materials"]["masks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == mask_id)
        .unwrap();
    assert_eq!(mask["config"]["width"], 0.8);
    assert_eq!(mask["config"]["height"], 0.6);
    assert_eq!(mask["config"]["feather"], 0.2);
    assert_eq!(mask["config"]["roundCorner"], 0.3);
    assert_eq!(mask["resource_id"], "7374021450748924432");
    let unmasked = run_ok(&[
        "timeline",
        "mask",
        &output.to_string_lossy(),
        &first_id,
        "--off",
        "--json",
    ]);
    assert_eq!(unmasked["removed"], 1);
    assert!(
        !draft::load_timeline(&output).unwrap()["tracks"][0]["segments"][0]["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some(mask_id.as_str()))
    );

    let blurred = run_ok(&[
        "timeline",
        "bg-blur",
        &output.to_string_lossy(),
        &first_id,
        "2",
        "--json",
    ]);
    assert_eq!(blurred["blur"], 0.375);
    let canvas_id = blurred["canvas_id"].as_str().unwrap().to_owned();
    let timeline = draft::load_timeline(&output).unwrap();
    assert!(timeline["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item.as_str() == Some(canvas_id.as_str())));
    assert_eq!(
        timeline["materials"]["canvases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["id"].as_str() == Some(canvas_id.as_str()))
            .unwrap()["type"],
        "canvas_blur"
    );
    let blur_off = run_ok(&[
        "timeline",
        "bg-blur",
        &output.to_string_lossy(),
        &first_id,
        "--off",
        "--json",
    ]);
    assert!(blur_off["canvas_id"].is_null());
    assert!(blur_off["blur"].is_null());
    assert!(
        !draft::load_timeline(&output).unwrap()["tracks"][0]["segments"][0]["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some(canvas_id.as_str()))
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn audio_fade_uses_prefix_ids_and_replaces_the_active_reference() {
    let root = std::env::temp_dir().join(format!("jianying-fade-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("music.wav"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"fade",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"audio","segments":[{
            "start_us":0,"duration_us":3000000,"source":"music.wav"
        }]}]
    }))
    .unwrap();
    let output = root.join("draft");
    draft::build(&plan, &root, &output, None, &audio_probe_stub).unwrap();
    let segment_id = draft::load_timeline(&output).unwrap()["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let prefix = &segment_id[..8];
    let first = run_ok(&[
        "timeline",
        "audio-fade",
        &output.to_string_lossy(),
        prefix,
        "--in",
        "0.5",
        "--fade-out",
        "1.0",
        "--json",
    ]);
    assert_eq!(first["segmentId"], segment_id);
    assert_eq!(first["fade_in_us"], 500_000);
    assert_eq!(first["fade_out_us"], 1_000_000);
    let first_id = first["fade_id"].as_str().unwrap().to_owned();
    let second = run_ok(&[
        "timeline",
        "audio-fade",
        &output.to_string_lossy(),
        prefix,
        "--in",
        "0.25",
        "--json",
    ]);
    let timeline = draft::load_timeline(&output).unwrap();
    let refs = timeline["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array()
        .unwrap();
    assert!(!refs
        .iter()
        .any(|item| item.as_str() == Some(first_id.as_str())));
    assert!(refs.iter().any(|item| item == &second["fade_id"]));
    assert_eq!(
        timeline["materials"]["audio_fades"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_cover_matches_capcut_wire_and_rejects_missing_images() {
    let root = std::env::temp_dir().join(format!("jianying-cover-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let output = root.join("draft");
    run_ok(&[
        "project",
        "init",
        "cover",
        "--out",
        &output.to_string_lossy(),
        "--json",
    ]);
    let image = root.join("cover.png");
    std::fs::write(&image, b"fixture-image").unwrap();

    let applied = run_ok(&[
        "project",
        "add-cover",
        &output.to_string_lossy(),
        &image.to_string_lossy(),
        "--time",
        "1500",
        "--json",
    ]);
    assert_eq!(applied["cover_path"], image.to_string_lossy().as_ref());
    assert_eq!(applied["time_ms"], 1500);
    let timeline = draft::load_timeline(&output).unwrap();
    assert_eq!(timeline["cover"]["path"], image.to_string_lossy().as_ref());
    assert_eq!(timeline["cover"]["type"], "image");
    assert_eq!(timeline["cover"]["time"], 1500);
    assert_eq!(timeline["cover"]["time_ms"], 1500);
    assert!(timeline["cover"]["custom_cover_id"].is_string());

    run_ok(&[
        "project",
        "add-cover",
        &output.to_string_lossy(),
        &image.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(
        draft::load_timeline(&output).unwrap()["cover"]["time_ms"],
        0
    );
    let unchanged = std::fs::read(output.join("draft_content.json")).unwrap();

    let missing = run(&[
        "project",
        "add-cover",
        &output.to_string_lossy(),
        &root.join("missing.png").to_string_lossy(),
        "--json",
    ]);
    assert_ne!(missing.status.code(), Some(0));
    let negative = run(&[
        "project",
        "add-cover",
        &output.to_string_lossy(),
        &image.to_string_lossy(),
        "--time=-1",
        "--json",
    ]);
    assert_ne!(negative.status.code(), Some(0));
    assert_eq!(
        std::fs::read(output.join("draft_content.json")).unwrap(),
        unchanged
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn add_filter_supports_catalogue_raw_resource_intensity_and_full_range() {
    let root = std::env::temp_dir().join(format!("jianying-filter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("duration.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:02,000\nduration\n").unwrap();
    let output = root.join("draft");
    let output_string = output.to_string_lossy().into_owned();
    run_ok(&[
        "project",
        "quickstart",
        "filter",
        "--out",
        &output.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);

    let added = run_ok(&[
        "timeline",
        "add-filter",
        &output.to_string_lossy(),
        "vintage",
        "0s",
        "1s",
        "--intensity",
        "0.4",
        "--json",
    ]);
    assert_eq!(added["name"], "Vintage");
    assert_eq!(added["start_us"], 0);
    assert_eq!(added["duration_us"], 1_000_000);
    let timeline = draft::load_timeline(&output).unwrap();
    let material = timeline["materials"]["video_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == added["materialId"])
        .unwrap();
    assert_eq!(material["type"], "filter");
    assert_eq!(material["effect_id"], "7028463716732079117");
    assert_eq!(material["value"], 0.4);
    assert_eq!(material["source_platform"], 0);

    let raw = run_ok(&[
        "timeline",
        "add-filter",
        &output.to_string_lossy(),
        "Store Look",
        "0s",
        "500ms",
        "--resource-id",
        "7529669127365202194",
        "--effect-id",
        "2222222222",
        "--json",
    ]);
    let full = run_ok(&[
        "timeline",
        "add-filter",
        &output.to_string_lossy(),
        "warm",
        "--full",
        "--json",
    ]);
    assert_eq!(full["duration_us"], 2_000_000);
    let timeline = draft::load_timeline(&output).unwrap();
    let raw_material = timeline["materials"]["video_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == raw["materialId"])
        .unwrap();
    assert_eq!(raw_material["resource_id"], "7529669127365202194");
    assert_eq!(raw_material["effect_id"], "2222222222");
    assert_eq!(raw_material["source_platform"], 1);

    for invalid in [vec!["--effect-id", "orphan"], vec!["--intensity", "1.5"]] {
        let mut args = vec![
            "timeline",
            "add-filter",
            &output_string,
            "vintage",
            "0s",
            "1s",
        ];
        args.extend(invalid);
        args.push("--json");
        assert_ne!(run(&args).status.code(), Some(0));
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn add_effect_supports_catalogue_params_raw_ids_binding_and_full_range() {
    let root = std::env::temp_dir().join(format!("jianying-effect-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("duration.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:02,000\nduration\n").unwrap();
    let output = root.join("draft");
    let output_string = output.to_string_lossy().into_owned();
    run_ok(&[
        "project",
        "quickstart",
        "effect",
        "--out",
        &output_string,
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let bound_id = draft::load_timeline(&output).unwrap()["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let added = run_ok(&[
        "timeline",
        "add-effect",
        &output_string,
        "shake",
        "0s",
        "1s",
        "--params",
        "[0.25,0.75]",
        "--intensity",
        "0.7",
        "--bind",
        &bound_id[..8],
        "--json",
    ]);
    assert_eq!(added["name"], "Shake");
    let timeline = draft::load_timeline(&output).unwrap();
    let material = timeline["materials"]["video_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == added["materialId"])
        .unwrap();
    assert_eq!(material["type"], "video_effect");
    assert_eq!(material["apply_target_type"], 0);
    assert_eq!(material["bind_segment_id"], bound_id);
    assert_eq!(material["value"], 0.7);
    assert_eq!(material["adjust_params"][1]["name"], "param_1");

    let raw = run_ok(&[
        "timeline",
        "add-effect",
        &output_string,
        "Store Effect",
        "0s",
        "500ms",
        "--resource-id",
        "7529669127365202194",
        "--effect-id",
        "2222222222",
        "--json",
    ]);
    let full = run_ok(&[
        "timeline",
        "add-effect",
        &output_string,
        "vhs",
        "--full",
        "--json",
    ]);
    assert_eq!(full["duration_us"], 2_000_000);
    let timeline = draft::load_timeline(&output).unwrap();
    let raw_material = timeline["materials"]["video_effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == raw["materialId"])
        .unwrap();
    assert_eq!(raw_material["resource_id"], "7529669127365202194");
    assert_eq!(raw_material["effect_id"], "2222222222");
    assert_eq!(raw_material["source_platform"], 1);
    assert_eq!(
        timeline["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|track| track["type"] == "effect")
            .count(),
        1
    );

    let missing_bind = run(&[
        "timeline",
        "add-effect",
        &output_string,
        "shake",
        "0s",
        "1s",
        "--bind",
        "missing",
        "--json",
    ]);
    assert_ne!(missing_bind.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn crop_reads_and_writes_video_material_with_ratio_rect_reset_and_dry_run() {
    let root = std::env::temp_dir().join(format!("jianying-crop-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("video.mp4"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"crop",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"video","segments":[{
            "start_us":0,"duration_us":2_000_000,"source":"video.mp4"
        }]}]
    }))
    .unwrap();
    let output = root.join("draft");
    draft::build(&plan, &root, &output, None, &probe_stub).unwrap();
    let output_string = output.to_string_lossy().into_owned();
    let segment_id = draft::load_timeline(&output).unwrap()["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let prefix = &segment_id[..8];
    let before = std::fs::read(output.join("draft_content.json")).unwrap();
    let read = run_ok(&["timeline", "crop", &output_string, prefix, "--json"]);
    assert_eq!(read["segmentId"], segment_id);
    assert_eq!(read["width"], 1920);
    assert_eq!(read["height"], 1080);
    assert!(read["crop"].is_object());
    assert_eq!(
        std::fs::read(output.join("draft_content.json")).unwrap(),
        before
    );

    let ratio = run_ok(&[
        "timeline",
        "crop",
        &output_string,
        prefix,
        "--ratio",
        "9:16",
        "--json",
    ]);
    assert_eq!(ratio["rect"]["x"], 0.341796875);
    assert_eq!(ratio["rect"]["w"], 0.31640625);
    let rect = run_ok(&[
        "timeline",
        "crop",
        &output_string,
        prefix,
        "--ratio",
        "1:1",
        "--rect",
        "0,0,0.5,0.5",
        "--json",
    ]);
    assert_eq!(
        rect["rect"],
        serde_json::json!({"x":0.0,"y":0.0,"w":0.5,"h":0.5})
    );
    let committed = std::fs::read(output.join("draft_content.json")).unwrap();
    let preview = run_ok(&[
        "timeline",
        "crop",
        &output_string,
        prefix,
        "--rect",
        "0.25,0.25,0.5,0.5",
        "--dry-run",
        "--json",
    ]);
    assert_eq!(preview["dryRun"], true);
    assert_eq!(
        std::fs::read(output.join("draft_content.json")).unwrap(),
        committed
    );
    let reset = run_ok(&[
        "timeline",
        "crop",
        &output_string,
        prefix,
        "--reset",
        "--json",
    ]);
    assert_eq!(
        reset["rect"],
        serde_json::json!({"x":0.0,"y":0.0,"w":1.0,"h":1.0})
    );
    let invalid = run(&[
        "timeline",
        "crop",
        &output_string,
        prefix,
        "--rect",
        "0.6,0,0.5,1",
        "--json",
    ]);
    assert_ne!(invalid.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_cut_extracts_a_rebased_standalone_timeline_without_mutating_source() {
    let root = std::env::temp_dir().join(format!("jianying-cut-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:01,000\none\n\n\
         2\n00:00:01,000 --> 00:00:02,000\ntwo\n\n\
         3\n00:00:02,000 --> 00:00:03,000\nthree\n",
    )
    .unwrap();
    let source = root.join("source");
    run_ok(&[
        "project",
        "quickstart",
        "cut-source",
        "--out",
        &source.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let source_before = std::fs::read(source.join("draft_content.json")).unwrap();
    let output = root.join("cut.json");
    let result = run_ok(&[
        "project",
        "cut",
        &source.to_string_lossy(),
        "500ms",
        "1500ms",
        "--out",
        &output.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(result["kept"], 2);
    assert_eq!(result["removed"], 1);
    assert_eq!(result["duration_us"], 1_000_000);
    assert_eq!(
        std::fs::read(source.join("draft_content.json")).unwrap(),
        source_before
    );
    let cut: serde_json::Value = serde_json::from_slice(&std::fs::read(&output).unwrap()).unwrap();
    assert_eq!(cut["duration"], 1_000_000);
    assert_eq!(cut["tracks"][0]["segments"].as_array().unwrap().len(), 2);
    assert_eq!(
        cut["tracks"][0]["segments"][0]["target_timerange"],
        serde_json::json!({"start":0,"duration":500000})
    );
    assert_eq!(
        cut["tracks"][0]["segments"][1]["target_timerange"],
        serde_json::json!({"start":500000,"duration":500000})
    );
    assert_eq!(cut["materials"]["texts"].as_array().unwrap().len(), 2);

    let invalid = run(&[
        "project",
        "cut",
        &source.to_string_lossy(),
        "2s",
        "1s",
        "--out",
        &output.to_string_lossy(),
        "--json",
    ]);
    assert_ne!(invalid.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn keyframe_supports_aliases_easing_and_batch_updates() {
    let root = std::env::temp_dir().join(format!("jianying-keyframe-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:06,000\nhello\n").unwrap();
    let draft_dir = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "keyframes",
        "--out",
        &draft_dir.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let id = run_ok(&[
        "timeline",
        "segments",
        &draft_dir.to_string_lossy(),
        "--json",
    ])[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let first = run_ok(&[
        "timeline",
        "keyframe",
        &draft_dir.to_string_lossy(),
        &id[..8],
        "scale",
        "0s",
        "1.0",
        "--easing",
        "ease-out",
        "--json",
    ]);
    assert_eq!(first["added"], 1);
    assert!(first["warnings"].as_array().is_some());
    let second = run_ok(&[
        "timeline",
        "keyframe",
        &draft_dir.to_string_lossy(),
        &id[..8],
        "uniform_scale",
        "5s",
        "1.3",
        "--easing",
        "ease-out",
        "--json",
    ]);
    assert_eq!(
        second["lists"][0],
        serde_json::json!({"property":"uniform_scale","count":2})
    );
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    let list = &timeline["tracks"][0]["segments"][0]["common_keyframes"][0];
    assert_eq!(list["property_type"], "UNIFORM_SCALE");
    assert_eq!(
        list["keyframe_list"][0]["right_control"],
        serde_json::json!({"x":1600000,"y":0.282})
    );
    assert_eq!(
        list["keyframe_list"][1]["left_control"],
        serde_json::json!({"x":-2000000,"y":0.0})
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn transition_supports_capcut_and_jianying_catalogues_and_rejects_stacking() {
    let root = std::env::temp_dir().join(format!("jianying-transition-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:01,000\none\n\n2\n00:00:01,000 --> 00:00:02,000\ntwo\n",
    )
    .unwrap();
    let draft_dir = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "transitions",
        "--out",
        &draft_dir.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let segments = run_ok(&[
        "timeline",
        "segments",
        &draft_dir.to_string_lossy(),
        "--json",
    ]);
    let first = segments[0]["id"].as_str().unwrap();
    let second = segments[1]["id"].as_str().unwrap();
    let capcut = run_ok(&[
        "timeline",
        "transition",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "dissolve",
        "--json",
    ]);
    assert_eq!(capcut["name"], "Dissolve");
    assert_eq!(capcut["duration_us"], 466666);
    let jianying = run_ok(&[
        "timeline",
        "transition",
        &draft_dir.to_string_lossy(),
        &second[..8],
        "_3D空间",
        "--duration",
        "750ms",
        "--jianying",
        "--json",
    ]);
    assert_eq!(jianying["name"], "3D空间");
    assert_eq!(jianying["duration_us"], 750000);
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    assert_eq!(
        timeline["materials"]["transitions"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        timeline["materials"]["transitions"][0]["effect_id"],
        "392FD26E-A514-4d0f-8950-EA4A20CB407C"
    );
    assert_eq!(
        timeline["materials"]["transitions"][1]["resource_id"],
        "7049979667406656014"
    );
    let stacked = run(&[
        "timeline",
        "transition",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "mix",
        "--json",
    ]);
    assert_ne!(stacked.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn text_animation_supports_dual_catalogues_anchors_and_container_reuse() {
    let root = std::env::temp_dir().join(format!("jianying-text-anim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:03,000\none\n\n2\n00:00:03,000 --> 00:00:06,000\ntwo\n",
    )
    .unwrap();
    let draft_dir = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "text-animations",
        "--out",
        &draft_dir.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let segments = run_ok(&[
        "timeline",
        "segments",
        &draft_dir.to_string_lossy(),
        "--json",
    ]);
    let first = segments[0]["id"].as_str().unwrap();
    let second = segments[1]["id"].as_str().unwrap();
    let capcut = run_ok(&[
        "captions",
        "animation",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "--intro",
        "typewriter",
        "--outro",
        "fade-out",
        "--outro-duration",
        "750ms",
        "--json",
    ]);
    assert_eq!(capcut["added"][0]["start_us"], 0);
    assert_eq!(capcut["added"][1]["start_us"], 2_250_000);
    let jianying = run_ok(&[
        "captions",
        "animation",
        &draft_dir.to_string_lossy(),
        &second[..8],
        "--intro",
        "卡拉OK",
        "--intro-duration",
        "600ms",
        "--jianying",
        "--json",
    ]);
    assert_eq!(jianying["added"][0]["duration_us"], 600000);
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    assert_eq!(
        timeline["materials"]["material_animations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        timeline["materials"]["material_animations"][0]["animations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let duplicate = run(&[
        "captions",
        "animation",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "--intro",
        "throw-out",
        "--json",
    ]);
    assert_ne!(duplicate.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn image_animation_supports_intro_outro_combo_and_dual_catalogues() {
    let root = std::env::temp_dir().join(format!("jianying-image-anim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("video.mp4"), b"fixture").unwrap();
    let plan: Plan = serde_json::from_value(serde_json::json!({
        "schema":"jianying-cli-plan/v1","name":"image-animations",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2_000_000,"source":"video.mp4"},
            {"start_us":2_000_000,"duration_us":2_000_000,"source":"video.mp4"}
        ]}]
    }))
    .unwrap();
    let draft_dir = root.join("draft");
    draft::build(&plan, &root, &draft_dir, None, &probe_stub).unwrap();
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    let first = timeline["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let second = timeline["tracks"][0]["segments"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let capcut = run_ok(&[
        "timeline",
        "image-animation",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "--intro",
        "fade-in",
        "--outro",
        "blur-out",
        "--outro-duration",
        "600ms",
        "--combo",
        "distort-and-stretch",
        "--combo-duration",
        "700ms",
        "--json",
    ]);
    assert_eq!(capcut["added"][0]["start_us"], 0);
    assert_eq!(capcut["added"][1]["start_us"], 1_400_000);
    assert_eq!(capcut["added"][2]["type"], "group");
    let jianying = run_ok(&[
        "timeline",
        "image-animation",
        &draft_dir.to_string_lossy(),
        &second[..8],
        "--intro",
        "缩小",
        "--combo",
        "三分割",
        "--jianying",
        "--json",
    ]);
    assert_eq!(jianying["added"][0]["duration_us"], 500_000);
    assert_eq!(jianying["added"][1]["type"], "group");
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    assert_eq!(
        timeline["materials"]["material_animations"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        timeline["materials"]["material_animations"][0]["animations"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        timeline["materials"]["material_animations"][0]["animations"][0]["material_type"],
        "video"
    );
    let duplicate = run(&[
        "timeline",
        "image-animation",
        &draft_dir.to_string_lossy(),
        &first[..8],
        "--intro",
        "flash-in",
        "--json",
    ]);
    assert_ne!(duplicate.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn sticker_add_reuses_named_tracks_and_writes_companion_materials() {
    let root = std::env::temp_dir().join(format!("jianying-sticker-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("captions.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:03,000\nbase\n").unwrap();
    let draft_dir = root.join("draft");
    run_ok(&[
        "project",
        "quickstart",
        "stickers",
        "--out",
        &draft_dir.to_string_lossy(),
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]);
    let first = run_ok(&[
        "media",
        "add-sticker",
        &draft_dir.to_string_lossy(),
        "7520000000000000001",
        "250ms",
        "1.5s",
        "--x",
        "0.25",
        "--y",
        "-0.5",
        "--scale",
        "1.2",
        "--rotation",
        "15",
        "--json",
    ]);
    let second = run_ok(&[
        "media",
        "add-sticker",
        &draft_dir.to_string_lossy(),
        "7520000000000000002",
        "2s",
        "500ms",
        "--json",
    ]);
    assert_eq!(first["trackId"], second["trackId"]);
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    let track = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "sticker")
        .unwrap();
    assert_eq!(track["name"], "sticker");
    assert_eq!(track["is_default_name"], true);
    assert_eq!(track["segments"].as_array().unwrap().len(), 2);
    assert_eq!(track["segments"][0]["clip"]["transform"]["x"], 0.25);
    assert_eq!(track["segments"][0]["clip"]["transform"]["y"], -0.5);
    assert_eq!(track["segments"][0]["clip"]["scale"]["x"], 1.2);
    assert_eq!(track["segments"][0]["clip"]["rotation"], 15.0);
    assert_eq!(
        track["segments"][0]["extra_material_refs"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(
        timeline["materials"]["stickers"][0]["resource_id"],
        "7520000000000000001"
    );
    assert_eq!(timeline["materials"]["speeds"].as_array().unwrap().len(), 2);
    assert_eq!(
        timeline["materials"]["placeholder_infos"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        timeline["materials"]["canvases"].as_array().unwrap().len(),
        2
    );
    let _ = std::fs::remove_dir_all(root);
}

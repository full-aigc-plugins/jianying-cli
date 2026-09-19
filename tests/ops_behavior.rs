//! Store / template / verify-guard / srt behavior tests (filesystem-level,
//! temp directories only — never touches the real 剪映 draft store).

use anyhow::Result;
use jianying_cli::{draft, plan::Plan, srt, store, template};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn probe_stub(_p: &Path) -> Result<jianying_cli::probe::MediaInfo> {
    Ok(jianying_cli::probe::MediaInfo {
        path: _p.to_string_lossy().into_owned(),
        duration_us: 10_000_000,
        width: 640,
        height: 360,
        has_video: true,
        has_audio: true,
        is_image: false,
    })
}

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("jyc-ops-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn build_simple(dir: &Path, name: &str) -> PathBuf {
    std::fs::write(dir.join("a.mp4"), b"fake media bytes").unwrap();
    let plan: Plan = serde_json::from_value(json!({
        "schema": "jianying-cli-plan/v1", "name": name,
        "canvas": {"width": 1280, "height": 720, "fps": 30},
        "tracks": [
            {"type": "video", "segments": [
                {"start_us": 0, "duration_us": 2_000_000, "source": "a.mp4"}]},
            {"type": "text", "segments": [
                {"start_us": 100_000, "duration_us": 1_000_000, "text": "你好世界"}]}]
    }))
    .unwrap();
    let out = dir.join(name);
    draft::build(&plan, dir, &out, None, &probe_stub).unwrap();
    out
}

#[test]
fn srt_parser_edges() {
    // CRLF, no index line, multiline text, out-of-range timestamps
    let cues = srt::parse(
        "1\r\n00:00:00,000 --> 00:00:01,000\r\n第一句\r\n\r\n00:00:02,000 --> 00:00:03,500\r\n第二句\r\n多行\r\n",
    )
    .unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[1].start_us, 2_000_000);
    assert_eq!(cues[1].end_us, 3_500_000);
    assert_eq!(cues[1].text, "第二句\n多行");
    assert!(srt::parse("1\n00:00:05,000 --> 00:00:03,000\n倒序\n").is_err());
    assert!(srt::parse("1\nbad --> line\nx\n").is_err());
    // SRT timestamp via tim() parity
    assert_eq!(
        jianying_cli::tim::parse("01:02:03,400").unwrap(),
        3_723_400_000
    );
}

#[test]
fn store_list_has_remove_roundtrip() {
    let dir = tmpdir("store");
    let built = build_simple(&dir, "s1");
    let root = dir.join("root");
    std::fs::create_dir_all(&root).unwrap();
    store::publish(&built, &root, true).unwrap();

    let listed = store::list(&root).unwrap();
    assert_eq!(listed["drafts"], json!(["s1"]));
    assert_eq!(store::has(&root, "s1").unwrap()["exists"], json!(true));
    assert_eq!(store::has(&root, "nope").unwrap()["exists"], json!(false));

    // remove unregisters from root_meta_info
    store::remove(&root, "s1").unwrap();
    assert_eq!(store::list(&root).unwrap()["drafts"], json!([]));
    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("root_meta_info.json")).unwrap())
            .unwrap();
    assert!(meta["all_draft_store"].as_array().unwrap().is_empty());
    assert!(!root.join("s1").exists());
    // removing again fails
    assert!(store::remove(&root, "s1").is_err());
}

#[test]
fn template_duplicate_replace_import() {
    let dir = tmpdir("tpl");
    let built = build_simple(&dir, "t1");

    // duplicate restamps ids and names
    template::duplicate(&built, "t2", None).unwrap();
    let d2 = dir.join("t2");
    let tl2: Value =
        serde_json::from_str(&std::fs::read_to_string(d2.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(tl2["name"], json!("t2"));
    let meta2: Value =
        serde_json::from_str(&std::fs::read_to_string(d2.join("draft_meta_info.json")).unwrap())
            .unwrap();
    assert_eq!(meta2["draft_name"], json!("t2"));

    // inspect reports tracks and named materials
    let inv = template::inspect_materials(&d2).unwrap();
    assert_eq!(inv["tracks"].as_array().unwrap().len(), 2);

    // replace_text recomputes the UTF-16 range
    template::replace_text(&d2, "text", 0, "替换后的文案内容").unwrap();
    let tl: Value =
        serde_json::from_str(&std::fs::read_to_string(d2.join("draft_content.json")).unwrap())
            .unwrap();
    let mat = &tl["materials"]["texts"][0];
    let content: Value = serde_json::from_str(mat["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["text"], json!("替换后的文案内容"));
    assert_eq!(content["styles"][0]["range"], json!([0, 8]));

    // import_track: same-named track in the target is refused (name collision),
    // while a track type the target lacks imports cleanly with remapped ids
    assert!(template::import_track(&d2, &built, "text").is_err());
    let with_audio = dir.join("t3src");
    std::fs::create_dir_all(&with_audio).unwrap();
    let plan: Plan = serde_json::from_value(json!({
        "schema": "jianying-cli-plan/v1", "name": "t3src",
        "canvas": {"width": 1280, "height": 720, "fps": 30},
        "tracks": [{"type": "audio", "segments": [
            {"start_us": 0, "duration_us": 2_000_000, "source": "a.mp4"}]}]
    }))
    .unwrap();
    std::fs::write(with_audio.join("a.mp4"), b"fake media bytes").unwrap();
    draft::build(
        &plan,
        &with_audio,
        &with_audio.join("t3src"),
        None,
        &probe_stub,
    )
    .unwrap();
    let r = template::import_track(&d2, &with_audio.join("t3src"), "audio").unwrap();
    assert_eq!(r["status"], json!("imported"));
    let tl: Value =
        serde_json::from_str(&std::fs::read_to_string(d2.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(
        tl["tracks"].as_array().unwrap().len(),
        3,
        "video + text + imported audio"
    );
    // duration recomputed to the max segment end
    assert_eq!(tl["duration"], json!(2_000_000));
    // duplicate import of the same track name is refused
    assert!(template::import_track(&d2, &with_audio.join("t3src"), "audio").is_err());
}

#[test]
fn verify_catches_injected_corruption() {
    let dir = tmpdir("verify");
    let built = build_simple(&dir, "v1");
    let tl_path = built.join("draft_content.json");
    let original = std::fs::read_to_string(&tl_path).unwrap();

    let mut tl: Value = serde_json::from_str(&original).unwrap();
    // 1) dangling extra_material_ref
    tl["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array_mut()
        .unwrap()
        .push(json!("deadbeefdeadbeefdeadbeefdeadbeef"));
    std::fs::write(&tl_path, serde_json::to_string_pretty(&tl).unwrap()).unwrap();
    let v = draft::verify(&built).unwrap();
    assert_eq!(v["ok"], json!(false));
    assert!(v["issues"].to_string().contains("dangling"));

    // 2) overlapping segments
    let mut tl: Value = serde_json::from_str(&original).unwrap();
    tl["tracks"][0]["segments"][0]["target_timerange"]["start"] = json!(500_000);
    std::fs::write(&tl_path, serde_json::to_string_pretty(&tl).unwrap()).unwrap();
    assert_eq!(draft::verify(&built).unwrap()["ok"], json!(false));

    // 3) top duration lying about the timeline
    let mut tl: Value = serde_json::from_str(&original).unwrap();
    tl["duration"] = json!(9_999_999);
    std::fs::write(&tl_path, serde_json::to_string_pretty(&tl).unwrap()).unwrap();
    assert_eq!(draft::verify(&built).unwrap()["ok"], json!(false));

    std::fs::write(&tl_path, &original).unwrap();
    assert_eq!(draft::verify(&built).unwrap()["ok"], json!(true));
}

#[test]
fn mirrors_and_materials_registration() {
    let dir = tmpdir("mirrors");
    let built = build_simple(&dir, "m1");
    let content = std::fs::read_to_string(built.join("draft_content.json")).unwrap();
    let info = std::fs::read_to_string(built.join("draft_info.json")).unwrap();
    assert_eq!(content, info, "timeline mirrors must be byte-identical");
    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(built.join("draft_meta_info.json")).unwrap())
            .unwrap();
    let reg = meta["draft_materials"][0]["value"].as_array().unwrap();
    assert_eq!(reg.len(), 1, "video source registered in draft_materials");
    assert_eq!(reg[0]["metetype"], json!("video"));
    // uppercase-hyphenated draft id sidecar vs hex timeline id
    assert!(meta["draft_id"].as_str().unwrap().contains('-'));
}

use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo, srt, store};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const FULLCAP_PLAN: &str = r##"
{
  "schema": "jianying-cli-plan/v1",
  "name": "integration-draft",
  "canvas": {"width": 1920, "height": 1080, "fps": 30},
  "tracks": [
    {"type": "video", "segments": [
      {"start_us": 0, "duration_us": 2000000, "source": "shot-a.mp4",
       "mask": {"name": "圆形", "size": 0.6, "feather": 20},
       "filters": [{"name": "1980", "intensity": 60}],
       "effects": [{"name": "1998"}],
       "mix_mode": "正片叠底",
       "animation_in": {"name": "渐显"},
       "animation_out": {"name": "渐隐"},
       "fade": {"in_us": 250000, "out_us": 250000},
       "chroma": {"color": "#00FF00", "intensity": 30, "edge_smooth": 10},
       "background_filling": {"type": "blur", "blur": 0.375},
       "transition_out": {"name": "闪黑", "duration_us": 300000},
       "keyframes": {"scale": [{"at_us": 0, "value": 1.0}, {"at_us": 2000000, "value": 1.12}]}},
      {"start_us": 2000000, "duration_us": 2000000, "source": "shot-b.mp4"}
    ]},
    {"type": "audio", "segments": [
      {"start_us": 0, "duration_us": 4000000, "source": "narration.mp3", "volume": 0.8,
       "fade": {"in_us": 300000, "out_us": 500000},
       "audio_effects": [{"name": "8bit"}]}
    ]},
    {"type": "text", "segments": [
      {"start_us": 200000, "duration_us": 1500000, "text": "样式字幕测试",
       "size": 8, "color": "#FFFFFF", "border_width": 40,
       "font": "Alice-Regular",
       "background": {"color": "#FF0000"},
       "shadow": {"angle": -45},
       "animation_in": {"name": "冲屏位移"},
       "styles": [{"range": [0, 4], "color": "#FF0000"}],
       "bubble": {"effect_id": "123", "resource_id": "456"}}
    ]},
    {"type": "sticker", "segments": [
      {"start_us": 500000, "duration_us": 1000000,
       "resource_id": "7310000000000000002"}
    ]},
    {"type": "filter", "segments": [
      {"start_us": 0, "duration_us": 4000000, "filters": [{"name": "1980"}], "intensity": 40}
    ]},
    {"type": "effect", "segments": [
      {"start_us": 0, "duration_us": 4000000, "effects": [{"name": "70s"}]}
    ]}
  ]
}
"##;

fn probe_stub(_p: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: _p.to_string_lossy().into_owned(),
        duration_us: 10_000_000,
        width: 640,
        height: 360,
        has_video: true,
        has_audio: true,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn write_plan(dir: &Path) -> PathBuf {
    let p = dir.join("plan.json");
    std::fs::write(&p, FULLCAP_PLAN).unwrap();
    for name in ["shot-a.mp4", "shot-b.mp4", "narration.mp3"] {
        std::fs::write(dir.join(name), b"fake media bytes").unwrap();
    }
    p
}

#[test]
fn catalogs_have_expected_populations_and_vip_guard() {
    assert_eq!(
        jianying_cli::catalogs::transitions()
            .as_array()
            .unwrap()
            .len(),
        453
    );
    assert_eq!(
        jianying_cli::catalogs::filters().as_array().unwrap().len(),
        1052
    );
    assert_eq!(
        jianying_cli::catalogs::fonts().as_array().unwrap().len(),
        798
    );
    assert_eq!(jianying_cli::catalogs::masks().as_array().unwrap().len(), 6);
    let plan: Plan = serde_json::from_value(json!({
        "schema": "jianying-cli-plan/v1", "name": "x",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "video", "segments": [
            {"start_us": 0, "duration_us": 1000000, "source": "a.mp4",
             "transition_out": {"name": "叠化扭曲"}}]}]
    }))
    .unwrap();
    assert!(
        plan.validate().is_err(),
        "vip transition must be rejected without allow_vip"
    );
}

#[test]
fn srt_parser_handles_cjk_blocks() {
    let cues = srt::parse("1\n00:00:00,200 --> 00:00:01,500\n第一句\n\n2\n00:00:02,000 --> 00:00:03,400\n第二句\n多行\n").unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].start_us, 200_000);
    assert_eq!(cues[1].text, "第二句\n多行");
}

#[test]
fn build_full_capability_draft_and_self_verify() {
    let tmp = std::env::temp_dir().join(format!("jycli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let plan_path = write_plan(&tmp);
    let out = tmp.join("integration-draft");
    let plan = Plan::load(&plan_path).unwrap();
    let report = draft::build(&plan, &tmp, &out, None, &probe_stub).expect("build succeeds");
    assert_eq!(report.tracks, 6);

    let d: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("draft_content.json")).unwrap())
            .unwrap();
    let info: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("draft_info.json")).unwrap())
            .unwrap();
    assert_eq!(d, info, "timeline mirrors must be identical");
    let wire = jianying_cli::lossless_draft::DraftTimelineWire::from_value(d.clone()).unwrap();
    let resources = wire.resource_inventory().unwrap();
    wire.validate_edit_semantics().unwrap();
    for kind in [
        jianying_cli::lossless_draft::DraftResourceKind::Video,
        jianying_cli::lossless_draft::DraftResourceKind::Audio,
        jianying_cli::lossless_draft::DraftResourceKind::Text,
        jianying_cli::lossless_draft::DraftResourceKind::Sticker,
        jianying_cli::lossless_draft::DraftResourceKind::Filter,
        jianying_cli::lossless_draft::DraftResourceKind::Effect,
        jianying_cli::lossless_draft::DraftResourceKind::Transition,
        jianying_cli::lossless_draft::DraftResourceKind::Mask,
        jianying_cli::lossless_draft::DraftResourceKind::Animation,
        jianying_cli::lossless_draft::DraftResourceKind::Keyframe,
    ] {
        assert!(resources.count(kind) >= 1, "missing resource kind {kind:?}");
    }
    let m = &d["materials"];
    assert_eq!(m["masks"].as_array().unwrap().len(), 1);
    assert_eq!(
        m["effects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["type"] == "filter")
            .count(),
        2,
        "segment filter + global filter track"
    );
    assert_eq!(
        m["video_effects"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["apply_target_type"] == 2)
            .count(),
        1,
        "global effect track is apply_target_type=2"
    );
    assert_eq!(
        m["audio_fades"].as_array().unwrap().len(),
        2,
        "video fade + audio fade"
    );
    let chroma = &m["chromas"][0];
    assert_eq!(chroma["type"], json!("chroma"));
    assert_eq!(chroma["intensity_value"], json!(0.3));
    assert_eq!(
        chroma["id"].as_str().unwrap().len(),
        36,
        "chroma id is an uppercase hyphenated uuid (pyJYD quirk)"
    );
    let bgf = m["canvases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["type"] == "canvas_blur")
        .expect("background filling entry");
    assert_eq!(bgf["blur"], json!(0.375));
    let bubble = m["effects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["type"] == "text_shape")
        .expect("text bubble entry");
    assert_eq!(bubble["effect_id"], json!("123"));
    assert_eq!(m["audio_effects"].as_array().unwrap().len(), 1);
    let anim_items: Vec<&Value> = m["material_animations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|e| e["animations"].as_array().unwrap())
        .collect();
    assert_eq!(anim_items.len(), 3, "video in + video out + text in");
    assert_eq!(
        m["transitions"].as_array().unwrap()[0]["name"],
        json!("闪黑")
    );
    let meta: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("draft_meta_info.json")).unwrap())
            .unwrap();
    assert_eq!(
        meta["draft_materials"][0]["value"]
            .as_array()
            .unwrap()
            .len(),
        3
    );

    let verdict = draft::verify(&out).unwrap();
    assert_eq!(verdict["ok"], json!(true), "issues: {}", verdict["issues"]);
}

#[test]
fn publish_registers_into_root_meta() {
    let tmp = std::env::temp_dir().join(format!("jycli-pub-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let plan_path = write_plan(&tmp);
    let out = tmp.join("integration-draft");
    let plan = Plan::load(&plan_path).unwrap();
    draft::build(&plan, &tmp, &out, None, &probe_stub).unwrap();
    let root = tmp.join("store-root");
    std::fs::create_dir_all(&root).unwrap();
    let result = store::publish(&out, &root, true).unwrap();
    assert_eq!(result["status"], json!("published"));
    let root_meta: Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("root_meta_info.json")).unwrap())
            .unwrap();
    assert_eq!(root_meta["all_draft_store"].as_array().unwrap().len(), 1);
    assert!(
        store::publish(&out, &root, true).is_err(),
        "name clash must refuse"
    );
}

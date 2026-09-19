//! Draft assembly: `jianying-cli-plan/v1` → a real 剪映/CapCut draft directory.
//!
//! Full-capability writers per domain. Wire shapes verified against drafts
//! produced by GuanYixuan/pyJianYingDraft (Apache-2.0) and the write/registration
//! discipline of renezander030/capcut-cli (MIT). The NC fork (partme-ai/
//! jianying-headless) contributed contract facts only.

use crate::catalogs;
use crate::plan::{hex_rgb, KeyPoint, Plan, Segment};
use crate::probe::MediaInfo;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const CONTENT_TEMPLATE: &str = include_str!("../assets/draft_content_template.json");
pub const META_TEMPLATE: &str = include_str!("../assets/draft_meta_info.json");

#[derive(Serialize)]
pub struct BuildReport {
    pub name: String,
    pub duration_us: i64,
    pub tracks: usize,
    pub segments: usize,
    pub out_dir: String,
    pub seed_donor: Option<String>,
}

fn hex_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

fn uuid_upper() -> String {
    uuid::Uuid::new_v4().to_string().to_uppercase()
}

fn now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

fn canvas_ratio(width: u64, height: u64) -> &'static str {
    match (width, height) {
        (1920, 1080) => "16:9",
        (1080, 1920) => "9:16",
        (1080, 1080) => "1:1",
        (1440, 1080) => "4:3",
        (1080, 1440) => "3:4",
        _ => "original",
    }
}

fn host_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "mac"
    } else {
        "linux"
    }
}

fn resolve_source(plan_dir: &Path, src: &Path) -> PathBuf {
    if src.is_absolute() {
        src.to_path_buf()
    } else {
        plan_dir.join(src)
    }
}

/// Copy schema markers from the newest app-written draft in `root`
/// (capcut-cli discipline: pure-template drafts are rejected by CapCut 8.4+).
fn seed_from_donor(root: &Path, content: &mut Value) -> Option<String> {
    let dirs = std::fs::read_dir(root).ok()?;
    let mut best: Option<(i64, PathBuf, Value)> = None;
    for entry in dirs.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        for name in ["draft_info.json", "draft_content.json", "draft_meta_info.json"] {
            let p = dir.join(name);
            let Ok(raw) = std::fs::read_to_string(&p) else { continue };
            let Ok(v) = serde_json::from_str::<Value>(&raw) else { continue };
            let is_timeline = v["tracks"].is_array() && v["materials"].is_object();
            let has_markers = v["version"].is_number()
                || v["new_version"].is_string()
                || v["last_modified_platform"].is_object();
            if !(is_timeline && has_markers) {
                continue;
            }
            let mtime = std::fs::metadata(&p)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_micros() as i64)
                .unwrap_or(0);
            if best.as_ref().map(|(t, _, _)| mtime > *t).unwrap_or(true) {
                best = Some((mtime, p, v));
            }
            break;
        }
    }
    let (_, path, donor) = best?;
    for field in ["version", "new_version", "color_space", "last_modified_platform"] {
        if let Some(v) = donor.get(field) {
            content[field] = v.clone();
        }
    }
    Some(path.to_string_lossy().into_owned())
}

fn base_segment(material_id: &str, seg: &Segment, render_index: i64) -> Value {
    let speed = seg.speed.unwrap_or(1.0);
    let src_duration = seg
        .source_duration_us
        .unwrap_or((seg.duration_us as f64 * speed).round() as i64);
    json!({
        "id": hex_id(),
        "material_id": material_id,
        "target_timerange": {"start": seg.start_us, "duration": seg.duration_us},
        "source_timerange": {"start": seg.source_start_us, "duration": src_duration},
        "speed": speed,
        "volume": seg.volume.unwrap_or(1.0),
        "visible": true,
        "reverse": false,
        "enable_adjust": true,
        "enable_color_correct_adjust": false,
        "enable_color_curves": true,
        "enable_color_match_adjust": false,
        "enable_color_wheels": true,
        "enable_lut": true,
        "enable_smart_color_adjust": false,
        "last_nonzero_volume": 1.0,
        "dur": seg.duration_us,
        "render_index": render_index,
        "track_render_index": 0,
        "track_attribute": 0,
        "extra_material_refs": [],
        "common_keyframes": [],
        "keyframe_refs": [],
    })
}

fn apply_visuals(seg_v: &mut Value, seg: &Segment, video: bool) {
    seg_v["clip"] = json!({
        "alpha": seg.opacity.unwrap_or(1.0),
        "rotation": seg.rotation.unwrap_or(0.0),
        "scale": {"x": seg.scale.unwrap_or(1.0), "y": seg.scale.unwrap_or(1.0)},
        "transform": {"x": seg.x.unwrap_or(0.0), "y": seg.y.unwrap_or(0.0)},
        "flip": {"horizontal": false, "vertical": false}
    });
    // visual segments (video/sticker/text) carry uniform_scale; non-uniform
    // keyframes turn it off (pyJianYingDraft authority)
    let has_scale_kf = seg
        .keyframes
        .as_ref()
        .map(|k| k.contains_key("scale"))
        .unwrap_or(false);
    if video || seg.text.is_some() || seg.sticker_id.is_some() || seg.resource_id.is_some() {
        seg_v["uniform_scale"] = json!({"on": !has_scale_kf, "value": 1.0});
    }
    if video {
        seg_v["hdr_settings"] = json!({"intensity": 1.0, "mode": 1, "nits": 1000});
    }
}

fn apply_keyframes(seg_v: &mut Value, seg: &Segment) {
    let Some(kfs) = &seg.keyframes else { return };
    if kfs.is_empty() {
        return;
    }
    let points_json = |points: &[KeyPoint]| -> Value {
        json!(points
            .iter()
            .map(|p| json!({
                "id": hex_id(),
                "curveType": "Line",
                "graphID": "",
                "left_control": {"x": 0.0, "y": 0.0},
                "right_control": {"x": 0.0, "y": 0.0},
                "time_offset": p.at_us,
                "values": [p.value]
            }))
            .collect::<Vec<_>>())
    };
    let mut push = |property: &str, points: &[KeyPoint]| {
        seg_v["common_keyframes"].as_array_mut().unwrap().push(json!({
            "id": hex_id(),
            "property_type": property,
            "keyframe_list": points_json(points),
            "material_id": ""
        }));
    };
    for (channel, points) in kfs {
        match channel.as_str() {
            "scale" => push("KFTypeScaleX", points),
            "x" => push("KFTypePositionX", points),
            "y" => push("KFTypePositionY", points),
            "rotation" => push("KFTypeRotation", points),
            "opacity" => push("KFTypeAlpha", points),
            "volume" => push("KFTypeVolume", points),
            other => debug_assert!(false, "validated channel slipped through: {other}"),
        }
    }
}

fn make_companions(materials: &mut Value, speed: f64, video: bool) -> Vec<String> {
    let speed_id = hex_id();
    materials["speeds"].as_array_mut().unwrap().push(json!({
        "id": speed_id, "type": "speed", "speed": speed, "mode": 0, "curve_speed": null
    }));
    let placeholder_id = hex_id();
    materials["placeholders"].as_array_mut().unwrap().push(json!({
        "id": placeholder_id, "type": "placeholder_info", "error_path": "", "error_text": "",
        "meta_type": "none", "res_path": "", "res_text": ""
    }));
    let scm_id = hex_id();
    materials["sound_channel_mappings"].as_array_mut().unwrap().push(json!({
        "id": scm_id, "type": "none", "audio_channel_mapping": 0, "is_config_open": false
    }));
    let vocal_id = hex_id();
    materials["vocal_separations"].as_array_mut().unwrap().push(json!({
        "id": vocal_id, "type": "vocal_separation", "choice": 0, "enter_from": "",
        "final_algorithm": "", "production_path": "", "removed_sounds": [], "time_range": null
    }));
    let mut refs = vec![speed_id, placeholder_id, scm_id, vocal_id];
    if video {
        let canvas_id = hex_id();
        materials["canvases"].as_array_mut().unwrap().push(json!({
            "id": canvas_id, "type": "canvas_color", "album_image": "", "blur": 0, "color": "",
            "image": "", "image_id": "", "image_name": "", "source_platform": 0, "team_id": ""
        }));
        let color_id = hex_id();
        materials["material_colors"].as_array_mut().unwrap().push(json!({
            "id": color_id, "type": "material_color", "gradient_angle": 90,
            "gradient_colors": [], "gradient_percents": [], "height": 0,
            "is_color_clip": false, "is_gradient": false, "solid_color": "", "width": 0
        }));
        refs.push(canvas_id);
        refs.push(color_id);
    }
    refs
}

fn copy_asset(out_dir: &Path, kind: &str, src: &Path) -> Result<String> {
    let dir = out_dir.join("assets").join(kind);
    std::fs::create_dir_all(&dir)?;
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "media.bin".to_string());
    let mut dest = dir.join(&name);
    let mut n = 0;
    let src_len = std::fs::metadata(src)?.len();
    while dest.exists() && std::fs::metadata(&dest)?.len() != src_len {
        n += 1;
        let stem = Path::new(&name)
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "media".to_string());
        let ext = Path::new(&name)
            .extension()
            .map(|e| format!(".{}", e.to_string_lossy()))
            .unwrap_or_default();
        dest = dir.join(format!("{stem}_{n}{ext}"));
    }
    if !dest.exists() {
        std::fs::copy(src, &dest)
            .with_context(|| format!("copying {} into the draft", src.display()))?;
    }
    Ok(dest.to_string_lossy().into_owned())
}

fn build_text_content(seg: &Segment, font_entry: Option<&Value>) -> Result<String> {
    let text = seg.text.as_deref().unwrap_or_default();
    let utf16_len: i64 = text.encode_utf16().count() as i64;
    let base_color = hex_rgb(seg.color.as_deref().unwrap_or("#FFFFFF"))?;
    let size = seg.size.unwrap_or(8.0);

    let style_for = |range: [i64; 2],
                     size: f64,
                     bold: bool,
                     italic: bool,
                     underline: bool,
                     color: [f64; 3]| -> Value {
        let mut s = json!({
            "range": range,
            "size": size,
            "bold": bold,
            "italic": italic,
            "underline": underline,
            "fill": {"alpha": 1.0, "content": {"render_type": "solid",
                     "solid": {"alpha": 1.0, "color": color}}},
        });
        if let Some(f) = font_entry {
            s["font"] = json!({"id": f["resource_id"], "path": "D:"});
        }
        s
    };

    let mut styles: Vec<Value> = Vec::new();
    styles.push(style_for(
        [0, utf16_len],
        size,
        seg.bold.unwrap_or(false),
        seg.italic.unwrap_or(false),
        seg.underline.unwrap_or(false),
        base_color,
    ));
    for r in &seg.styles {
        styles.push(style_for(
            [r.range[0] as i64, r.range[1] as i64],
            r.size.unwrap_or(size),
            r.bold.unwrap_or(false),
            r.italic.unwrap_or(false),
            r.underline.unwrap_or(false),
            hex_rgb(r.color.as_deref().unwrap_or("#FFFFFF"))?,
        ));
    }
    let border = seg.border_width.unwrap_or(0.0);
    if border > 0.0 {
        let bc = hex_rgb(seg.border_color.as_deref().unwrap_or("#000000"))?;
        styles[0]["strokes"] = json!([{"content": {"solid": {"alpha": 1.0, "color": bc}},
                                       "width": border / 100.0 * 0.2}]);
    }
    // pyJYD emits an empty strokes list on every style
    for s in styles.iter_mut() {
        if s.get("strokes").is_none() {
            s["strokes"] = json!([]);
        }
    }
    if let Some(sh) = &seg.shadow {
        let sc = hex_rgb(sh.color.as_deref().unwrap_or("#000000"))?;
        styles[0]["shadows"] = json!([{
            "content": {"solid": {"color": sc}},
            "diffuse": (sh.diffuse.unwrap_or(15.0) / 100.0) / 6.0,
            "alpha": sh.alpha.unwrap_or(1.0),
            "distance": sh.distance.unwrap_or(5.0),
            "angle": sh.angle.unwrap_or(-45.0),
        }]);
    }
    Ok(json!({"text": text, "styles": styles}).to_string())
}

/// VIP is already adjudicated by plan.validate (allow_vip); builders resolve
/// with vip allowed so the plan-level guard is the single authorization point.
fn vip_ok() -> bool {
    true
}

fn mask_entry(materials: &mut Value, mask: &crate::plan::Mask, info: &MediaInfo) -> Result<String> {
    let entry = catalogs::resolve(catalogs::masks(), "mask", &mask.name, vip_ok())?;
    let aspect = entry["default_aspect"].as_f64().unwrap_or(1.0);
    let shape = entry["shape"].as_str().unwrap_or("circle");
    let size = if mask.size != 0.0 { mask.size } else { 0.5 };
    let width = mask
        .rect_width
        .unwrap_or(size * info.height.max(1) as f64 * aspect / info.width.max(1) as f64);
    let id = hex_id();
    materials["masks"].as_array_mut().unwrap().push(json!({
        "config": {
            "aspectRatio": aspect,
            "centerX": mask.center_x,
            "centerY": mask.center_y,
            "feather": mask.feather / 100.0,
            "height": size,
            "invert": mask.invert,
            "rotation": mask.rotation,
            "roundCorner": mask.round_corner.unwrap_or(0.0) / 100.0,
            "width": width
        },
        "id": id,
        "name": mask.name,
        "platform": "all",
        "position_info": "",
        "resource_type": shape,
        "resource_id": entry["resource_id"],
        "type": "mask"
    }));
    Ok(id)
}

/// Normalize `#RRGGBB` to `#RRGGBBFF`-style 8-digit form (pyJianYingDraft
/// `_normalize_rgba_color` accepts both and stores 8 digits).
fn rgba_normalize(hex: &str) -> String {
    let h = hex.trim().trim_start_matches('#');
    if h.len() == 6 {
        format!("#{}ff", h.to_lowercase())
    } else {
        format!("#{}", h.to_lowercase())
    }
}

/// Build a full draft directory from a validated plan.
pub fn build(
    plan: &Plan,
    plan_dir: &Path,
    out_dir: &Path,
    donor_root: Option<&Path>,
    prober: &dyn Fn(&Path) -> Result<MediaInfo>,
) -> Result<BuildReport> {
    if out_dir.exists() {
        bail!("output directory {} already exists", out_dir.display());
    }
    std::fs::create_dir_all(out_dir)?;

    let mut content: Value = serde_json::from_str(CONTENT_TEMPLATE)?;
    let mut meta: Value = serde_json::from_str(META_TEMPLATE)?;
    let now = now_us();

    content["id"] = json!(uuid_upper());
    content["name"] = json!(plan.name);
    content["duration"] = json!(plan.total_duration());
    content["fps"] = json!(plan.canvas.fps as f64);
    content["canvas_config"] = json!({
        "width": plan.canvas.width, "height": plan.canvas.height,
        "ratio": canvas_ratio(plan.canvas.width, plan.canvas.height)
    });
    content["create_time"] = json!(now / 1_000_000);
    content["update_time"] = json!(now / 1_000_000);
    content["platform"]["os"] = json!(host_os());
    content["last_modified_platform"]["os"] = json!(host_os());
    content["extra_info"] = json!({"created_via": "jianying-cli"});
    let seed_donor = donor_root.and_then(|r| seed_from_donor(r, &mut content));

    let materials = &mut content["materials"];
    let mut draft_materials: Vec<Value> = Vec::new();
    let mut tracks_json: Vec<Value> = Vec::with_capacity(plan.tracks.len());
    let mut segment_count = 0usize;

    for (ti, track) in plan.tracks.iter().enumerate() {
        let mut segments_json: Vec<Value> = Vec::with_capacity(track.segments.len());

        for seg in track.segments.iter() {
            segment_count += 1;
            let mut seg_v: Value;
            let mut refs: Vec<String>;

            match track.kind.as_str() {
                "video" | "audio" => {
                    let src = resolve_source(
                        plan_dir,
                        seg.source.as_deref().unwrap_or_else(|| Path::new("")),
                    );
                    let info = prober(&src)?;
                    let stored = copy_asset(out_dir, track.kind.as_str(), &src)?;

                    let (material_id, companions) = if track.kind == "video" {
                        let id = hex_id();
                        let mtype = if seg.photo || info.is_image { "photo" } else { "video" };
                        materials["videos"].as_array_mut().unwrap().push(json!({
                            "audio_fade": null, "category_id": "", "category_name": "local",
                            "check_flag": 63487,
                            "crop": {"lower_left_x": 0.0, "lower_left_y": 1.0,
                                      "lower_right_x": 1.0, "lower_right_y": 1.0,
                                      "upper_left_x": 0.0, "upper_left_y": 0.0,
                                      "upper_right_x": 1.0, "upper_right_y": 0.0},
                            "crop_ratio": "free", "crop_scale": 1.0,
                            "duration": if mtype == "photo" { 10_800_000_000 } else { info.duration_us },
                            "height": info.height, "width": info.width,
                            "id": id, "local_material_id": "", "material_id": id,
                            "material_name": src.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                            "media_path": "", "path": stored, "type": mtype,
                            "has_audio": info.has_audio && mtype == "video"
                        }));
                        draft_materials.push(json!({
                            "ai_group_type": "", "create_time": -1,
                            "duration": if mtype == "photo" { json!(5_000_000) } else { json!(info.duration_us) },
                            "enter_from": 0,
                            "extra_info": src.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                            "file_Path": stored, "height": info.height, "id": hex_id(),
                            "import_time": -1, "import_time_ms": -1, "item_source": 1,
                            "material_color_tag": "", "md5": "",
                            "metetype": mtype,
                            "roughcut_time_range": {"duration": -1, "start": -1},
                            "sub_time_range": {"duration": -1, "start": -1},
                            "type": 0, "width": info.width
                        }));
                        let c = make_companions(materials, seg.speed.unwrap_or(1.0), true);
                        (id, c)
                    } else {
                        let id = hex_id();
                        materials["audios"].as_array_mut().unwrap().push(json!({
                            "app_id": 0, "category_id": "", "category_name": "local",
                            "check_flag": 3, "copyright_limit_type": "none",
                            "duration": info.duration_us, "effect_id": "", "formula_id": "",
                            "id": id, "local_material_id": id, "music_id": id,
                            "name": src.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                            "path": stored, "source_platform": 0, "type": "extract_music",
                            "wave_points": []
                        }));
                        draft_materials.push(json!({
                            "ai_group_type": "", "create_time": -1, "duration": info.duration_us,
                            "enter_from": 0,
                            "extra_info": src.file_name().map(|n| n.to_string_lossy()).unwrap_or_default(),
                            "file_Path": stored, "height": 0, "id": hex_id(),
                            "import_time": -1, "import_time_ms": -1, "item_source": 1,
                            "material_color_tag": "", "md5": "", "metetype": "music",
                            "roughcut_time_range": {"duration": -1, "start": -1},
                            "sub_time_range": {"duration": -1, "start": -1},
                            "type": 0, "width": 0
                        }));
                        let c = make_companions(materials, seg.speed.unwrap_or(1.0), false);
                        (id, c)
                    };

                    seg_v = base_segment(&material_id, seg, ti as i64);
                    if track.kind == "video" {
                        apply_visuals(&mut seg_v, seg, true);
                    } else {
                        seg_v["clip"] = json!(null);
                        seg_v["hdr_settings"] = json!(null);
                    }
                    refs = companions;

                    if track.kind == "video" {
                        if let Some(m) = &seg.mask {
                            refs.push(mask_entry(materials, m, &info)?);
                        }
                        for f in &seg.filters {
                            let entry =
                                catalogs::resolve(catalogs::filters(), "filter", &f.name, vip_ok())?;
                            let fid = hex_id();
                            materials["effects"].as_array_mut().unwrap().push(json!({
                                "adjust_params": [], "algorithm_artifact_path": "",
                                "apply_target_type": 0, "bloom_params": null,
                                "category_id": "", "category_name": "",
                                "color_match_info": {"source_feature_path": "",
                                    "target_feature_path": "", "target_image_path": ""},
                                "effect_id": entry["effect_id"],
                                "enable_skin_tone_correction": false, "exclusion_group": [],
                                "face_adjust_params": [], "formula_id": "",
                                "id": fid, "intensity_key": "", "multi_language_current": "",
                                "name": entry["name"], "panel_id": "", "platform": "all",
                                "resource_id": entry["resource_id"], "source_platform": 1,
                                "sub_type": "none", "time_range": null, "type": "filter",
                                "value": (f.intensity.unwrap_or(100.0) / 100.0),
                                "version": ""
                            }));
                            refs.push(fid);
                        }
                        for e in &seg.effects {
                            let entry = catalogs::resolve(
                                catalogs::video_scene_effects(), "effect", &e.name, vip_ok())
                                .or_else(|_| catalogs::resolve(
                                    catalogs::video_character_effects(), "effect", &e.name, vip_ok()))?;
                            let eid = hex_id();
                            materials["video_effects"].as_array_mut().unwrap().push(json!({
                                "adjust_params": catalogs::adjust_params(entry, Some(&e.params)),
                                "apply_target_type": 0, "apply_time_range": null,
                                "category_id": "", "category_name": "", "common_keyframes": [],
                                "disable_effect_faces": [], "effect_id": entry["effect_id"],
                                "formula_id": "", "id": eid, "name": entry["name"],
                                "platform": "all", "render_index": 11000,
                                "resource_id": entry["resource_id"], "source_platform": 0,
                                "time_range": null, "track_render_index": 0,
                                "type": "video_effect", "value": 1.0, "version": ""
                            }));
                            refs.push(eid);
                        }
                        if let Some(mm) = &seg.mix_mode {
                            let entry =
                                catalogs::resolve(catalogs::mix_modes(), "mix mode", mm, vip_ok())?;
                            let mid = hex_id();
                            materials["effects"].as_array_mut().unwrap().push(json!({
                                "type": "mix_mode", "name": entry["name"],
                                "effect_id": entry["effect_id"], "resource_id": entry["resource_id"],
                                "value": 1.0, "apply_target_type": 0, "platform": "all",
                                "source_platform": 0, "category_id": "", "category_name": "",
                                "sub_type": "none", "time_range": null, "id": mid
                            }));
                            refs.push(mid);
                        }
                        let mut anims: Vec<Value> = Vec::new();
                        for (kind, a) in [
                            ("in", &seg.animation_in),
                            ("out", &seg.animation_out),
                            ("group", &seg.animation_group),
                        ] {
                            if let Some(a) = a {
                                let catalog = match kind {
                                    "in" => catalogs::video_animations_in(),
                                    "out" => catalogs::video_animations_out(),
                                    _ => catalogs::video_animations_group(),
                                };
                                let entry =
                                    catalogs::resolve(catalog, "animation", &a.name, vip_ok())?;
                                let dur = a.duration_us.unwrap_or_else(|| {
                                    entry["duration_us"].as_i64().unwrap_or(500_000)
                                });
                                let start = match kind {
                                    "out" => (seg.duration_us - dur).max(0),
                                    _ => 0,
                                };
                                anims.push(json!({
                                    "anim_adjust_params": null, "platform": "all",
                                    "panel": "video", "material_type": "video",
                                    "name": entry["name"], "id": entry["effect_id"],
                                    "type": kind, "resource_id": entry["resource_id"],
                                    "start": start, "duration": dur
                                }));
                            }
                        }
                        if !anims.is_empty() {
                            let aid = hex_id();
                            materials["material_animations"].as_array_mut().unwrap().push(json!({
                                "id": aid, "type": "sticker_animation",
                                "multi_language_current": "none", "animations": anims
                            }));
                            refs.push(aid);
                        }
                    }
                    if track.kind == "audio" {
                        for e in &seg.audio_effects {
                            let entry = catalogs::resolve(
                                catalogs::audio_scene_effects(), "audio effect", &e.name, vip_ok())
                                .or_else(|_| catalogs::resolve(
                                    catalogs::tone_effects(), "audio effect", &e.name, vip_ok()))
                                .or_else(|_| catalogs::resolve(
                                    catalogs::speech_to_songs(), "audio effect", &e.name, vip_ok()))?;
                            let aid = hex_id();
                            let domain = if catalogs::find(
                                catalogs::audio_scene_effects(), &e.name).is_some() {
                                ("sound_effect", "场景音")
                            } else if catalogs::find(catalogs::tone_effects(), &e.name).is_some() {
                                ("timbre", "音色")
                            } else {
                                ("song", "声音成曲")
                            };
                            materials["audio_effects"].as_array_mut().unwrap().push(json!({
                                "audio_adjust_params": catalogs::adjust_params(entry, Some(&e.params)),
                                "id": aid, "name": entry["name"],
                                "resource_id": entry["resource_id"],
                                "type": "audio_effect",
                                "category_id": domain.0, "category_name": domain.1,
                                "sub_type": 1, "time_range": {"duration": 0, "start": 0},
                                "is_ugc": false, "production_path": "", "speaker_id": ""
                            }));
                            refs.push(aid);
                        }
                    }
                    if let Some(f) = &seg.fade {
                        let fid = hex_id();
                        materials["audio_fades"].as_array_mut().unwrap().push(json!({
                            "id": fid, "fade_in_duration": f.in_us,
                            "fade_out_duration": f.out_us, "fade_type": 0,
                            "type": "audio_fade"
                        }));
                        refs.push(fid);
                    }
                    if let Some(c) = &seg.chroma {
                        // Chroma.global_id is an UPPERCASE uuid in pyJianYingDraft — quirk kept
                        let cid = uuid_upper();
                        materials["chromas"].as_array_mut().unwrap().push(json!({
                            "color": rgba_normalize(&c.color),
                            "edge_smooth_value": c.edge_smooth / 100.0,
                            "id": cid,
                            "intensity_value": c.intensity / 100.0,
                            "shadow_value": c.shadow / 100.0,
                            "should_transfer_color": true,
                            "spill_value": c.spill / 100.0,
                            "type": "chroma",
                            "version": "v2"
                        }));
                        refs.push(cid);
                    }
                    if let Some(bf) = &seg.background_filling {
                        let bid = hex_id();
                        let ftype = if bf.fill_type == "blur" { "canvas_blur" } else { "canvas_color" };
                        materials["canvases"].as_array_mut().unwrap().push(json!({
                            "id": bid, "type": ftype, "blur": bf.blur,
                            "color": rgba_normalize(if bf.color.is_empty() { "#00000000" } else { &bf.color }),
                            "source_platform": 0
                        }));
                        refs.push(bid);
                    }
                    apply_keyframes(&mut seg_v, seg);
                }
                "text" => {
                    let font_entry = seg
                        .font
                        .as_deref()
                        .map(|f| catalogs::resolve(catalogs::fonts(), "font", f, vip_ok()))
                        .transpose()?;
                    let material_id = hex_id();
                    let content_str = build_text_content(seg, font_entry)?;
                    let border = seg.border_width.unwrap_or(0.0);
                    let mut check_flag = 7;
                    if border > 0.0 {
                        check_flag |= 8;
                    }
                    if seg.background.is_some() {
                        check_flag |= 16;
                    }
                    if seg.shadow.is_some() {
                        check_flag |= 32;
                    }
                    let mut text_material = json!({
                        "id": material_id, "type": "text", "content": content_str,
                        "alignment": seg.alignment.unwrap_or(1),
                        "font_size": seg.size.unwrap_or(8.0),
                        "text_color": seg.color.clone().unwrap_or_else(|| "#FFFFFF".into()).to_uppercase(),
                        "typesetting": 0, "letter_spacing": 0, "line_spacing": 0.02, "line_feed": 1,
                        "line_max_width": 0.82, "force_apply_line_max_width": false,
                        "check_flag": check_flag,
                        "global_alpha": 1.0,
                        "fixed_width": -1, "fixed_height": -1
                    });
                    if border > 0.0 {
                        text_material["has_border"] = json!(true);
                        text_material["border_color"] = json!(seg.border_color.clone()
                            .unwrap_or_else(|| "#000000".into()).to_uppercase());
                        text_material["border_width"] = json!(border);
                        text_material["border_alpha"] = json!(1);
                    }
                    if let Some(b) = &seg.background {
                        text_material["background_style"] = json!(b.style.unwrap_or(1));
                        text_material["background_color"] = json!(b.color.to_uppercase());
                        text_material["background_alpha"] = json!(b.alpha.unwrap_or(1.0));
                        text_material["background_round_radius"] = json!(b.round_radius.unwrap_or(0.0));
                        text_material["background_height"] = json!(b.height.unwrap_or(0.14));
                        text_material["background_width"] = json!(b.width.unwrap_or(0.14));
                        text_material["background_horizontal_offset"] =
                            json!(b.horizontal_offset.unwrap_or(0.0));
                        text_material["background_vertical_offset"] =
                            json!(b.vertical_offset.unwrap_or(0.0));
                    }
                    materials["texts"].as_array_mut().unwrap().push(text_material);

                    refs = Vec::new();
                    for (raw, kind) in [(&seg.text_effect, "text_shape"), (&seg.bubble, "text_shape")] {
                        if let Some(r) = raw {
                            let tid = hex_id();
                            materials["effects"].as_array_mut().unwrap().push(json!({
                                "apply_target_type": 0, "effect_id": r.effect_id,
                                "id": tid, "resource_id": r.resource_id,
                                "type": kind, "value": 1.0,
                                "platform": "all", "source_platform": 0,
                                "category_id": "", "category_name": "", "sub_type": "none",
                                "time_range": null
                            }));
                            refs.push(tid);
                        }
                    }

                    seg_v = base_segment(&material_id, seg, ti as i64);
                    seg_v["source_timerange"] = json!({"start": 0, "duration": seg.duration_us});
                    apply_visuals(&mut seg_v, seg, false);
                    // pyJianYingDraft TextSegment defaults ClipSettings(transform_y=-0.78)
                    if seg.y.is_none() {
                        seg_v["clip"]["transform"]["y"] = json!(-0.78);
                    }
                    let mut anims: Vec<Value> = Vec::new();
                    for (kind, a) in [
                        ("in", &seg.animation_in),
                        ("out", &seg.animation_out),
                        ("group", &seg.animation_group),
                    ] {
                        if let Some(a) = a {
                            let catalog = match kind {
                                "in" => catalogs::text_animations_in(),
                                "out" => catalogs::text_animations_out(),
                                _ => catalogs::text_animations_loop(),
                            };
                            let entry = catalogs::resolve(catalog, "animation", &a.name, vip_ok())?;
                            let dur = a
                                .duration_us
                                .unwrap_or_else(|| entry["duration_us"].as_i64().unwrap_or(500_000));
                            let start = match kind {
                                "out" => (seg.duration_us - dur).max(0),
                                _ => 0,
                            };
                            // pyJYD wire types: in/out/loop for text, in/out/group for video
                            let wire_type = if kind == "group" { "loop" } else { kind };
                            anims.push(json!({
                                "anim_adjust_params": null, "platform": "all", "panel": "",
                                "material_type": "sticker", "name": entry["name"],
                                "id": entry["effect_id"], "type": wire_type,
                                "resource_id": entry["resource_id"],
                                "start": start, "duration": dur
                            }));
                        }
                    }
                    if !anims.is_empty() {
                        let aid = hex_id();
                        materials["material_animations"].as_array_mut().unwrap().push(json!({
                            "id": aid, "type": "sticker_animation",
                            "multi_language_current": "none", "animations": anims
                        }));
                        refs.push(aid);
                    }
                    apply_keyframes(&mut seg_v, seg);
                }
                "sticker" => {
                    let material_id = hex_id();
                    materials["stickers"].as_array_mut().unwrap().push(json!({
                        "id": material_id,
                        "resource_id": seg.resource_id,
                        "sticker_id": seg.sticker_id.as_deref().unwrap_or(seg.resource_id.as_deref().unwrap_or_default()),
                        "source_platform": 1, "type": "sticker"
                    }));
                    seg_v = base_segment(&material_id, seg, ti as i64);
                    apply_visuals(&mut seg_v, seg, false);
                    refs = Vec::new();
                    apply_keyframes(&mut seg_v, seg);
                }
                "filter" | "effect" => {
                    let (entry, is_filter) = if track.kind == "filter" {
                        let f = &seg.filters[0];
                        (catalogs::resolve(catalogs::filters(), "filter", &f.name, vip_ok())?, true)
                    } else {
                        let e = &seg.effects[0];
                        (catalogs::resolve(
                            catalogs::video_scene_effects(), "effect", &e.name, vip_ok())
                            .or_else(|_| catalogs::resolve(
                                catalogs::video_character_effects(), "effect", &e.name, vip_ok()))?, false)
                    };
                    let material_id = hex_id();
                    if is_filter {
                        materials["effects"].as_array_mut().unwrap().push(json!({
                            "adjust_params": [], "algorithm_artifact_path": "",
                            "apply_target_type": 0, "bloom_params": null,
                            "category_id": "", "category_name": "",
                            "color_match_info": {"source_feature_path": "",
                                "target_feature_path": "", "target_image_path": ""},
                            "effect_id": entry["effect_id"],
                            "enable_skin_tone_correction": false, "exclusion_group": [],
                            "face_adjust_params": [], "formula_id": "",
                            "id": material_id, "intensity_key": "", "multi_language_current": "",
                            "name": entry["name"], "panel_id": "", "platform": "all",
                            "resource_id": entry["resource_id"], "source_platform": 1,
                            "sub_type": "none", "time_range": null, "type": "filter",
                            "value": ((seg.intensity.or(seg.filters[0].intensity).unwrap_or(100.0)) / 100.0),
                            "version": ""
                        }));
                    } else {
                        let params = seg
                            .params
                            .clone()
                            .or_else(|| Some(seg.effects[0].params.clone()))
                            .unwrap_or_default();
                        materials["video_effects"].as_array_mut().unwrap().push(json!({
                            "adjust_params": catalogs::adjust_params(entry, Some(&params)),
                            "apply_target_type": 2, "apply_time_range": null,
                            "category_id": "", "category_name": "", "common_keyframes": [],
                            "disable_effect_faces": [], "effect_id": entry["effect_id"],
                            "formula_id": "", "id": material_id, "name": entry["name"],
                            "platform": "all", "render_index": 11000,
                            "resource_id": entry["resource_id"], "source_platform": 0,
                            "time_range": null, "track_render_index": 0,
                            "type": "video_effect", "value": 1.0, "version": ""
                        }));
                    }
                    seg_v = base_segment(&material_id, seg, ti as i64);
                    seg_v["extra_material_refs"] = json!([material_id]);
                    segments_json.push(seg_v);
                    continue;
                }
                _ => unreachable!("validated track kind"),
            }

            if let Some(t) = &seg.transition_out {
                let entry =
                    catalogs::resolve(catalogs::transitions(), "transition", &t.name, vip_ok())?;
                let transition_id = hex_id();
                materials["transitions"].as_array_mut().unwrap().push(json!({
                    "category_id": "", "category_name": "",
                    "duration": t.duration_us.unwrap_or_else(|| {
                        entry["duration_us"].as_i64().unwrap_or(500_000)
                    }),
                    "effect_id": entry["effect_id"],
                    "id": transition_id,
                    "is_overlap": entry["is_overlap"],
                    "name": entry["name"],
                    "platform": "all",
                    "resource_id": entry["resource_id"],
                    "type": "transition"
                }));
                refs.push(transition_id);
            }

            seg_v["extra_material_refs"] = json!(refs);
            segments_json.push(seg_v);
        }

        tracks_json.push(json!({
            "attribute": 0, "flag": 0, "id": hex_id(), "is_default_name": false,
            "name": track.name.clone().unwrap_or_else(|| track.kind.clone()),
            "segments": segments_json, "type": track.kind
        }));
    }

    content["tracks"] = json!(tracks_json);

    if !draft_materials.is_empty() {
        meta["draft_materials"] = json!([{"type": 0, "value": draft_materials}]);
    }
    meta["draft_id"] = json!(uuid::Uuid::new_v4().to_string());
    meta["draft_name"] = json!(plan.name);
    meta["draft_fold_path"] = json!(out_dir
        .canonicalize()
        .unwrap_or_else(|_| out_dir.to_path_buf())
        .to_string_lossy());
    meta["draft_root_path"] =
        json!(out_dir.parent().map(|p| p.to_string_lossy()).unwrap_or_default());
    meta["draft_json_file"] = json!(out_dir.join("draft_content.json").to_string_lossy());
    meta["tm_draft_create"] = json!(now);
    meta["tm_draft_modified"] = json!(now);
    meta["tm_draft_removed"] = json!(0);
    meta["tm_duration"] = json!(plan.total_duration());

    let content_str = serde_json::to_string_pretty(&content)?;
    std::fs::write(out_dir.join("draft_content.json"), &content_str)?;
    std::fs::write(out_dir.join("draft_info.json"), &content_str)?;
    std::fs::write(
        out_dir.join("draft_meta_info.json"),
        serde_json::to_string_pretty(&meta)?,
    )?;

    Ok(BuildReport {
        name: plan.name.clone(),
        duration_us: plan.total_duration(),
        tracks: plan.tracks.len(),
        segments: segment_count,
        out_dir: out_dir.to_string_lossy().into_owned(),
        seed_donor,
    })
}

/// Load the timeline from a draft directory (either mirror).
pub fn load_timeline(draft_dir: &Path) -> Result<Value> {
    for name in ["draft_content.json", "draft_info.json"] {
        let p = draft_dir.join(name);
        if p.is_file() {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
            if v.get("tracks").map(Value::is_array).unwrap_or(false) {
                return Ok(v);
            }
        }
    }
    bail!(
        "no readable timeline in {} (expected draft_content.json or draft_info.json)",
        draft_dir.display()
    );
}

/// Structural lint over a built/published draft.
pub fn verify(draft_dir: &Path) -> Result<Value> {
    let tl = load_timeline(draft_dir)?;
    let mut issues: Vec<String> = Vec::new();

    let tracks = tl["tracks"].as_array().context("tracks must be an array")?;
    if tracks.is_empty() {
        issues.push("no tracks".into());
    }
    let first_video = tracks.iter().position(|t| t["type"] == "video");
    if let Some(fv) = first_video {
        if fv != 0 {
            issues.push("a video track exists but is not the first track".into());
        }
    }
    let ref_buckets = ["speeds", "placeholders", "sound_channel_mappings", "vocal_separations",
        "canvases", "material_colors", "transitions", "masks", "chromas", "effects",
        "video_effects", "audio_fades", "audio_effects", "material_animations"];
    let mut max_end: i64 = 0;
    for (ti, t) in tracks.iter().enumerate() {
        let segs = t["segments"].as_array().context("segments must be an array")?;
        let mut prior_end: i64 = 0;
        for s in segs {
            let start = s["target_timerange"]["start"].as_i64().unwrap_or(0);
            let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
            if start < prior_end {
                issues.push(format!("track {ti}: overlapping segment at {start}"));
            }
            prior_end = start + dur;
            max_end = max_end.max(prior_end);
            let mid = s["material_id"].as_str().unwrap_or_default();
            let bucket = match t["type"].as_str() {
                Some("video") => Some("videos"),
                Some("audio") => Some("audios"),
                Some("text") => Some("texts"),
                Some("sticker") => Some("stickers"),
                Some("filter") => Some("effects"),
                Some("effect") => Some("video_effects"),
                _ => None,
            };
            if let Some(bucket) = bucket {
                let found = tl["materials"][bucket]
                    .as_array()
                    .map(|a| a.iter().any(|m| m["id"] == json!(mid)))
                    .unwrap_or(false);
                if !found {
                    issues.push(format!("track {ti}: material {mid} missing from {bucket}"));
                }
            }
            for r in s["extra_material_refs"].as_array().unwrap_or(&Vec::new()) {
                let r = r.as_str().unwrap_or_default();
                let known = ref_buckets.iter().any(|b| {
                    tl["materials"][b]
                        .as_array()
                        .map(|a| a.iter().any(|m| m["id"] == json!(r)))
                        .unwrap_or(false)
                });
                if !known {
                    issues.push(format!("track {ti}: dangling extra_material_ref {r}"));
                }
            }
        }
        if ti == 0 && t["type"] == "video" {
            let first = t["segments"][0]["target_timerange"]["start"].as_i64().unwrap_or(-1);
            if first != 0 {
                issues.push("main video must start at 0".into());
            }
        }
    }
    let declared = tl["duration"].as_i64().unwrap_or(0);
    if declared != max_end {
        issues.push(format!("top duration {declared} != max segment end {max_end}"));
    }
    Ok(json!({
        "ok": issues.is_empty(),
        "name": tl["name"],
        "duration_us": declared,
        "tracks": tracks.len(),
        "issues": issues,
    }))
}

/// Human/agent-readable summary of a draft.
pub fn inspect(draft_dir: &Path) -> Result<Value> {
    let tl = load_timeline(draft_dir)?;
    let tracks: Vec<Value> = tl["tracks"]
        .as_array()
        .unwrap_or(&Vec::new())
        .iter()
        .map(|t| {
            json!({
                "type": t["type"], "name": t["name"],
                "segments": t["segments"].as_array().map(|a| a.len()).unwrap_or(0),
            })
        })
        .collect();
    Ok(json!({
        "name": tl["name"], "duration_us": tl["duration"],
        "canvas": tl["canvas_config"], "fps": tl["fps"],
        "platform": tl["platform"], "tracks": tracks,
    }))
}


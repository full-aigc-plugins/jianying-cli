//! Template mode (pyJianYingDraft parity): operate on an existing draft —
//! inspect materials, duplicate, replace text, replace materials, import a
//! track from another draft, and build on top of a template.

use crate::probe;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

const TEMPLATE_MANIFEST: &str = ".jianying-template.json";
const TEMPLATE_SCHEMA: &str = "jianying-template/v1";
const PRESET_SCHEMA: &str = "jianying-template-preset/v1";

fn hex_id() -> String {
    Uuid::new_v4().simple().to_string()
}

fn now_us() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

fn load(draft: &Path) -> Result<Value> {
    for name in ["draft_content.json", "draft_info.json"] {
        let p = draft.join(name);
        if p.is_file() {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
            if v.get("tracks").is_some() {
                return Ok(v);
            }
        }
    }
    bail!("{} is not a readable draft", draft.display())
}

fn save(draft: &Path, tl: &Value) -> Result<()> {
    save_as(draft, tl, draft)
}

fn save_as(draft: &Path, tl: &Value, identity: &Path) -> Result<()> {
    jianying_draft::DraftTimelineWire::from_value(tl.clone())?
        .validate_references()
        .context("template operation produced invalid material references")?;
    let s = serde_json::to_string_pretty(tl)?;
    std::fs::write(draft.join("draft_content.json"), &s)?;
    std::fs::write(draft.join("draft_info.json"), &s)?;
    sync_metadata(draft, tl, identity)?;
    crate::draft::validate_bundle_as(draft, identity)
        .context("template operation produced an inconsistent draft bundle")?;
    Ok(())
}

/// 保存模板合成后的时间线，并原子同步双镜像、元数据和素材注册关系。
pub fn save_timeline(draft: &Path, timeline: &Value) -> Result<()> {
    save(draft, timeline)
}

/// 保存隔离工作副本，并将草稿路径元数据固定为原子提交后的最终路径。
pub fn save_timeline_as(draft: &Path, timeline: &Value, identity: &Path) -> Result<()> {
    save_as(draft, timeline, identity)
}

fn sync_metadata(draft: &Path, tl: &Value, identity: &Path) -> Result<()> {
    let meta_path = draft.join("draft_meta_info.json");
    if !meta_path.is_file() {
        bail!("{} is missing draft_meta_info.json", draft.display());
    }
    let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
    meta["draft_name"] = tl["name"].clone();
    meta["tm_duration"] = tl["duration"].clone();
    meta["draft_fold_path"] = json!(identity
        .canonicalize()
        .unwrap_or_else(|_| identity.to_path_buf())
        .to_string_lossy());
    meta["draft_root_path"] = json!(identity
        .parent()
        .unwrap_or(identity)
        .canonicalize()
        .unwrap_or_else(|_| identity.parent().unwrap_or(identity).to_path_buf())
        .to_string_lossy());
    meta["draft_json_file"] = json!(identity.join("draft_content.json").to_string_lossy());

    let existing = registered_materials(&meta);
    let mut registrations = Vec::new();
    for (bucket, fallback_kind) in [("videos", "video"), ("audios", "music")] {
        for material in tl["materials"][bucket].as_array().into_iter().flatten() {
            let path = material["path"]
                .as_str()
                .or_else(|| material["media_path"].as_str())
                .unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            if let Some(entry) = existing.get(path) {
                registrations.push(entry.clone());
                continue;
            }
            let metetype = if bucket == "videos" {
                material["type"].as_str().unwrap_or(fallback_kind)
            } else {
                fallback_kind
            };
            registrations.push(json!({
                "ai_group_type": "", "create_time": -1,
                "duration": material["duration"], "enter_from": 0,
                "extra_info": material["material_name"].as_str()
                    .or_else(|| material["name"].as_str()).unwrap_or_default(),
                "file_Path": path, "height": material["height"].as_i64().unwrap_or(0),
                "id": hex_id(), "import_time": -1, "import_time_ms": -1,
                "item_source": 1, "material_color_tag": "", "md5": "",
                "metetype": metetype,
                "roughcut_time_range": {"duration": -1, "start": -1},
                "sub_time_range": {"duration": -1, "start": -1},
                "type": 0, "width": material["width"].as_i64().unwrap_or(0)
            }));
        }
    }
    let groups = meta["draft_materials"]
        .as_array_mut()
        .context("draft_materials must be an array")?;
    if let Some(local) = groups.iter_mut().find(|group| group["type"] == json!(0)) {
        local["value"] = json!(registrations);
    } else {
        groups.push(json!({"type": 0, "value": registrations}));
    }
    meta["tm_draft_modified"] = json!(now_us());
    std::fs::write(meta_path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

fn registered_materials(meta: &Value) -> std::collections::BTreeMap<String, Value> {
    let mut registered = std::collections::BTreeMap::new();
    for group in meta["draft_materials"].as_array().into_iter().flatten() {
        for entry in group["value"].as_array().into_iter().flatten() {
            if let Some(path) = entry["file_Path"].as_str() {
                registered.insert(path.to_owned(), entry.clone());
            }
        }
    }
    registered
}

/// `inspect_material` parity: tracks and a per-bucket material inventory.
pub fn inspect_materials(draft: &Path) -> Result<Value> {
    let tl = load(draft)?;
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
    let mut materials: Vec<Value> = Vec::new();
    if let Some(buckets) = tl["materials"].as_object() {
        for (bucket, items) in buckets {
            for m in items.as_array().unwrap_or(&Vec::new()) {
                let name = m["material_name"]
                    .as_str()
                    .or_else(|| m["name"].as_str())
                    .unwrap_or_default();
                if name.is_empty() && m["content"].is_null() {
                    continue;
                }
                materials.push(json!({
                    "bucket": bucket,
                    "id": m["id"],
                    "name": name,
                    "type": m["type"],
                    "path": m["path"].as_str().or(m["media_path"].as_str()),
                    "duration_us": m["duration"],
                }));
            }
        }
    }
    Ok(json!({"draft": draft.to_string_lossy(), "tracks": tracks, "materials": materials}))
}

/// 列出模板根目录中由 `template save` 登记的模板。
pub fn list_templates(root: &Path) -> Result<Value> {
    if !root.is_dir() {
        return Ok(json!({"root":root,"templates":[]}));
    }
    let mut templates = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let manifest_path = entry.path().join(TEMPLATE_MANIFEST);
        if !manifest_path.is_file() {
            continue;
        }
        let manifest: Value = serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
        if manifest["schema"].as_str() != Some(TEMPLATE_SCHEMA) {
            bail!("unsupported template manifest: {}", manifest_path.display());
        }
        crate::draft::validate_bundle(&entry.path())
            .with_context(|| format!("invalid template bundle {}", entry.path().display()))?;
        templates.push(json!({
            "name":manifest["name"],"description":manifest["description"],
            "created_at_us":manifest["created_at_us"],"path":entry.path()
        }));
    }
    templates.sort_by(|left, right| {
        left["name"]
            .as_str()
            .unwrap_or_default()
            .cmp(right["name"].as_str().unwrap_or_default())
    });
    Ok(json!({"root":root,"templates":templates}))
}

/// 将既有草稿复制、重标识并登记为可枚举模板。
pub fn save_template(
    draft: &Path,
    name: &str,
    root: &Path,
    description: Option<&str>,
) -> Result<Value> {
    validate_library_name(name)?;
    std::fs::create_dir_all(root)?;
    let duplicated = duplicate(draft, name, Some(root))?;
    let template_path = root.join(name);
    let manifest = json!({
        "schema":TEMPLATE_SCHEMA,"name":name,"description":description.unwrap_or_default(),
        "created_at_us":now_us(),
        "source_name":draft.file_name().and_then(|value| value.to_str()).unwrap_or_default()
    });
    std::fs::write(
        template_path.join(TEMPLATE_MANIFEST),
        serde_json::to_string_pretty(&manifest)?,
    )?;
    Ok(
        json!({"status":"saved","template":template_path,"manifest":manifest,
        "duplicate":duplicated}),
    )
}

/// 从已登记模板生成一个独立草稿；输出不保留模板登记标记。
pub fn apply_template(template: &Path, new_name: &str, root: &Path) -> Result<Value> {
    validate_library_name(new_name)?;
    let manifest_path = template.join(TEMPLATE_MANIFEST);
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .with_context(|| format!("reading template manifest {}", manifest_path.display()))?,
    )?;
    if manifest["schema"].as_str() != Some(TEMPLATE_SCHEMA) {
        bail!("unsupported template manifest schema");
    }
    crate::draft::validate_bundle(template)?;
    std::fs::create_dir_all(root)?;
    let output = duplicate(template, new_name, Some(root))?;
    let applied = root.join(new_name);
    let copied_manifest = applied.join(TEMPLATE_MANIFEST);
    if copied_manifest.is_file() {
        std::fs::remove_file(copied_manifest)?;
    }
    crate::draft::validate_bundle(&applied)?;
    Ok(json!({"status":"applied","name":new_name,"draft":applied,
        "template_name":manifest["name"],"duplicate":output}))
}

/// 从草稿首个文字素材提取可移植基础样式预设。
pub fn make_preset(draft: &Path, name: &str, root: &Path) -> Result<Value> {
    validate_library_name(name)?;
    let timeline = load(draft)?;
    let material = timeline["materials"]["texts"]
        .as_array()
        .and_then(|items| items.first())
        .context("draft has no text material to create a preset from")?;
    let content: Value = match &material["content"] {
        Value::String(raw) => serde_json::from_str(raw)?,
        Value::Object(_) => material["content"].clone(),
        _ => bail!("text material content must be a JSON string or object"),
    };
    let base_style = content["styles"]
        .as_array()
        .and_then(|styles| styles.first())
        .cloned()
        .context("text material has no base style")?;
    let preset = json!({
        "schema":PRESET_SCHEMA,"name":name,"created_at_us":now_us(),
        "text_style":{"font_size":material["font_size"],"text_color":material["text_color"],
            "alignment":material["alignment"],"base_style":base_style}
    });
    std::fs::create_dir_all(root)?;
    let path = root.join(format!("{name}.json"));
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    std::fs::write(&path, serde_json::to_string_pretty(&preset)?)?;
    Ok(json!({"status":"created","preset":path,"name":name}))
}

/// 将文字样式预设事务化应用到草稿中的全部文字素材。
pub fn apply_preset(draft: &Path, preset_path: &Path) -> Result<Value> {
    let preset: Value = serde_json::from_str(&std::fs::read_to_string(preset_path)?)?;
    if preset["schema"].as_str() != Some(PRESET_SCHEMA) {
        bail!("unsupported template preset schema");
    }
    let style = preset["text_style"]
        .as_object()
        .context("preset text_style must be an object")?
        .clone();
    let base_style = style
        .get("base_style")
        .filter(|value| value.is_object())
        .cloned()
        .context("preset base_style must be an object")?;
    crate::timeline_ops::mutate(draft, |timeline| {
        let texts = timeline["materials"]["texts"]
            .as_array_mut()
            .context("materials.texts must be an array")?;
        for material in texts.iter_mut() {
            for field in ["font_size", "text_color", "alignment"] {
                if let Some(value) = style.get(field) {
                    material[field] = value.clone();
                }
            }
            let was_object = material["content"].is_object();
            let mut content: Value = match &material["content"] {
                Value::String(raw) => serde_json::from_str(raw)?,
                Value::Object(_) => material["content"].clone(),
                _ => bail!("text material content must be a JSON string or object"),
            };
            let length = content["text"]
                .as_str()
                .unwrap_or_default()
                .encode_utf16()
                .count();
            let styles = content["styles"]
                .as_array_mut()
                .context("text content styles must be an array")?;
            let mut applied = base_style.clone();
            applied["range"] = json!([0, length]);
            if styles.is_empty() {
                styles.push(applied);
            } else {
                styles[0] = applied;
            }
            material["content"] = if was_object {
                content
            } else {
                Value::String(serde_json::to_string(&content)?)
            };
        }
        Ok(json!({"status":"applied","preset":preset_path,
            "name":preset["name"],"text_materials":texts.len()}))
    })
}

fn validate_library_name(name: &str) -> Result<()> {
    let path = Path::new(name);
    if name.trim().is_empty()
        || path.is_absolute()
        || path.components().count() != 1
        || matches!(name, "." | "..")
    {
        bail!("template or preset name must be one safe path component");
    }
    Ok(())
}

/// `duplicate_as_template` parity: copy the draft under a new name and restamp.
pub fn duplicate(src: &Path, new_name: &str, root: Option<&Path>) -> Result<Value> {
    validate_library_name(new_name)?;
    let dest_root = root
        .map(Path::to_path_buf)
        .or_else(|| src.parent().map(Path::to_path_buf))
        .ok_or_else(|| anyhow::anyhow!("cannot determine destination"))?;
    let dest = dest_root.join(new_name);
    if dest.exists() {
        bail!("{} already exists", dest.display());
    }
    copy_dir(src, &dest)?;
    let mut tl = load(&dest)?;
    crate::draft::relocate_material_paths(&mut tl, src, &dest);
    tl["id"] = json!(Uuid::new_v4().to_string().to_uppercase());
    tl["name"] = json!(new_name);
    let now = now_us();
    tl["create_time"] = json!(now / 1_000_000);
    tl["update_time"] = json!(now / 1_000_000);
    save(&dest, &tl)?;
    let meta_path = dest.join("draft_meta_info.json");
    if meta_path.is_file() {
        let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
        meta["draft_id"] = json!(Uuid::new_v4().to_string());
        meta["draft_name"] = json!(new_name);
        meta["draft_fold_path"] = json!(dest.to_string_lossy());
        meta["draft_json_file"] = json!(dest.join("draft_content.json").to_string_lossy());
        meta["tm_draft_create"] = json!(now);
        meta["tm_draft_modified"] = json!(now);
        std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    }
    Ok(json!({"status": "duplicated", "name": new_name, "draft": dest.to_string_lossy()}))
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dest.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// `replace_text` parity: rewrite a text segment's content on a text track.
/// `recalc_style` semantics: the base style range is recomputed to the new
/// UTF-16 length and styled ranges are clamped.
pub fn replace_text(
    draft: &Path,
    track_name: &str,
    seg_index: usize,
    new_text: &str,
) -> Result<Value> {
    let mut tl = load(draft)?;
    let material_id = {
        let track = tl["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .find(|t| t["type"] == "text")
            .ok_or_else(|| anyhow::anyhow!("no text track"))?;
        let segs = track["segments"]
            .as_array_mut()
            .context("segments must be an array")?;
        if seg_index >= segs.len() {
            bail!(
                "segment index {seg_index} out of range ({} segments)",
                segs.len()
            );
        }
        segs[seg_index]["material_id"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    };
    let texts = tl["materials"]["texts"]
        .as_array_mut()
        .context("no texts bucket")?;
    let mat = texts
        .iter_mut()
        .find(|m| m["id"] == json!(material_id))
        .ok_or_else(|| anyhow::anyhow!("text material {material_id} not found"))?;
    let mut content: Value = serde_json::from_str(mat["content"].as_str().unwrap_or("{}"))?;
    content["text"] = json!(new_text);
    let utf16_len = new_text.encode_utf16().count() as i64;
    if let Some(styles) = content["styles"].as_array_mut() {
        if let Some(base) = styles.first_mut() {
            base["range"] = json!([0, utf16_len]);
        }
        for s in styles.iter_mut().skip(1) {
            if let Some(range) = s["range"].as_array_mut() {
                let end = range.get(1).and_then(Value::as_i64).unwrap_or(utf16_len);
                range[1] = json!(end.min(utf16_len));
            }
        }
    }
    mat["content"] = json!(content.to_string());
    save(draft, &tl)?;
    Ok(
        json!({"status": "replaced", "track": track_name, "segment": seg_index,
              "text": new_text, "utf16_len": utf16_len}),
    )
}

/// `replace_material_by_name` / `replace_material_by_seg` parity: swap a
/// material's source file and re-probe duration/dimensions; segment source
/// ranges are clamped into the new media.
pub fn replace_material(
    draft: &Path,
    by_name: Option<&str>,
    track: Option<&str>,
    seg_index: Option<usize>,
    new_source: &Path,
) -> Result<Value> {
    let mut tl = load(draft)?;
    let info = probe::probe(new_source)?;
    let stored_source = copy_replacement_asset(draft, new_source)?;
    let mut target_id = String::new();
    if let Some(name) = by_name {
        for bucket in ["videos", "audios"] {
            if let Some(items) = tl["materials"][bucket].as_array_mut() {
                if let Some(m) = items.iter_mut().find(|m| {
                    m["material_name"].as_str() == Some(name) || m["name"].as_str() == Some(name)
                }) {
                    apply_material_swap(m, bucket, &stored_source, &info)?;
                    target_id = m["id"].as_str().unwrap_or_default().to_string();
                    break;
                }
            }
        }
    } else if let (Some(track_name), Some(idx)) = (track, seg_index) {
        let t = tl["tracks"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|t| t["name"] == json!(track_name))
            .ok_or_else(|| anyhow::anyhow!("track {track_name} not found"))?;
        let seg = &t["segments"].as_array().unwrap()[idx];
        target_id = seg["material_id"].as_str().unwrap_or_default().to_string();
        for bucket in ["videos", "audios"] {
            if let Some(items) = tl["materials"][bucket].as_array_mut() {
                if let Some(m) = items.iter_mut().find(|m| m["id"] == json!(target_id)) {
                    apply_material_swap(m, bucket, &stored_source, &info)?;
                    break;
                }
            }
        }
    } else {
        bail!("provide --name or --track + --index");
    }
    if target_id.is_empty() {
        bail!("material not found");
    }
    // clamp every segment source range that references this material
    for t in tl["tracks"].as_array_mut().unwrap() {
        for s in t["segments"].as_array_mut().unwrap_or(&mut Vec::new()) {
            if s["material_id"] == json!(target_id) {
                if let Some(src) = s["source_timerange"].as_object_mut() {
                    let start = src.get("start").and_then(Value::as_i64).unwrap_or(0);
                    let dur = src.get("duration").and_then(Value::as_i64).unwrap_or(0);
                    let clamped = dur.min((info.duration_us - start).max(0));
                    src.insert("duration".into(), json!(clamped));
                }
            }
        }
    }
    save(draft, &tl)?;
    Ok(json!({"status": "replaced", "material_id": target_id,
              "source": stored_source.to_string_lossy(), "duration_us": info.duration_us}))
}

fn copy_replacement_asset(draft: &Path, source: &Path) -> Result<std::path::PathBuf> {
    if !source.is_file() {
        bail!("replacement material does not exist: {}", source.display());
    }
    let target_dir = draft.join("assets/replaced");
    std::fs::create_dir_all(&target_dir)?;
    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "material.bin".to_owned());
    let target = target_dir.join(format!("{}-{file_name}", hex_id()));
    std::fs::copy(source, &target)?;
    Ok(target)
}

fn apply_material_swap(
    m: &mut Value,
    bucket: &str,
    new_source: &Path,
    info: &probe::MediaInfo,
) -> Result<()> {
    let name = new_source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    m["path"] = json!(new_source.to_string_lossy());
    m["duration"] = json!(info.duration_us);
    if bucket == "videos" {
        m["width"] = json!(info.width);
        m["height"] = json!(info.height);
        m["material_name"] = json!(name);
        m["type"] = json!(if info.has_video { "video" } else { "photo" });
        m["has_audio"] = json!(info.has_audio);
    } else {
        m["name"] = json!(name);
    }
    Ok(())
}

/// `import_track` parity: copy a named track from another draft, remapping
/// material ids and copying the referenced material entries. `before` gives
/// pyJYD insert-track semantics (insert at a position instead of appending).
pub fn import_track_at(
    target_draft: &Path,
    source_draft: &Path,
    track_name: &str,
    before: Option<&str>,
) -> Result<Value> {
    let mut target = load(target_draft)?;
    let source = load(source_draft)?;
    let track = source["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == json!(track_name) || t["type"] == json!(track_name))
        .ok_or_else(|| anyhow::anyhow!("track {track_name} not found in source"))?
        .clone();
    if target["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|t| t["name"] == track["name"])
    {
        bail!("target already has a track named {}", track["name"]);
    }
    let mut id_map: std::collections::BTreeMap<String, String> = Default::default();
    let mut new_track = track.clone();
    new_track["id"] = json!(hex_id());
    for seg in new_track["segments"].as_array_mut().context("segments")? {
        seg["id"] = json!(hex_id());
        let old_mid = seg["material_id"].as_str().unwrap_or_default().to_string();
        let new_mid = copy_material(&source, &mut target, &old_mid, source_draft, target_draft)?;
        id_map.insert(old_mid.clone(), new_mid.clone());
        seg["material_id"] = json!(new_mid);
        if let Some(refs) = seg["extra_material_refs"].as_array_mut() {
            for r in refs.iter_mut() {
                let old = r.as_str().unwrap_or_default().to_string();
                let mapped = if let Some(mapped) = id_map.get(&old) {
                    mapped.clone()
                } else {
                    let new =
                        copy_material(&source, &mut target, &old, source_draft, target_draft)?;
                    id_map.insert(old, new.clone());
                    new
                };
                *r = json!(mapped);
            }
        }
    }
    let tracks = target["tracks"].as_array_mut().unwrap();
    match before {
        Some(anchor) => {
            let pos = tracks
                .iter()
                .position(|t| t["name"] == json!(anchor))
                .ok_or_else(|| anyhow::anyhow!("track {anchor} not found in target"))?;
            tracks.insert(pos, new_track);
        }
        None => tracks.push(new_track),
    }
    let new_dur = target["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["segments"].as_array().cloned())
        .flatten()
        .map(|s| {
            s["target_timerange"]["start"].as_i64().unwrap_or(0)
                + s["target_timerange"]["duration"].as_i64().unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    target["duration"] = json!(new_dur);
    save(target_draft, &target)?;
    Ok(json!({"status": "imported", "track": track_name,
              "before": before,
              "source": source_draft.to_string_lossy(),
              "segments": track["segments"].as_array().map(|a| a.len()).unwrap_or(0)}))
}

fn copy_material(
    source: &Value,
    target: &mut Value,
    material_id: &str,
    source_draft: &Path,
    target_draft: &Path,
) -> Result<String> {
    for (bucket, items) in source["materials"].as_object().context("materials")? {
        if let Some(arr) = items.as_array() {
            if let Some(m) = arr.iter().find(|m| m["id"] == json!(material_id)) {
                let new_id = hex_id();
                let mut copy = m.clone();
                copy["id"] = json!(new_id);
                if copy.get("material_id").is_some() {
                    copy["material_id"] = json!(new_id);
                }
                if copy.get("local_material_id").and_then(Value::as_str) == Some(material_id) {
                    copy["local_material_id"] = json!(new_id);
                }
                if copy.get("music_id").and_then(Value::as_str) == Some(material_id) {
                    copy["music_id"] = json!(new_id);
                }
                copy_material_file(&mut copy, source_draft, target_draft, &new_id)?;
                target["materials"][bucket]
                    .as_array_mut()
                    .with_context(|| format!("bucket {bucket} missing in target"))?
                    .push(copy);
                return Ok(new_id);
            }
        }
    }
    bail!("material {material_id} not found in source")
}

fn copy_material_file(
    material: &mut Value,
    source_draft: &Path,
    target_draft: &Path,
    new_id: &str,
) -> Result<()> {
    let field = if material["path"]
        .as_str()
        .is_some_and(|path| !path.is_empty())
    {
        "path"
    } else if material["media_path"]
        .as_str()
        .is_some_and(|path| !path.is_empty())
    {
        "media_path"
    } else {
        return Ok(());
    };
    let raw = material[field].as_str().unwrap_or_default();
    let raw_path = Path::new(raw);
    let source_path = if raw_path.is_absolute() {
        raw_path.to_path_buf()
    } else {
        source_draft.join(raw_path)
    };
    if !source_path.is_file() {
        bail!(
            "referenced material file does not exist: {}",
            source_path.display()
        );
    }
    let file_name = source_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "material.bin".to_owned());
    let target_dir = target_draft.join("assets/imported");
    std::fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(format!("{new_id}-{file_name}"));
    std::fs::copy(&source_path, &target_path)?;
    material[field] = json!(target_path.to_string_lossy());
    Ok(())
}

/// `load_template` + add-segments parity: build the plan's tracks on top of a
/// template timeline, keeping its tracks and materials.
pub fn build_on_template(
    template_dir: &Path,
    target_dir: &Path,
    base: &mut Value,
    plan_name: &str,
) -> Result<()> {
    let template = load(template_dir)?;
    let now = now_us();
    base["id"] = template["id"].clone();
    base["name"] = json!(plan_name);
    base["platform"] = template["platform"].clone();
    base["last_modified_platform"] = template["last_modified_platform"].clone();
    base["update_time"] = json!(now / 1_000_000);
    // keep template canvas/fps unless the plan changed them is decided by caller;
    // merge template materials buckets into base
    if let (Some(tm), Some(bm)) = (
        template["materials"].as_object(),
        base["materials"].as_object_mut(),
    ) {
        for (bucket, items) in tm {
            if let Some(arr) = items.as_array() {
                for m in arr {
                    let entry_id = m["id"].as_str().unwrap_or_default();
                    let already = bm
                        .get(bucket)
                        .and_then(Value::as_array)
                        .map(|a| a.iter().any(|e| e["id"] == json!(entry_id)))
                        .unwrap_or(false);
                    if !already && !entry_id.is_empty() {
                        let mut copied = m.clone();
                        if matches!(bucket.as_str(), "videos" | "audios") {
                            copy_material_file(&mut copied, template_dir, target_dir, entry_id)?;
                        }
                        bm.entry(bucket.to_string())
                            .or_insert_with(|| json!([]))
                            .as_array_mut()
                            .unwrap()
                            .push(copied);
                    }
                }
            }
        }
    }
    // append template tracks before the plan tracks? pyJYD imports tracks then
    // user adds segments; here plan tracks are appended after template tracks.
    if let Some(tt) = template["tracks"].as_array() {
        let mut tracks = tt.clone();
        tracks.extend(base["tracks"].as_array().cloned().unwrap_or_default());
        base["tracks"] = json!(tracks);
    }
    let duration = base["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["segments"].as_array().cloned())
        .flatten()
        .map(|s| {
            s["target_timerange"]["start"].as_i64().unwrap_or(0)
                + s["target_timerange"]["duration"].as_i64().unwrap_or(0)
        })
        .max()
        .unwrap_or(0);
    base["duration"] = json!(duration);
    Ok(())
}

//! Template mode (pyJianYingDraft parity): operate on an existing draft —
//! inspect materials, duplicate, replace text, replace materials, import a
//! track from another draft, and build on top of a template.

use crate::probe;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

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
    let s = serde_json::to_string_pretty(tl)?;
    std::fs::write(draft.join("draft_content.json"), &s)?;
    std::fs::write(draft.join("draft_info.json"), &s)?;
    Ok(())
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

/// `duplicate_as_template` parity: copy the draft under a new name and restamp.
pub fn duplicate(src: &Path, new_name: &str, root: Option<&Path>) -> Result<Value> {
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
    let mut target_id = String::new();
    if let Some(name) = by_name {
        for bucket in ["videos", "audios"] {
            if let Some(items) = tl["materials"][bucket].as_array_mut() {
                if let Some(m) = items.iter_mut().find(|m| {
                    m["material_name"].as_str() == Some(name) || m["name"].as_str() == Some(name)
                }) {
                    apply_material_swap(m, bucket, new_source, &info)?;
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
                    apply_material_swap(m, bucket, new_source, &info)?;
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
              "source": new_source.to_string_lossy(), "duration_us": info.duration_us}))
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
/// material ids and copying the referenced material entries.
pub fn import_track(target_draft: &Path, source_draft: &Path, track_name: &str) -> Result<Value> {
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
        let new_mid = copy_material(&source, &mut target, &old_mid)?;
        id_map.insert(old_mid.clone(), new_mid.clone());
        seg["material_id"] = json!(new_mid);
        if let Some(refs) = seg["extra_material_refs"].as_array_mut() {
            for r in refs.iter_mut() {
                let old = r.as_str().unwrap_or_default().to_string();
                let mapped = id_map.get(&old).cloned().unwrap_or_else(|| {
                    let new =
                        copy_material(&source, &mut target, &old).unwrap_or_else(|_| old.clone());
                    id_map.insert(old.clone(), new.clone());
                    new
                });
                *r = json!(mapped);
            }
        }
    }
    target["tracks"].as_array_mut().unwrap().push(new_track);
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
              "source": source_draft.to_string_lossy(),
              "segments": track["segments"].as_array().map(|a| a.len()).unwrap_or(0)}))
}

fn copy_material(source: &Value, target: &mut Value, material_id: &str) -> Result<String> {
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

/// `load_template` + add-segments parity: build the plan's tracks on top of a
/// template timeline, keeping its tracks and materials.
pub fn build_on_template(template_dir: &Path, base: &mut Value, plan_name: &str) -> Result<()> {
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
                        bm.entry(bucket.to_string())
                            .or_insert_with(|| json!([]))
                            .as_array_mut()
                            .unwrap()
                            .push(m.clone());
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

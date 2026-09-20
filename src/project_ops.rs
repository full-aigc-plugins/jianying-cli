//! Project-level inspection, comparison, migration and composition operations.

use anyhow::{bail, Context, Result};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

/// Inspect canonical draft siblings, divergence, layout and editor-write safety.
pub fn diagnose(draft: &Path, bundle: Option<&Path>) -> Result<Value> {
    use sha2::{Digest, Sha256};
    const FILES: &[&str] = &[
        "draft_content.json",
        "draft_info.json",
        "draft_meta_info.json",
        "template-2.tmp",
    ];
    if !draft.is_dir() {
        bail!("no draft found at {}", draft.display());
    }
    let mut candidates = Vec::new();
    let mut parsed = Vec::<(String, Value, String)>::new();
    for name in FILES {
        let path = draft.join(name);
        if !path.is_file() {
            candidates.push(json!({"file":name,"exists":false,"size":0,"mtime":null,
                "sha256":null,"parseable_timeline":false,"envelope":"root","timeline_hash":null,
                "app_version":null}));
            continue;
        }
        let metadata = std::fs::metadata(&path)?;
        let raw = std::fs::read_to_string(&path)?;
        let raw = raw.trim_start_matches('\u{feff}');
        let sha = format!("{:x}", Sha256::digest(raw.as_bytes()));
        let mtime = metadata.modified().ok().and_then(|modified| {
            let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
            time::OffsetDateTime::from_unix_timestamp_nanos(duration.as_nanos() as i128)
                .ok()?
                .format(&time::format_description::well_known::Rfc3339)
                .ok()
        });
        let value = serde_json::from_str::<Value>(raw);
        let timeline = value.as_ref().ok().and_then(find_timeline_envelope);
        let mut row = json!({"file":name,"exists":true,"size":metadata.len(),"mtime":mtime,
            "sha256":sha,"parseable_timeline":timeline.is_some(),"envelope":"root",
            "timeline_hash":null,"app_version":null});
        match (value, timeline) {
            (Ok(_), Some((timeline, envelope))) => {
                let timeline_hash = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_string(&timeline)?.as_bytes())
                );
                row["envelope"] = json!(if envelope.is_empty() {
                    "root".to_owned()
                } else {
                    envelope.join(".")
                });
                row["timeline_hash"] = json!(timeline_hash);
                row["tracks"] = json!(timeline["tracks"].as_array().map(Vec::len).unwrap_or(0));
                row["segments"] = json!(timeline["tracks"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|track| track["segments"].as_array().map(Vec::len).unwrap_or(0))
                    .sum::<usize>());
                row["app_version"] = timeline["platform"]["app_version"].clone();
                parsed.push(((*name).to_owned(), timeline, timeline_hash));
            }
            (Ok(_), None) => {
                row["error"] = json!("JSON file does not contain a recognizable timeline")
            }
            (Err(error), _) => row["error"] = json!(format!("JSON parse failed: {error}")),
        }
        candidates.push(row);
    }
    let canonical = ["draft_content.json", "draft_info.json", "template-2.tmp"]
        .iter()
        .find_map(|name| parsed.iter().find(|item| item.0 == *name))
        .context("no readable timeline in canonical draft files")?;
    let target_hashes: HashSet<&str> = parsed
        .iter()
        .filter(|item| item.0 != "draft_meta_info.json")
        .map(|item| item.2.as_str())
        .collect();
    let diverged = target_hashes.len() > 1;
    let version = canonical.1["platform"]["app_version"]
        .as_str()
        .map(str::to_owned);
    let modern_storage = version.as_deref().is_some_and(version_at_least_8_7);
    let nested_timelines = nested_timeline_paths(draft)?;
    let layout = if canonical.0 == "draft_content.json" {
        if !modern_storage && !nested_timelines.is_empty() {
            "timelines-nested"
        } else {
            "content-primary"
        }
    } else if canonical.0 == "draft_info.json" {
        "info-primary"
    } else {
        "unknown"
    };
    let editors = crate::store::editors_running();
    let mut actions = Vec::new();
    if diverged {
        actions.push("Timeline files diverge. Close JianYing/CapCut, back up the project, then review `jianying store sync` before --apply.".to_owned());
    }
    if !draft.join("draft_meta_info.json").is_file() {
        actions.push(
            "draft_meta_info.json is missing; review `jianying store register` before publishing."
                .to_owned(),
        );
    }
    if !editors.is_empty() {
        actions.push(format!(
            "Close {} before editing this managed draft.",
            editors.join(" / ")
        ));
    }
    if actions.is_empty() {
        actions.push(
            "Storage targets are readable and agree. A normal CLI write will synchronize them."
                .to_owned(),
        );
    }
    let report = json!({"ok":!diverged,"project_dir":"<project>","canonical":canonical.0,
        "version":version,"modern_storage":modern_storage,"diverged":diverged,"layout":layout,
        "nested_timelines":nested_timelines,"write_guard":"ok","editor_running":editors,
        "candidates":candidates,"next_actions":actions});
    if let Some(path) = bundle {
        let parent = path
            .parent()
            .context("diagnostic bundle requires a parent directory")?;
        std::fs::create_dir_all(parent)?;
        let temporary = parent.join(format!(".diagnose-{}.tmp", uuid::Uuid::new_v4().simple()));
        std::fs::write(
            &temporary,
            [serde_json::to_vec_pretty(&report)?, b"\n".to_vec()].concat(),
        )?;
        std::fs::rename(&temporary, path)?;
    }
    let mut output = report;
    output["bundle"] = bundle.map(|path| json!(path)).unwrap_or(Value::Null);
    Ok(output)
}

fn find_timeline_envelope(value: &Value) -> Option<(Value, Vec<String>)> {
    fn visit(value: &Value, path: Vec<String>, depth: usize) -> Option<(Value, Vec<String>)> {
        if value["tracks"].is_array() && value["materials"].is_object() {
            return Some((value.clone(), path));
        }
        if depth >= 3 {
            return None;
        }
        for (key, child) in value.as_object()? {
            if let Some(raw) = child.as_str().filter(|raw| raw.trim().starts_with('{')) {
                if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                    let mut next = path.clone();
                    next.push(format!("{key}:json"));
                    if let Some(found) = visit(&parsed, next, depth + 1) {
                        return Some(found);
                    }
                }
            } else if child.is_object() {
                let mut next = path.clone();
                next.push(key.clone());
                if let Some(found) = visit(child, next, depth + 1) {
                    return Some(found);
                }
            }
        }
        None
    }
    visit(value, Vec::new(), 0)
}

fn version_at_least_8_7(version: &str) -> bool {
    let mut parts = version
        .split('.')
        .filter_map(|part| part.parse::<u64>().ok());
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0)) >= (8, 7)
}

pub(crate) fn nested_timeline_paths(draft: &Path) -> Result<Vec<String>> {
    let root = draft.join("Timelines");
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut output = Vec::new();
    for entry in std::fs::read_dir(&root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.file_name().and_then(|name| name.to_str()) == Some("project.json")
        {
            output.push("Timelines/project.json".to_owned());
        } else if path.is_dir() {
            for name in ["draft_info.json", "draft_content.json"] {
                if path.join(name).is_file() {
                    output.push(format!(
                        "Timelines/{}/{}",
                        entry.file_name().to_string_lossy(),
                        name
                    ));
                }
            }
        }
    }
    output.sort();
    Ok(output)
}

/// Return the capcut-cli compatible project overview and material summary.
pub fn info(draft: &Path) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let tracks = timeline["tracks"].as_array().cloned().unwrap_or_default();
    let segments = tracks
        .iter()
        .map(|track| track["segments"].as_array().map(Vec::len).unwrap_or(0))
        .sum::<usize>();
    let material_summary: Vec<Value> = timeline["materials"]
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(kind, values)| {
            let count = values.as_array().map(Vec::len).unwrap_or(0);
            (count > 0).then(|| json!({"type":kind,"count":count}))
        })
        .collect();
    let platform = &timeline["platform"];
    let platform_name = match platform["app_source"].as_str() {
        Some("cc") => platform["app_version"]
            .as_str()
            .map(|version| format!("CapCut {version}")),
        Some("lv") => platform["app_version"]
            .as_str()
            .map(|version| format!("JianYing {version}")),
        _ => None,
    };
    Ok(json!({
        "id": timeline["id"],
        "name": timeline["name"].as_str().or_else(|| timeline["id"].as_str()),
        "duration_us": timeline["duration"],
        "fps": timeline["fps"],
        "width": timeline["canvas_config"]["width"],
        "height": timeline["canvas_config"]["height"],
        "ratio": timeline["canvas_config"]["ratio"],
        "tracks": tracks.len(),
        "segments": segments,
        "platform": platform_name,
        "material_types": timeline["materials"].as_object().map(Map::len).unwrap_or(0),
        "materials_with_items": material_summary.len(),
        "material_summary": material_summary
    }))
}

/// 设置项目封面图及其毫秒时间点，对应 capcut-cli `setCover`。
pub fn add_cover(draft: &Path, image: &Path, time_ms: i64) -> Result<Value> {
    if !image.exists() {
        bail!("cover image not found: {}", image.display());
    }
    if time_ms < 0 {
        bail!("cover time must be a non-negative integer in milliseconds");
    }
    let cover_path = image.to_string_lossy().into_owned();
    crate::timeline_ops::mutate(draft, |timeline| {
        timeline["cover"] = json!({
            "path": cover_path,
            "type": "image",
            "time": time_ms,
            "time_ms": time_ms,
            "custom_cover_id": uuid::Uuid::new_v4().to_string(),
        });
        Ok(json!({"ok":true,"cover_path":cover_path,"time_ms":time_ms}))
    })
}

/// 将时间窗提取为独立时间线 JSON。对应 capcut-cli `cutProject`。
pub fn cut(draft: &Path, start: i64, end: i64, out: &Path) -> Result<Value> {
    if end <= start {
        bail!("end time must be after start time");
    }
    let duration = end - start;
    let mut timeline = crate::draft::load_timeline(draft)?;
    let mut kept = 0usize;
    let mut removed = 0usize;
    let mut removed_primary = HashSet::new();
    let mut removed_extra = HashSet::new();

    for track in timeline["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?
    {
        let segments = track["segments"]
            .as_array_mut()
            .context("track segments must be an array")?;
        let mut surviving = Vec::with_capacity(segments.len());
        for mut segment in std::mem::take(segments) {
            let segment_start = segment["target_timerange"]["start"]
                .as_i64()
                .context("segment target_timerange.start must be an integer")?;
            let segment_duration = segment["target_timerange"]["duration"]
                .as_i64()
                .context("segment target_timerange.duration must be an integer")?;
            let segment_end = segment_start + segment_duration;
            if segment_end <= start || segment_start >= end {
                if let Some(id) = segment["material_id"].as_str() {
                    removed_primary.insert(id.to_owned());
                }
                for id in segment["extra_material_refs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                {
                    removed_extra.insert(id.to_owned());
                }
                removed += 1;
                continue;
            }

            let clipped_start = segment_start.max(start);
            let clipped_end = segment_end.min(end);
            let trim_from_start = clipped_start - segment_start;
            let clipped_duration = clipped_end - clipped_start;
            let speed = segment["speed"].as_f64().unwrap_or(1.0);
            if segment
                .get("source_timerange")
                .is_some_and(Value::is_object)
            {
                let source_start = segment["source_timerange"]["start"]
                    .as_i64()
                    .context("segment source_timerange.start must be an integer")?;
                segment["source_timerange"]["start"] =
                    json!(source_start + (trim_from_start as f64 * speed).round() as i64);
                segment["source_timerange"]["duration"] =
                    json!((clipped_duration as f64 * speed).round() as i64);
            }
            segment["target_timerange"]["start"] = json!(clipped_start - start);
            segment["target_timerange"]["duration"] = json!(clipped_duration);
            surviving.push(segment);
            kept += 1;
        }
        *segments = surviving;
    }
    timeline["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?
        .retain(|track| {
            track["segments"]
                .as_array()
                .is_some_and(|segments| !segments.is_empty())
        });

    let mut surviving_primary = HashSet::new();
    let mut surviving_extra = HashSet::new();
    for segment in timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
    {
        if let Some(id) = segment["material_id"].as_str() {
            surviving_primary.insert(id.to_owned());
        }
        surviving_extra.extend(
            segment["extra_material_refs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .map(str::to_owned),
        );
    }
    for materials in timeline["materials"]
        .as_object_mut()
        .context("materials must be an object")?
        .values_mut()
    {
        let Some(items) = materials.as_array_mut() else {
            continue;
        };
        items.retain(|material| {
            let Some(id) = material["id"].as_str() else {
                return true;
            };
            surviving_primary.contains(id)
                || surviving_extra.contains(id)
                || !(removed_primary.contains(id) || removed_extra.contains(id))
        });
    }
    timeline["duration"] = json!(duration);
    let wire = jianying_draft::DraftTimelineWire::from_value(timeline.clone())?;
    wire.resource_inventory()?;
    wire.validate_references()?;
    wire.validate_edit_semantics()?;

    let encoded = serde_json::to_vec(&timeline)?;
    let parent = out.parent().unwrap_or_else(|| Path::new("."));
    if !parent.is_dir() {
        bail!(
            "output parent directory does not exist: {}",
            parent.display()
        );
    }
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        out.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("cut"),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::write(&temporary, encoded)?;
    if out.exists() {
        std::fs::remove_file(out)?;
    }
    std::fs::rename(&temporary, out)
        .with_context(|| format!("committing extracted timeline to {}", out.display()))?;
    Ok(json!({
        "ok":true,"kept":kept,"removed":removed,
        "duration_us":duration,"out":out.to_string_lossy()
    }))
}

/// Detect wire markers and schema features while failing closed on write support.
pub fn version(draft: &Path) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let materials = timeline["materials"].as_object();
    let populated = |key: &str| {
        materials
            .and_then(|items| items.get(key))
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    };
    let mask_fields: Vec<&str> = ["masks", "common_mask", "common_masks"]
        .into_iter()
        .filter(|key| populated(key))
        .collect();
    let mask_field = match mask_fields.as_slice() {
        [] => "none",
        ["masks"] => "mask",
        [only] => only,
        _ => "both",
    };
    let has_text_ranges = materials
        .and_then(|items| items.get("texts"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .any(|material| {
            material["content"]
                .as_str()
                .and_then(|content| serde_json::from_str::<Value>(content).ok())
                .is_some_and(|content| content["styles"].as_array().is_some())
        });
    let platform = &timeline["platform"];
    let app_source = platform["app_source"].as_str().unwrap_or("unknown");
    let app = match app_source {
        "cc" => "CapCut",
        "lv" => "JianYing",
        _ => "unknown",
    };
    Ok(json!({
        "app": app,
        "app_source": app_source,
        "app_version": platform.get("app_version").cloned().unwrap_or(Value::Null),
        "os": platform.get("os").cloned().unwrap_or(Value::Null),
        "schema": {
            "mask_field": mask_field,
            "has_text_ranges": has_text_ranges,
            "has_audio_fades": materials.and_then(|items| items.get("audio_fades")).and_then(Value::as_array).is_some(),
            "new_version_field": timeline.get("new_version").cloned().unwrap_or(Value::Null),
            "last_modified_platform": timeline.get("last_modified_platform").cloned().unwrap_or(Value::Null),
            "schema_int": timeline.get("version").cloned().unwrap_or(Value::Null)
        },
        "support": {
            "status": "untested",
            "evidence": "runtime profile not frozen",
            "beyond_known_range": true,
            "write_guard": "block",
            "notes": ["Unknown or unverified runtime profile; mutating commands fail closed."]
        }
    }))
}

/// Compare segments, materials and track counts using capcut-cli semantics.
pub fn diff(left: &Path, right: &Path) -> Result<Value> {
    let left = crate::draft::load_timeline(left)?;
    let right = crate::draft::load_timeline(right)?;
    let left_segments = index_segments(&left);
    let right_segments = index_segments(&right);
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();
    for (id, segment) in &right_segments {
        let Some(previous) = left_segments.get(id) else {
            added.push(id.clone());
            continue;
        };
        let fields: Vec<&str> = [
            ("start", "/target_timerange/start"),
            ("duration", "/target_timerange/duration"),
            ("material_id", "/material_id"),
            ("content", "/content"),
            ("speed", "/speed"),
            ("volume", "/volume"),
        ]
        .into_iter()
        .filter_map(|(field, pointer)| {
            (previous.pointer(pointer) != segment.pointer(pointer)).then_some(field)
        })
        .collect();
        if !fields.is_empty() {
            changed.push(json!({"id":id,"fields":fields}));
        }
    }
    for id in left_segments.keys() {
        if !right_segments.contains_key(id) {
            removed.push(id.clone());
        }
    }
    let left_materials = index_materials(&left);
    let right_materials = index_materials(&right);
    let material_added: Vec<String> = right_materials
        .keys()
        .filter(|id| !left_materials.contains_key(*id))
        .cloned()
        .collect();
    let material_removed: Vec<String> = left_materials
        .keys()
        .filter(|id| !right_materials.contains_key(*id))
        .cloned()
        .collect();
    let material_changed: Vec<String> = right_materials
        .iter()
        .filter(|(id, value)| {
            left_materials
                .get(*id)
                .is_some_and(|previous| previous != *value)
        })
        .map(|(id, _)| id.clone())
        .collect();
    let has_changes = !added.is_empty()
        || !removed.is_empty()
        || !changed.is_empty()
        || !material_added.is_empty()
        || !material_removed.is_empty()
        || !material_changed.is_empty();
    Ok(json!({
        "ok": true,
        "changed": has_changes,
        "tracks": {
            "a": left["tracks"].as_array().map(Vec::len).unwrap_or(0),
            "b": right["tracks"].as_array().map(Vec::len).unwrap_or(0)
        },
        "segments": {"added":added,"removed":removed,"changed":changed},
        "materials": {"added":material_added,"removed":material_removed,"changed":material_changed}
    }))
}

/// Migrate the known mask material rename across the 9.6 schema boundary.
pub fn migrate(draft: &Path, from: &str, to: &str) -> Result<Value> {
    reject_running_editor()?;
    let mut transaction = begin_mutation(draft)?;
    let mut timeline = crate::draft::load_timeline(transaction.work_copy())?;
    let direction = migration_direction(from, to);
    let mut applied = Vec::new();
    let mut skipped = Vec::new();
    let mut warnings = Vec::new();
    match direction {
        Some((target, sources)) => {
            for source in sources {
                let moved = move_materials(&mut timeline, source, target)?;
                let source_label = if source == "masks" { "mask" } else { source };
                let label = format!("{source_label}->{target}");
                if moved == 0 {
                    skipped.push(format!(
                        "{label} (no `{source}[]` entries to migrate)"
                    ));
                } else {
                    applied.push(format!("{label} ({moved} entries)"));
                }
            }
        }
        None => warnings.push(format!(
            "No registered migration for {from} -> {to}. Only known migration so far: mask <-> common_masks across JianYing 5.9 / CapCut 9.6 boundary."
        )),
    }
    crate::template::save_timeline_as(transaction.work_copy(), &timeline, draft)?;
    validate_and_commit(&mut transaction)?;
    Ok(
        json!({"ok":true,"from":from,"to":to,"applied":applied,"skipped":skipped,"warnings":warnings}),
    )
}

/// Restamp schema markers from a distinct app-written donor draft.
pub fn restamp(draft: &Path, donor: &Path) -> Result<Value> {
    reject_running_editor()?;
    if draft.canonicalize().ok() == donor.canonicalize().ok() {
        bail!("the donor must be a different project than the draft being restamped");
    }
    let mut transaction = begin_mutation(draft)?;
    let mut timeline = crate::draft::load_timeline(transaction.work_copy())?;
    let donor_timeline = crate::draft::load_timeline(donor)?;
    let mut restamped = Vec::new();
    let mut added = Vec::new();
    let mut unchanged = Vec::new();
    let mut unavailable = Vec::new();
    for field in [
        "version",
        "new_version",
        "platform",
        "last_modified_platform",
        "color_space",
        "render_index_track_mode_on",
        "free_render_index_mode_on",
        "source",
    ] {
        let Some(wanted) = donor_timeline.get(field).cloned() else {
            unavailable.push(field);
            continue;
        };
        if timeline.get(field).is_none() {
            timeline[field] = wanted;
            added.push(field);
        } else if timeline[field] != wanted {
            timeline[field] = wanted;
            restamped.push(field);
        } else {
            unchanged.push(field);
        }
    }
    if timeline.get("config").is_none() {
        if let Some(config) = donor_timeline.get("config").cloned() {
            timeline["config"] = config;
            added.push("config");
        }
    }
    crate::template::save_timeline_as(transaction.work_copy(), &timeline, draft)?;
    validate_and_commit(&mut transaction)?;
    Ok(json!({
        "ok":true,
        "donor":donor,
        "donor_app_version":donor_timeline["platform"]["app_version"],
        "restamped":restamped,
        "added":added,
        "unchanged":unchanged,
        "unavailable":unavailable,
        "from":Value::Null,"to":Value::Null,"applied":[],"skipped":[],"warnings":[]
    }))
}

/// Select the newest other app-written draft in the same store and restamp from it.
pub fn restamp_from_store(draft: &Path) -> Result<Value> {
    let project_dir = draft.canonicalize().unwrap_or_else(|_| draft.to_path_buf());
    let root = project_dir
        .parent()
        .context("draft has no containing store directory")?;
    let mut candidates = Vec::new();
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let candidate = entry.path();
        if !candidate.is_dir() || candidate.canonicalize().ok() == Some(project_dir.clone()) {
            continue;
        }
        let Ok(timeline) = crate::draft::load_timeline(&candidate) else {
            continue;
        };
        let app_written = timeline.get("version").is_some()
            || timeline.get("new_version").is_some()
            || timeline.get("last_modified_platform").is_some();
        if !app_written {
            continue;
        }
        let modified = std::fs::metadata(candidate.join("draft_content.json"))
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        candidates.push((modified, candidate));
    }
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
    let donor = candidates
        .first()
        .map(|(_, path)| path)
        .context("no other readable app-written project is available for --from-store")?;
    restamp(draft, donor)
}

/// Append the second draft after the first while de-colliding ids and references.
pub fn concat(left: &Path, right: &Path, out: Option<&Path>) -> Result<Value> {
    if out.is_none() {
        reject_running_editor()?;
    }
    let mut combined = crate::draft::load_timeline(left)?;
    let mut appended = crate::draft::load_timeline(right)?;
    let offset = combined["duration"].as_i64().unwrap_or(0);
    let existing_material_ids: HashSet<String> = index_materials(&combined).into_keys().collect();
    let existing_segment_ids: HashSet<String> = index_segments(&combined).into_keys().collect();
    let mut remap = HashMap::new();
    for values in appended["materials"]
        .as_object_mut()
        .into_iter()
        .flatten()
        .map(|(_, value)| value)
    {
        for material in values.as_array_mut().into_iter().flatten() {
            if let Some(id) = material["id"].as_str().map(str::to_owned) {
                if existing_material_ids.contains(&id) {
                    let replacement = uuid::Uuid::new_v4().simple().to_string();
                    material["id"] = json!(replacement);
                    remap.insert(id, replacement);
                }
            }
        }
    }
    remap_known_references(&mut appended, &remap);
    for track in appended["tracks"].as_array_mut().into_iter().flatten() {
        for segment in track["segments"].as_array_mut().into_iter().flatten() {
            if segment["id"]
                .as_str()
                .is_some_and(|id| existing_segment_ids.contains(id))
            {
                segment["id"] = json!(uuid::Uuid::new_v4().simple().to_string());
            }
            if let Some(start) = segment["target_timerange"]["start"].as_i64() {
                segment["target_timerange"]["start"] = json!(start + offset);
            }
        }
    }
    merge_materials(&mut combined, &mut appended)?;
    merge_tracks(&mut combined, &mut appended)?;
    combined["duration"] = json!(offset + appended["duration"].as_i64().unwrap_or(0));
    let target = out.unwrap_or(left);
    if let Some(out) = out {
        copy_draft(left, out)?;
        crate::template::save_timeline(target, &combined)?;
    } else {
        let mut transaction = begin_mutation(left)?;
        crate::template::save_timeline_as(transaction.work_copy(), &combined, target)?;
        validate_and_commit(&mut transaction)?;
    }
    let location_key = if out.is_some() { "out" } else { "project" };
    let mut result = Map::new();
    result.insert("ok".to_owned(), json!(true));
    result.insert(location_key.to_owned(), json!(target));
    result.insert("duration_us".to_owned(), combined["duration"].clone());
    result.insert("remapped_ids".to_owned(), json!(remap.len()));
    Ok(Value::Object(result))
}

fn index_segments(timeline: &Value) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        for segment in track["segments"].as_array().into_iter().flatten() {
            if let Some(id) = segment["id"].as_str() {
                result.insert(id.to_owned(), segment.clone());
            }
        }
    }
    result
}

fn index_materials(timeline: &Value) -> BTreeMap<String, Value> {
    let mut result = BTreeMap::new();
    for values in timeline["materials"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(_, value)| value)
    {
        for material in values.as_array().into_iter().flatten() {
            if let Some(id) = material["id"].as_str() {
                result.insert(id.to_owned(), material.clone());
            }
        }
    }
    result
}

fn migration_direction(from: &str, to: &str) -> Option<(&'static str, [&'static str; 2])> {
    let parse = |value: &str| {
        value
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".")
            .parse::<f64>()
            .ok()
    };
    match (parse(from), parse(to)) {
        (Some(from), Some(to)) if from < 9.6 && to >= 9.6 => {
            Some(("common_masks", ["masks", "common_mask"]))
        }
        (Some(from), Some(to)) if from >= 9.6 && to < 9.6 => {
            Some(("masks", ["common_masks", "common_mask"]))
        }
        _ => None,
    }
}

fn move_materials(timeline: &mut Value, source: &str, target: &str) -> Result<usize> {
    let materials = timeline["materials"]
        .as_object_mut()
        .context("materials must be an object")?;
    let Some(source_value) = materials.get_mut(source) else {
        return Ok(0);
    };
    let source_items = source_value
        .as_array_mut()
        .context("mask material bucket must be an array")?;
    if source_items.is_empty() {
        return Ok(0);
    }
    let moved_items = std::mem::take(source_items);
    let target_items = materials
        .entry(target.to_owned())
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .context("target mask material bucket must be an array")?;
    let existing: BTreeSet<String> = target_items
        .iter()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect();
    let before = target_items.len();
    target_items.extend(
        moved_items
            .into_iter()
            .filter(|item| item["id"].as_str().is_none_or(|id| !existing.contains(id))),
    );
    Ok(target_items.len() - before)
}

fn remap_known_references(value: &mut Value, remap: &HashMap<String, String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key == "material_id"
                    || key == "extra_material_refs"
                    || key.ends_with("_material_id")
                {
                    remap_string_values(child, remap);
                } else {
                    remap_known_references(child, remap);
                }
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| remap_known_references(item, remap)),
        _ => {}
    }
}

fn remap_string_values(value: &mut Value, remap: &HashMap<String, String>) {
    match value {
        Value::String(current) => {
            if let Some(replacement) = remap.get(current) {
                *current = replacement.clone();
            }
        }
        Value::Array(items) => items
            .iter_mut()
            .for_each(|item| remap_string_values(item, remap)),
        _ => {}
    }
}

fn merge_materials(combined: &mut Value, appended: &mut Value) -> Result<()> {
    let source = appended["materials"]
        .as_object_mut()
        .context("materials must be an object")?;
    let destination = combined["materials"]
        .as_object_mut()
        .context("materials must be an object")?;
    for (kind, values) in source {
        let entries = std::mem::take(values)
            .as_array()
            .cloned()
            .unwrap_or_default();
        destination
            .entry(kind.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .context("material bucket must be an array")?
            .extend(entries);
    }
    Ok(())
}

fn merge_tracks(combined: &mut Value, appended: &mut Value) -> Result<()> {
    let source = appended["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?;
    let destination = combined["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?;
    for mut track in std::mem::take(source) {
        let matching = destination.iter_mut().find(|candidate| {
            candidate["type"] == track["type"] && candidate["name"] == track["name"]
        });
        if let Some(matching) = matching {
            let segments = std::mem::take(&mut track["segments"])
                .as_array()
                .cloned()
                .unwrap_or_default();
            matching["segments"]
                .as_array_mut()
                .context("track segments must be an array")?
                .extend(segments);
        } else {
            destination.push(track);
        }
    }
    Ok(())
}

fn reject_running_editor() -> Result<()> {
    let running = crate::store::editors_running();
    if !running.is_empty() {
        bail!(
            "editor is running ({}); close JianYing/CapCut before mutating drafts",
            running.join(", ")
        );
    }
    Ok(())
}

fn begin_mutation(draft: &Path) -> Result<jianying_store::MutationPlan> {
    let state_root = draft
        .parent()
        .unwrap_or(draft)
        .join(".jianying-transactions");
    let mut plan = jianying_store::MutationPlan::new(draft.to_path_buf(), state_root)?;
    plan.stage()?;
    Ok(plan)
}

fn validate_and_commit(plan: &mut jianying_store::MutationPlan) -> Result<()> {
    let identity = plan.source().to_path_buf();
    plan.validate(|work_copy| {
        crate::draft::validate_bundle_as(work_copy, &identity)
            .map_err(|error| jianying_store::StoreError::Validation(format!("{error:#}")))
    })?;
    plan.commit()?;
    Ok(())
}

fn copy_draft(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        bail!("output {} already exists", target.display());
    }
    copy_directory(source, target)
}

fn copy_directory(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_directory(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

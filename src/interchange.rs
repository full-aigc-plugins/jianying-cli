//! OpenTimelineIO 时间线交接。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

#[derive(Clone)]
struct ImportClip {
    name: String,
    target_start_us: i64,
    target_duration_us: i64,
    source_start_us: i64,
    source_duration_us: i64,
    speed: f64,
    volume: Option<f64>,
    media_path: Option<String>,
    media_duration_us: i64,
}

#[derive(Clone)]
struct ImportTrack {
    kind: String,
    name: String,
    clips: Vec<ImportClip>,
}

struct ImportCaption {
    text: String,
    start_us: i64,
    duration_us: i64,
    track: String,
}

struct ImportPlan {
    name: String,
    rate: f64,
    tracks: Vec<ImportTrack>,
    captions: Vec<ImportCaption>,
    gaps: usize,
    skipped: Vec<Value>,
}

/// 将草稿导出为 OTIO Timeline.1 文档或文件摘要。
///
/// `captions` 仅接受 `skip` 或 `markers`；后者把文字片段写入 Stack markers。
pub fn export_timeline(
    draft: &Path,
    output: Option<&Path>,
    captions: &str,
    quiet: bool,
) -> Result<Value> {
    if !matches!(captions, "skip" | "markers") {
        bail!("--captions must be skip|markers (got {captions})")
    }
    let timeline = crate::draft::load_timeline(draft)?;
    let rate = timeline["fps"]
        .as_f64()
        .filter(|rate| rate.is_finite() && *rate > 0.0)
        .unwrap_or(30.0);
    let mut children = Vec::new();
    let mut markers = Vec::new();
    let mut skipped = Vec::new();
    let mut track_count = 0usize;
    let mut clip_count = 0usize;
    let mut gap_count = 0usize;
    let mut caption_count = 0usize;

    for track in timeline["tracks"].as_array().into_iter().flatten() {
        let track_type = track["type"].as_str().unwrap_or_default();
        let track_name = track["name"].as_str().unwrap_or_default();
        let Some(kind) = (match track_type {
            "video" => Some("Video"),
            "audio" => Some("Audio"),
            _ => None,
        }) else {
            if track_type == "text" && captions == "markers" {
                let mut segments = track["segments"].as_array().cloned().unwrap_or_default();
                segments.sort_by_key(|segment| segment["target_timerange"]["start"].as_i64());
                for segment in segments {
                    let Some(text) = text_for_segment(&timeline, &segment) else {
                        continue;
                    };
                    let start = frames_for(
                        segment["target_timerange"]["start"].as_i64().unwrap_or(0),
                        rate,
                    );
                    let duration = frames_for(
                        segment["target_timerange"]["duration"]
                            .as_i64()
                            .unwrap_or(0),
                        rate,
                    )
                    .max(1);
                    markers.push(json!({
                        "OTIO_SCHEMA":"Marker.1",
                        "color":"YELLOW",
                        "marked_range":time_range(start,duration,rate),
                        "metadata":{
                            "Resolve_OTIO":{"Keywords":[],"Note":text},
                            "capcut":{"kind":"caption","text":text,"track":track_name,
                                "track_id":track["id"],"segment_id":segment["id"],
                                "material_id":segment["material_id"]}
                        },
                        "name":text
                    }));
                    caption_count += 1;
                }
                continue;
            }
            let reason = if track_type == "text" {
                "OTIO has no standard title schema — pass --captions markers to carry cues as timeline markers, or export them with `capcut export-srt` and re-attach them in the NLE"
            } else {
                "no portable OTIO equivalent for this track type"
            };
            skipped.push(json!({"track":track_name,"type":track_type,"reason":reason}));
            continue;
        };
        let mut segments = track["segments"].as_array().cloned().unwrap_or_default();
        if segments.is_empty() {
            continue;
        }
        segments.sort_by_key(|segment| segment["target_timerange"]["start"].as_i64());
        let mut items = Vec::new();
        let mut cursor_us = 0i64;
        for segment in segments {
            let start_us = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
            let duration_us = segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            let gap_frames = frames_for(start_us - cursor_us, rate);
            if gap_frames > 0 {
                items.push(gap(gap_frames, rate));
                gap_count += 1;
            }
            items.push(clip(&timeline, &segment, rate));
            clip_count += 1;
            cursor_us = start_us + duration_us;
        }
        children.push(json!({
            "OTIO_SCHEMA":"Track.1","children":items,"effects":[],"kind":kind,
            "markers":[],"metadata":{"capcut":{"track_id":track["id"]}},
            "name":track_name,"source_range":null
        }));
        track_count += 1;
    }

    let document = json!({
        "OTIO_SCHEMA":"Timeline.1",
        "global_start_time":rational_time(0,rate),
        "metadata":{"capcut":{"draft_id":timeline["id"],"duration_us":timeline["duration"],
            "exported_by":"capcut-cli export-timeline"}},
        "name":timeline["name"].as_str().filter(|name| !name.is_empty()).unwrap_or("capcut draft"),
        "tracks":{"OTIO_SCHEMA":"Stack.1","children":children,"effects":[],"markers":markers,
            "metadata":{},"name":"tracks","source_range":null}
    });
    if !quiet {
        for entry in &skipped {
            eprintln!(
                "skipped track \"{}\" ({}): {}",
                entry["track"].as_str().unwrap_or_default(),
                entry["type"].as_str().unwrap_or_default(),
                entry["reason"].as_str().unwrap_or_default()
            );
        }
    }
    let Some(output) = output else {
        return Ok(document);
    };
    let serialized = serde_json::to_string_pretty(&document)? + "\n";
    std::fs::write(output, serialized)
        .with_context(|| format!("cannot write OTIO output {}", output.display()))?;
    let mut summary = json!({"ok":true,"out":output,"tracks":track_count,"clips":clip_count,
        "gaps":gap_count,"skipped":skipped});
    if captions == "markers" {
        summary["captions"] = json!(caption_count);
    }
    Ok(summary)
}

/// 将 OTIO Timeline.1 导入新草稿或以单事务追加到已有草稿。
pub fn import_timeline(
    file: &Path,
    output: Option<&Path>,
    into: Option<&Path>,
    dry_run: bool,
    quiet: bool,
) -> Result<Value> {
    let target = match (output, into) {
        (Some(_), Some(_)) => bail!("--out and --into are mutually exclusive"),
        (None, None) => bail!("pass exactly one of --out or --into"),
        (Some(path), None) | (None, Some(path)) => path,
    };
    let bytes =
        std::fs::read(file).with_context(|| format!("OTIO file not found: {}", file.display()))?;
    let document: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("import-timeline: {} is not valid JSON", file.display()))?;
    let plan = parse_import_plan(&document)?;
    if dry_run {
        return Ok(import_summary(&plan, target, output.is_some(), true, &[]));
    }
    if let Some(output) = output {
        create_empty_draft(output, &plan.name, plan.rate)?;
    }
    let fresh = output.is_some();
    let placeholders = crate::timeline_ops::mutate_with_path(target, |timeline, work, identity| {
        if fresh {
            timeline["tracks"] = json!([]);
            for bucket in ["videos", "audios", "texts"] {
                timeline["materials"][bucket] = json!([]);
            }
            timeline["name"] = json!(plan.name);
            timeline["fps"] = json!(plan.rate);
        }
        apply_import_plan(timeline, work, identity, &plan)
    });
    let placeholders = match placeholders {
        Ok(value) => value.as_array().cloned().unwrap_or_default(),
        Err(error) => {
            if output.is_some() {
                let _ = std::fs::remove_dir_all(target);
            }
            return Err(error);
        }
    };
    if !quiet {
        for skip in &plan.skipped {
            eprintln!(
                "skipped in \"{}\" ({}): {}",
                skip["track"].as_str().unwrap_or_default(),
                skip["type"].as_str().unwrap_or_default(),
                skip["reason"].as_str().unwrap_or_default()
            );
        }
        for placeholder in &placeholders {
            eprintln!(
                "placeholder: \"{}\" on track \"{}\" ({}) — swap in the file with `jianying media replace`",
                placeholder["clip"].as_str().unwrap_or_default(),
                placeholder["track"].as_str().unwrap_or_default(),
                placeholder["path"].as_str().unwrap_or("no media reference")
            );
        }
    }
    Ok(import_summary(&plan, target, fresh, false, &placeholders))
}

fn create_empty_draft(output: &Path, name: &str, rate: f64) -> Result<()> {
    if output.exists() {
        bail!("output already exists: {}", output.display());
    }
    let plan: crate::plan::Plan = serde_json::from_value(json!({
        "schema":crate::plan::SCHEMA,
        "name":if name.is_empty() { output.file_name().unwrap_or_default().to_string_lossy() } else { name.into() },
        "canvas":{"width":1920,"height":1080,"fps":rate.round() as u64},
        "tracks":[{"type":"text","name":"text","segments":[]}]
    }))?;
    crate::draft::build(&plan, Path::new("."), output, None, &|path| {
        crate::probe::probe(path)
    })?;
    Ok(())
}

fn import_summary(
    plan: &ImportPlan,
    target: &Path,
    fresh: bool,
    dry_run: bool,
    placeholders: &[Value],
) -> Value {
    let tracks = plan
        .tracks
        .iter()
        .filter(|track| !track.clips.is_empty())
        .count()
        + plan
            .captions
            .iter()
            .map(|caption| &caption.track)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
    let clips = plan
        .tracks
        .iter()
        .map(|track| track.clips.len())
        .sum::<usize>();
    let duration = plan
        .tracks
        .iter()
        .flat_map(|track| &track.clips)
        .map(|clip| clip.target_start_us + clip.target_duration_us)
        .chain(
            plan.captions
                .iter()
                .map(|cue| cue.start_us + cue.duration_us),
        )
        .max()
        .unwrap_or(0);
    let mut value = json!({"ok":true,"mode":if fresh {"out"} else {"into"},
        "draft_path":target,"file_path":target.join("draft_content.json"),
        "tracks":tracks,"clips":clips,"gaps":plan.gaps,
        "placeholders":placeholders,"duration_us":duration,"skipped":plan.skipped});
    if !plan.captions.is_empty() {
        value["captions"] = json!(plan.captions.len());
    }
    if dry_run {
        value["dryRun"] = json!(true);
    }
    value
}

fn apply_import_plan(
    timeline: &mut Value,
    work: &Path,
    identity: &Path,
    plan: &ImportPlan,
) -> Result<Value> {
    let mut claimed = timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|track| {
            format!(
                "{}\0{}",
                track["type"].as_str().unwrap_or_default(),
                track["name"].as_str().unwrap_or_default()
            )
        })
        .collect::<std::collections::HashSet<_>>();
    let mut placeholders = Vec::new();
    for source_track in &plan.tracks {
        if source_track.clips.is_empty() {
            continue;
        }
        let name = claim_track_name(&mut claimed, &source_track.kind, &source_track.name);
        let mut segments = Vec::new();
        for clip in &source_track.clips {
            let material_id = Uuid::new_v4().simple().to_string();
            let segment_id = Uuid::new_v4().simple().to_string();
            let (stored_path, placeholder) = match clip.media_path.as_deref() {
                Some(path) if Path::new(path).is_file() => {
                    let copied =
                        crate::draft::copy_asset(work, &source_track.kind, Path::new(path))?;
                    let stored = identity
                        .join(
                            Path::new(&copied)
                                .strip_prefix(work)
                                .unwrap_or(Path::new(&copied)),
                        )
                        .to_string_lossy()
                        .into_owned();
                    (stored, false)
                }
                Some(_) => (String::new(), true),
                None => (String::new(), true),
            };
            if placeholder {
                placeholders.push(json!({"track":name,"clip":clip.name,"path":clip.media_path}));
            }
            let bucket = if source_track.kind == "video" {
                "videos"
            } else {
                "audios"
            };
            let mut material = json!({"id":material_id,"path":stored_path,"duration":clip.media_duration_us,
                "type":source_track.kind});
            if source_track.kind == "video" {
                material["material_name"] = json!(clip.name);
                material["width"] = json!(0);
                material["height"] = json!(0);
            } else {
                material["name"] = json!(clip.name);
            }
            timeline["materials"][bucket]
                .as_array_mut()
                .with_context(|| format!("materials.{bucket} must be an array"))?
                .push(material);
            segments.push(json!({"id":segment_id,"material_id":material_id,
                "target_timerange":{"start":clip.target_start_us,"duration":clip.target_duration_us},
                "source_timerange":{"start":clip.source_start_us,"duration":clip.source_duration_us},
                "speed":clip.speed,"volume":clip.volume.unwrap_or(1.0),"visible":true,
                "reverse":false,"render_index":0,"extra_material_refs":[],"common_keyframes":[],
                "keyframe_refs":[],"clip":{"alpha":1.0,"rotation":0.0,
                    "scale":{"x":1.0,"y":1.0},"transform":{"x":0.0,"y":0.0},
                    "flip":{"horizontal":false,"vertical":false}}}));
        }
        timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .push(json!({
                "attribute":0,"flag":0,"id":Uuid::new_v4().simple().to_string(),
                "is_default_name":false,"name":name,"segments":segments,"type":source_track.kind
            }));
    }
    for cue in &plan.captions {
        crate::caption_ops::add_to_timeline(
            timeline,
            &cue.text,
            cue.start_us,
            cue.duration_us,
            &cue.track,
            None,
        )?;
    }
    Ok(Value::Array(placeholders))
}

fn claim_track_name(
    claimed: &mut std::collections::HashSet<String>,
    kind: &str,
    base: &str,
) -> String {
    let base = if base.is_empty() { kind } else { base };
    let mut name = base.to_owned();
    let mut suffix = 2;
    while claimed.contains(&format!("{kind}\0{name}")) {
        name = format!("{base} ({suffix})");
        suffix += 1;
    }
    claimed.insert(format!("{kind}\0{name}"));
    name
}

fn parse_import_plan(document: &Value) -> Result<ImportPlan> {
    if document["OTIO_SCHEMA"].as_str() != Some("Timeline.1") {
        bail!("import-timeline: not an OpenTimelineIO Timeline.1 document")
    }
    let stack = &document["tracks"];
    if stack["OTIO_SCHEMA"].as_str() != Some("Stack.1") || !stack["children"].is_array() {
        bail!("import-timeline: timeline has no Stack.1 tracks container")
    }
    let rate = document["global_start_time"]["rate"]
        .as_f64()
        .filter(|rate| *rate > 0.0)
        .unwrap_or(30.0);
    let mut skipped = Vec::new();
    if document["global_start_time"]["value"]
        .as_f64()
        .unwrap_or(0.0)
        != 0.0
    {
        skipped.push(json!({"track":"(timeline)","type":"global_start_time",
            "reason":"non-zero timeline start is ignored — CapCut drafts start at 0"}));
    }
    let mut captions = Vec::new();
    let mut foreign_markers = 0usize;
    for marker in stack["markers"].as_array().into_iter().flatten() {
        if marker["metadata"]["capcut"]["kind"].as_str() != Some("caption") {
            foreign_markers += 1;
            continue;
        }
        let (start_us, duration_us) = range_us(&marker["marked_range"], rate)?;
        let text = marker["metadata"]["capcut"]["text"]
            .as_str()
            .or_else(|| marker["name"].as_str())
            .unwrap_or_default();
        if !text.is_empty() {
            captions.push(ImportCaption {
                text: text.to_owned(),
                start_us,
                duration_us: duration_us.max(1),
                track: marker["metadata"]["capcut"]["track"]
                    .as_str()
                    .unwrap_or("captions")
                    .to_owned(),
            });
        }
    }
    if foreign_markers > 0 {
        skipped.push(json!({"track":"(timeline)","type":"markers",
            "reason":format!("{foreign_markers} timeline marker(s) without capcut caption metadata have no CapCut equivalent")}));
    }
    let mut tracks = Vec::new();
    let mut gaps = 0usize;
    for node in stack["children"].as_array().into_iter().flatten() {
        let label = node["name"].as_str().unwrap_or_default();
        if node["OTIO_SCHEMA"].as_str() != Some("Track.1") {
            skipped.push(json!({"track":label,"type":node["OTIO_SCHEMA"],
                "reason":"unsupported stack child — only Track.1 imports"}));
            continue;
        }
        let kind = match node["kind"].as_str() {
            Some("Video") => "video",
            Some("Audio") => "audio",
            other => {
                skipped.push(json!({"track":label,"type":other.unwrap_or("unknown"),
                    "reason":"unsupported track kind — only Video and Audio tracks import"}));
                continue;
            }
        };
        let mut clips = Vec::new();
        let mut cursor = 0i64;
        for item in node["children"].as_array().into_iter().flatten() {
            match item["OTIO_SCHEMA"].as_str() {
                Some("Gap.1") => {
                    cursor += range_us(&item["source_range"], rate)?.1;
                    gaps += 1;
                }
                Some("Clip.1") => {
                    let (source_start_us, source_duration_us) =
                        range_us(&item["source_range"], rate)?;
                    let speed = item["effects"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .find(|effect| effect["OTIO_SCHEMA"].as_str() == Some("LinearTimeWarp.1"))
                        .and_then(|effect| effect["time_scalar"].as_f64())
                        .filter(|value| *value > 0.0)
                        .unwrap_or(1.0);
                    let duration = (source_duration_us as f64 / speed).round() as i64;
                    let reference = &item["media_reference"];
                    let media_path = (reference["OTIO_SCHEMA"].as_str()
                        == Some("ExternalReference.1"))
                    .then(|| reference["target_url"].as_str().map(str::to_owned))
                    .flatten();
                    let media_duration_us = if reference["available_range"].is_object() {
                        range_us(&reference["available_range"], rate)?.1
                    } else {
                        0
                    };
                    clips.push(ImportClip {
                        name: item["name"].as_str().unwrap_or("clip").to_owned(),
                        target_start_us: cursor,
                        target_duration_us: duration,
                        source_start_us,
                        source_duration_us,
                        speed,
                        volume: item["metadata"]["capcut"]["volume"].as_f64(),
                        media_path,
                        media_duration_us,
                    });
                    cursor += duration;
                }
                other => skipped.push(json!({"track":label,"type":other.unwrap_or("unknown"),
                    "reason":"unsupported timeline item — only Clip.1 and Gap.1 import"})),
            }
        }
        tracks.push(ImportTrack {
            kind: kind.to_owned(),
            name: label.to_owned(),
            clips,
        });
    }
    Ok(ImportPlan {
        name: document["name"].as_str().unwrap_or_default().to_owned(),
        rate,
        tracks,
        captions,
        gaps,
        skipped,
    })
}

fn range_us(range: &Value, fallback_rate: f64) -> Result<(i64, i64)> {
    if range["OTIO_SCHEMA"].as_str() != Some("TimeRange.1") {
        bail!("import-timeline: range is not a TimeRange.1")
    }
    Ok((
        rational_us(&range["start_time"], fallback_rate)?,
        rational_us(&range["duration"], fallback_rate)?,
    ))
}

fn rational_us(value: &Value, fallback_rate: f64) -> Result<i64> {
    if value["OTIO_SCHEMA"].as_str() != Some("RationalTime.1") {
        bail!("import-timeline: value is not a RationalTime.1")
    }
    let rate = value["rate"]
        .as_f64()
        .filter(|rate| *rate > 0.0)
        .unwrap_or(fallback_rate);
    let frames = value["value"]
        .as_f64()
        .context("RationalTime value must be numeric")?;
    Ok((frames / rate * 1_000_000.0).round() as i64)
}

fn clip(timeline: &Value, segment: &Value, rate: f64) -> Value {
    let material = media_for_segment(timeline, segment);
    let speed = segment["speed"]
        .as_f64()
        .filter(|speed| *speed > 0.0)
        .unwrap_or(1.0);
    let effects = if (speed - 1.0).abs() > f64::EPSILON {
        vec![
            json!({"OTIO_SCHEMA":"LinearTimeWarp.1","effect_name":"LinearTimeWarp",
            "metadata":{},"name":"speed","time_scalar":speed}),
        ]
    } else {
        Vec::new()
    };
    let path = material
        .as_ref()
        .and_then(|value| value["path"].as_str())
        .unwrap_or_default();
    let material_name = material
        .as_ref()
        .and_then(|value| {
            value["material_name"]
                .as_str()
                .or_else(|| value["name"].as_str())
        })
        .map(str::to_owned)
        .or_else(|| {
            (!path.is_empty()).then(|| {
                Path::new(path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            })
        })
        .unwrap_or_else(|| {
            segment["material_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        });
    let media_reference = if path.is_empty() {
        json!({"OTIO_SCHEMA":"MissingReference.1","metadata":{},"name":material_name})
    } else {
        let duration = material
            .as_ref()
            .and_then(|value| value["duration"].as_i64())
            .unwrap_or(0);
        json!({"OTIO_SCHEMA":"ExternalReference.1",
            "available_range":if duration > 0 { time_range(0,frames_for(duration,rate),rate) } else { Value::Null },
            "metadata":{},"name":material_name,"target_url":path})
    };
    json!({
        "OTIO_SCHEMA":"Clip.1","effects":effects,"markers":[],"media_reference":media_reference,
        "metadata":{"capcut":{"material_id":segment["material_id"],"segment_id":segment["id"],
            "speed":speed,"volume":segment["volume"]}},
        "name":material_name,
        "source_range":time_range(
            frames_for(segment["source_timerange"]["start"].as_i64().unwrap_or(0),rate),
            frames_for(segment["source_timerange"]["duration"].as_i64().unwrap_or(0),rate),rate)
    })
}

fn media_for_segment<'a>(timeline: &'a Value, segment: &Value) -> Option<&'a Value> {
    let material_id = segment["material_id"].as_str()?;
    ["videos", "audios"].into_iter().find_map(|kind| {
        timeline["materials"][kind]
            .as_array()?
            .iter()
            .find(|material| material["id"].as_str() == Some(material_id))
    })
}

fn text_for_segment(timeline: &Value, segment: &Value) -> Option<String> {
    let material_id = segment["material_id"].as_str()?;
    let material = timeline["materials"]["texts"]
        .as_array()?
        .iter()
        .find(|material| material["id"].as_str() == Some(material_id))?;
    let content = material["content"].as_str()?;
    serde_json::from_str::<Value>(content)
        .ok()
        .and_then(|value| value["text"].as_str().map(str::to_owned))
        .or_else(|| (!content.is_empty()).then(|| content.to_owned()))
}

fn frames_for(microseconds: i64, rate: f64) -> i64 {
    let frames = ((microseconds as f64 / 1_000_000.0) * rate).round() as i64;
    if frames == 0 && microseconds > 0 {
        1
    } else {
        frames
    }
}

fn rational_time(value: i64, rate: f64) -> Value {
    json!({"OTIO_SCHEMA":"RationalTime.1","rate":rate,"value":value})
}

fn time_range(start: i64, duration: i64, rate: f64) -> Value {
    json!({"OTIO_SCHEMA":"TimeRange.1","duration":rational_time(duration,rate),
        "start_time":rational_time(start,rate)})
}

fn gap(duration: i64, rate: f64) -> Value {
    json!({"OTIO_SCHEMA":"Gap.1","effects":[],"markers":[],"metadata":{},"name":"",
        "source_range":time_range(0,duration,rate)})
}

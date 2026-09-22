//! 轨道和片段的查询与事务化编辑。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::Path;

/// 单个关键帧写入请求。
#[derive(Debug, Clone)]
pub struct KeyframeInput {
    pub property: String,
    pub time_us: i64,
    pub value: f64,
    pub easing: Option<String>,
}

/// 视频或图片片段的入场、出场与组合动画参数。
pub struct ImageAnimationOptions<'a> {
    pub intro: Option<&'a str>,
    pub outro: Option<&'a str>,
    pub combo: Option<&'a str>,
    pub intro_duration_us: Option<i64>,
    pub outro_duration_us: Option<i64>,
    pub combo_duration_us: Option<i64>,
    pub jianying: bool,
}

const KEYFRAME_PROPERTIES: &[(&str, &str)] = &[
    ("position_x", "KFTypePositionX"),
    ("position_y", "KFTypePositionY"),
    ("rotation", "KFTypeRotation"),
    ("scale_x", "KFTypeScaleX"),
    ("scale_y", "KFTypeScaleY"),
    ("uniform_scale", "UNIFORM_SCALE"),
    ("alpha", "KFTypeAlpha"),
    ("saturation", "KFTypeSaturation"),
    ("contrast", "KFTypeContrast"),
    ("brightness", "KFTypeBrightness"),
    ("volume", "KFTypeVolume"),
];

/// 解析关键帧属性别名和值表示。
pub fn parse_keyframe_value(property: &str, raw: &str) -> Result<(String, f64)> {
    let property = match property.trim() {
        "scale" => "uniform_scale",
        "x" => "position_x",
        "y" => "position_y",
        "opacity" => "alpha",
        other => other,
    };
    if !KEYFRAME_PROPERTIES
        .iter()
        .any(|(name, _)| *name == property)
    {
        bail!("unsupported keyframe property: {property}");
    }
    let value = raw.trim();
    let parsed = if property == "rotation" {
        value.strip_suffix("deg").unwrap_or(value).parse::<f64>()?
    } else if matches!(property, "alpha" | "volume") && value.ends_with('%') {
        value[..value.len() - 1].parse::<f64>()? / 100.0
    } else {
        value.parse::<f64>()?
    };
    if !parsed.is_finite() {
        bail!("invalid {property} value: {raw}");
    }
    if matches!(property, "position_x" | "position_y") && !(-10.0..=10.0).contains(&parsed) {
        bail!("{property} must be a finite number in [-10, 10], got: {raw}");
    }
    Ok((property.to_owned(), parsed))
}

/// 返回完整轨道布局，并按指定列数计算每个片段的可视区间。
pub fn show(draft: &Path, cols: u64) -> Result<Value> {
    if cols == 0 {
        bail!("cols must be greater than zero");
    }
    let timeline = crate::draft::load_timeline(draft)?;
    let mut span = timeline["duration"].as_i64().unwrap_or(0).max(1);
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        for segment in track["segments"].as_array().into_iter().flatten() {
            let end = segment["target_timerange"]["start"].as_i64().unwrap_or(0)
                + segment["target_timerange"]["duration"]
                    .as_i64()
                    .unwrap_or(0);
            span = span.max(end);
        }
    }
    let scale =
        |microseconds: i64| ((microseconds as f64 / span as f64) * cols as f64).round() as i64;
    let tracks = timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|track| {
            let segments = track["segments"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|segment| {
                    let start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
                    let duration = segment["target_timerange"]["duration"]
                        .as_i64()
                        .unwrap_or(0);
                    let col_start = scale(start);
                    let col_end = (col_start + 1).max(scale(start + duration));
                    json!({"id":segment["id"],"start_us":start,"duration_us":duration,
                        "col_start":col_start,"col_end":col_end})
                })
                .collect::<Vec<_>>();
            json!({"type":track["type"],"name":track["name"],"segments":segments})
        })
        .collect::<Vec<_>>();
    Ok(json!({"ok":true,"span_us":span,"cols":cols,"tracks":tracks}))
}

/// 列出轨道摘要，字段与固定 capcut-cli 基线一致。
pub fn tracks(draft: &Path) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let tracks = timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?;
    Ok(Value::Array(
        tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let duration = track["segments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(segment_end)
                    .max()
                    .unwrap_or(0);
                let attribute = track["attribute"].as_i64().unwrap_or(0);
                json!({
                    "index":index,"id":track["id"],"type":track["type"],"name":track["name"],
                    "segments":track["segments"].as_array().map(Vec::len).unwrap_or(0),
                    "duration_us":duration,"muted":attribute & 1 != 0,
                    "hidden":attribute & 2 != 0,"locked":attribute & 4 != 0
                })
            })
            .collect(),
    ))
}

/// 列出全部片段，可按轨道类型过滤。
pub fn segments(draft: &Path, track_type: Option<&str>) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let mut output = Vec::new();
    let mut matched = false;
    for track in timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?
    {
        if track_type.is_some_and(|wanted| track["type"].as_str() != Some(wanted)) {
            continue;
        }
        matched = true;
        for segment in track["segments"].as_array().into_iter().flatten() {
            output.push(segment_summary(&timeline, track, segment));
        }
    }
    if track_type.is_some() && !matched {
        bail!("no tracks of type {:?}", track_type.unwrap_or_default());
    }
    Ok(Value::Array(output))
}

/// 返回单个片段、所属轨道及主素材的完整无损 JSON。
pub fn segment(draft: &Path, id: &str) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    for track in timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?
    {
        for item in track["segments"].as_array().into_iter().flatten() {
            if item["id"].as_str() == Some(id) {
                let mut detail = item.clone();
                let object = detail
                    .as_object_mut()
                    .context("segment must be an object")?;
                object.insert("_track_type".into(), track["type"].clone());
                object.insert("_track_name".into(), track["name"].clone());
                object.insert("_track_id".into(), track["id"].clone());
                object.insert(
                    "_material".into(),
                    find_material(&timeline, item["material_id"].as_str().unwrap_or_default()),
                );
                return Ok(detail);
            }
        }
    }
    bail!("segment not found: {id}")
}

/// 平移单个片段，负值会在时间线零点截断。
pub fn move_segment(draft: &Path, id: &str, offset_us: i64) -> Result<Value> {
    mutate(draft, |timeline| {
        let segment = find_segment_mut(timeline, id)?;
        let old = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
        let new = (old + offset_us).max(0);
        segment["target_timerange"]["start"] = json!(new);
        Ok(json!({"ok":true,"id":id,"old_start_us":old,"new_start_us":new}))
    })
}

/// 平移全部或指定类型轨道的片段。
pub fn move_all(draft: &Path, offset_us: i64, track_type: Option<&str>) -> Result<Value> {
    mutate(draft, |timeline| {
        let mut count = 0;
        for track in timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
        {
            if track_type.is_some_and(|wanted| track["type"].as_str() != Some(wanted)) {
                continue;
            }
            for segment in track["segments"].as_array_mut().into_iter().flatten() {
                let start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
                segment["target_timerange"]["start"] = json!((start + offset_us).max(0));
                count += 1;
            }
        }
        Ok(json!({"ok":true,"shifted":count,"offset_us":offset_us}))
    })
}

/// 设置片段播放速度并同步 speed 素材。
pub fn speed(draft: &Path, id: &str, speed: f64) -> Result<Value> {
    if !speed.is_finite() || speed <= 0.0 {
        bail!("speed must be a positive finite number");
    }
    mutate(draft, |timeline| {
        let (old, refs, target_duration) = {
            let segment = find_segment_mut(timeline, id)?;
            let old = segment["speed"].as_f64().unwrap_or(1.0);
            let target_duration = segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            let refs = segment["extra_material_refs"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            segment["speed"] = json!(speed);
            segment["source_timerange"]["duration"] =
                json!((target_duration as f64 * speed).round() as i64);
            (old, refs, target_duration)
        };
        for reference in refs {
            if let Some(material) = timeline["materials"]["speeds"]
                .as_array_mut()
                .and_then(|items| items.iter_mut().find(|item| item["id"] == reference))
            {
                material["speed"] = json!(speed);
            }
        }
        let _ = target_duration;
        Ok(json!({"ok":true,"id":id,"old_speed":old,"new_speed":speed}))
    })
}

/// 设置片段音量。
pub fn volume(draft: &Path, id: &str, level: f64) -> Result<Value> {
    if !level.is_finite() || level < 0.0 {
        bail!("volume must be a non-negative finite number");
    }
    mutate(draft, |timeline| {
        let segment = find_segment_mut(timeline, id)?;
        let old = segment["volume"].as_f64().unwrap_or(1.0);
        segment["volume"] = json!(level);
        Ok(json!({"ok":true,"id":id,"old_volume":old,"new_volume":level}))
    })
}

/// 按精确轨道 ID 设置静音位；保留其余属性位、未知字段和片段音量。
pub fn track_mute(draft: &Path, track_id: &str, muted: bool) -> Result<Value> {
    if track_id.trim().is_empty() {
        bail!("track id must not be blank");
    }
    let change = mutate(draft, |timeline| {
        let track = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .find(|track| track["id"].as_str() == Some(track_id))
            .with_context(|| format!("track not found: {track_id}"))?;
        let old_attribute = track["attribute"]
            .as_i64()
            .filter(|value| *value >= 0)
            .with_context(|| format!("track {track_id} has invalid attribute"))?;
        let new_attribute = if muted {
            old_attribute | 1
        } else {
            old_attribute & !1
        };
        track["attribute"] = json!(new_attribute);
        Ok(json!({
            "ok":true,"track_id":track_id,
            "old_muted":old_attribute & 1 != 0,"new_muted":muted,
            "old_attribute":old_attribute,"new_attribute":new_attribute
        }))
    })?;
    // 事务提交后从真实草稿回读，而非仅相信内存中的拟写值。
    let timeline = crate::draft::load_timeline(draft)?;
    let track = timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?
        .iter()
        .find(|track| track["id"].as_str() == Some(track_id))
        .with_context(|| format!("track missing after mute commit: {track_id}"))?;
    let readback_attribute = track["attribute"]
        .as_i64()
        .with_context(|| format!("track {track_id} has invalid attribute after commit"))?;
    if readback_attribute != change["new_attribute"].as_i64().unwrap_or(-1) {
        bail!("track mute readback mismatch for {track_id}");
    }
    Ok(change)
}

/// 修剪片段的素材入点和持续时间。
pub fn trim(draft: &Path, id: &str, start_us: i64, duration_us: i64) -> Result<Value> {
    if start_us < 0 || duration_us <= 0 {
        bail!("trim start must be >= 0 and duration must be > 0");
    }
    mutate(draft, |timeline| {
        let segment = find_segment_mut(timeline, id)?;
        let speed = segment["speed"].as_f64().unwrap_or(1.0);
        let target_duration = (duration_us as f64 / speed).round() as i64;
        segment["source_timerange"]["start"] = json!(start_us);
        segment["source_timerange"]["duration"] = json!(duration_us);
        segment["target_timerange"]["duration"] = json!(target_duration);
        Ok(json!({"ok":true,"id":id,"source_start_us":start_us,
            "source_duration_us":duration_us,"target_duration_us":target_duration}))
    })
}

/// 设置画面片段不透明度。
pub fn opacity(draft: &Path, id: &str, alpha: f64) -> Result<Value> {
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        bail!("opacity must be between 0 and 1");
    }
    mutate(draft, |timeline| {
        let segment = find_segment_mut(timeline, id)?;
        if !segment["clip"].is_object() {
            bail!("segment {id} has no clip");
        }
        let old = segment["clip"]["alpha"].as_f64().unwrap_or(1.0);
        segment["clip"]["alpha"] = json!(alpha);
        Ok(json!({"ok":true,"id":id,"old_opacity":old,"new_opacity":alpha}))
    })
}

/// 新增空轨道；视频轨保持在其他类型之前。
pub fn add_track(draft: &Path, kind: &str, name: &str) -> Result<Value> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    add_track_at(draft, &id, kind, name, None)
}

/// 使用调用方声明的稳定标识新增轨道，并可指定绝对轨道位置。
pub fn add_track_at(
    draft: &Path,
    id: &str,
    kind: &str,
    name: &str,
    index: Option<usize>,
) -> Result<Value> {
    if !["video", "audio", "text", "sticker", "effect", "filter"].contains(&kind) {
        bail!("unsupported track type: {kind}");
    }
    if id.trim().is_empty() || name.trim().is_empty() {
        bail!("track id and name must not be blank");
    }
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        if tracks.iter().any(|track| track["id"].as_str() == Some(id)) {
            bail!("duplicate track id: {id}");
        }
        let index = index.unwrap_or_else(|| if kind == "video" { 0 } else { tracks.len() });
        if index > tracks.len() {
            bail!("track index {index} exceeds track count {}", tracks.len());
        }
        let track = json!({"attribute":0,"flag":0,"id":id,"is_default_name":false,
            "name":name,"segments":[],"type":kind});
        tracks.insert(index, track);
        Ok(json!({"ok":true,"track_id":id,"index":index,"type":kind,"name":name}))
    })
}

/// 删除整条轨道，并按剩余片段引用保守清扫孤儿素材。
pub fn remove_track(draft: &Path, id: &str) -> Result<Value> {
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        let index = tracks
            .iter()
            .position(|track| track["id"].as_str() == Some(id))
            .with_context(|| format!("track not found: {id}"))?;
        let removed = tracks.remove(index);
        let segment_count = removed["segments"].as_array().map_or(0, Vec::len);
        let (materials_removed, materials_by_type) = prune_in_timeline(timeline)?;
        Ok(json!({"ok":true,"track_id":id,"index":index,
            "segments_removed":segment_count,"materials_removed":materials_removed,
            "materials_by_type":materials_by_type}))
    })
}

/// 将轨道移动到给定绝对索引；索引按移除前的最终轨道数量解释。
pub fn reorder_track(draft: &Path, id: &str, index: usize) -> Result<Value> {
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        if index >= tracks.len() {
            bail!(
                "track index {index} exceeds final track count {}",
                tracks.len()
            );
        }
        let old_index = tracks
            .iter()
            .position(|track| track["id"].as_str() == Some(id))
            .with_context(|| format!("track not found: {id}"))?;
        let track = tracks.remove(old_index);
        tracks.insert(index, track);
        Ok(json!({"ok":true,"track_id":id,"old_index":old_index,"index":index}))
    })
}

/// 将刚导入的单片段临时轨道收敛到目标轨道，并恢复领域片段标识。
pub fn adopt_imported_segment(
    draft: &Path,
    imported_track_name: &str,
    target_track_id: &str,
    segment_id: &str,
) -> Result<Value> {
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        let source_index = tracks
            .iter()
            .position(|track| track["name"].as_str() == Some(imported_track_name))
            .with_context(|| format!("imported track not found: {imported_track_name}"))?;
        let source_track = tracks.remove(source_index);
        let source_kind = source_track["type"]
            .as_str()
            .context("imported track type is missing")?
            .to_owned();
        let mut source_segments = source_track["segments"]
            .as_array()
            .context("imported track segments must be an array")?
            .clone();
        if source_segments.len() != 1 {
            bail!("imported track must contain exactly one segment");
        }
        if tracks.iter().any(|track| {
            track["segments"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|segment| segment["id"].as_str() == Some(segment_id))
        }) {
            bail!("duplicate segment id: {segment_id}");
        }
        let target = tracks
            .iter_mut()
            .find(|track| track["id"].as_str() == Some(target_track_id))
            .with_context(|| format!("track not found: {target_track_id}"))?;
        if target["type"].as_str() != Some(source_kind.as_str()) {
            bail!("track {target_track_id} has incompatible type");
        }
        let mut segment = source_segments.remove(0);
        let start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
        let end = start
            + segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
        let target_segments = target["segments"]
            .as_array_mut()
            .context("target segments must be an array")?;
        if target_segments.iter().any(|other| {
            let other_start = other["target_timerange"]["start"].as_i64().unwrap_or(0);
            let other_end =
                other_start + other["target_timerange"]["duration"].as_i64().unwrap_or(0);
            other_start < end && other_end > start
        }) {
            bail!("track {target_track_id} is occupied over the target range");
        }
        segment["id"] = json!(segment_id);
        segment["raw_segment_id"] = json!(target_track_id);
        let material_id = segment["material_id"].clone();
        let references = segment["extra_material_refs"].clone();
        target_segments.push(segment);
        target_segments.sort_by_key(|item| item["target_timerange"]["start"].as_i64().unwrap_or(0));
        Ok(
            json!({"ok":true,"track_id":target_track_id,"segment_id":segment_id,
            "material_id":material_id,"extra_material_refs":references}),
        )
    })
}

/// 将一个完整的无损 segment JSON 加入指定轨道，引用闭包由提交校验保证。
pub fn add_segment(draft: &Path, track_id: &str, segment_file: &Path) -> Result<Value> {
    let segment: Value = serde_json::from_str(&std::fs::read_to_string(segment_file)?)?;
    let id = segment["id"]
        .as_str()
        .context("segment JSON requires string id")?
        .to_owned();
    mutate(draft, |timeline| {
        if timeline["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|track| {
                track["segments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|item| item["id"] == id)
            })
        {
            bail!("duplicate segment id: {id}");
        }
        let track = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .find(|track| track["id"].as_str() == Some(track_id))
            .with_context(|| format!("track not found: {track_id}"))?;
        track["segments"]
            .as_array_mut()
            .context("segments must be an array")?
            .push(segment);
        track["segments"]
            .as_array_mut()
            .unwrap()
            .sort_by_key(|item| item["target_timerange"]["start"].as_i64().unwrap_or(0));
        Ok(json!({"ok":true,"track_id":track_id,"segment_id":id}))
    })
}

/// 一次原子设置片段的常用时间和混音属性。
pub fn set_segment(
    draft: &Path,
    id: &str,
    start_us: Option<i64>,
    duration_us: Option<i64>,
    speed: Option<f64>,
    volume: Option<f64>,
    opacity: Option<f64>,
) -> Result<Value> {
    if duration_us.is_some_and(|value| value <= 0)
        || speed.is_some_and(|value| !value.is_finite() || value <= 0.0)
        || volume.is_some_and(|value| !value.is_finite() || value < 0.0)
        || opacity.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
    {
        bail!("invalid segment property value");
    }
    mutate(draft, |timeline| {
        let segment = find_segment_mut(timeline, id)?;
        if let Some(value) = start_us {
            segment["target_timerange"]["start"] = json!(value.max(0));
        }
        if let Some(value) = duration_us {
            segment["target_timerange"]["duration"] = json!(value);
        }
        if let Some(value) = speed {
            segment["speed"] = json!(value);
        }
        if let Some(value) = volume {
            segment["volume"] = json!(value);
        }
        if let Some(value) = opacity {
            if !segment["clip"].is_object() {
                bail!("segment {id} has no clip");
            }
            segment["clip"]["alpha"] = json!(value);
        }
        Ok(json!({"ok":true,"id":id,"segment":segment}))
    })
}

/// 在片段目标时间范围内拆分，保留原片段 ID 给左半段。
pub fn split(draft: &Path, id: &str, at_us: i64) -> Result<Value> {
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        for track in tracks {
            let segments = track["segments"]
                .as_array_mut()
                .context("segments must be an array")?;
            if let Some(index) = segments
                .iter()
                .position(|item| item["id"].as_str() == Some(id))
            {
                let mut right = segments[index].clone();
                let start = segments[index]["target_timerange"]["start"]
                    .as_i64()
                    .unwrap_or(0);
                let duration = segments[index]["target_timerange"]["duration"]
                    .as_i64()
                    .unwrap_or(0);
                if at_us <= start || at_us >= start + duration {
                    bail!("split point must be inside segment range");
                }
                let left_duration = at_us - start;
                let right_duration = duration - left_duration;
                let speed = segments[index]["speed"].as_f64().unwrap_or(1.0);
                let source_start = segments[index]["source_timerange"]["start"]
                    .as_i64()
                    .unwrap_or(0);
                let source_left = (left_duration as f64 * speed).round() as i64;
                segments[index]["target_timerange"]["duration"] = json!(left_duration);
                segments[index]["source_timerange"]["duration"] = json!(source_left);
                let new_id = uuid::Uuid::new_v4().simple().to_string();
                right["id"] = json!(new_id);
                right["target_timerange"]["start"] = json!(at_us);
                right["target_timerange"]["duration"] = json!(right_duration);
                right["source_timerange"]["start"] = json!(source_start + source_left);
                right["source_timerange"]["duration"] =
                    json!((right_duration as f64 * speed).round() as i64);
                segments.insert(index + 1, right);
                return Ok(
                    json!({"ok":true,"source_segment_id":id,"new_segment_id":new_id,"at_us":at_us}),
                );
            }
        }
        bail!("segment not found: {id}")
    })
}

/// 深拷贝片段、主素材、伴随素材和嵌入式关键帧。
pub fn duplicate(
    draft: &Path,
    id: &str,
    target_track: Option<&str>,
    dry_run: bool,
) -> Result<Value> {
    if dry_run {
        let mut timeline = crate::draft::load_timeline(draft)?;
        let mut result = duplicate_in_timeline(&mut timeline, id, target_track)?;
        result["dryRun"] = json!(true);
        return Ok(result);
    }
    mutate(draft, |timeline| {
        duplicate_in_timeline(timeline, id, target_track)
    })
}

fn duplicate_in_timeline(
    timeline: &mut Value,
    id: &str,
    target_track: Option<&str>,
) -> Result<Value> {
    let tracks = timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?;
    let (source_index, source_segment) = tracks
        .iter()
        .enumerate()
        .find_map(|(index, track)| {
            track["segments"]
                .as_array()?
                .iter()
                .find(|segment| segment["id"].as_str() == Some(id))
                .cloned()
                .map(|segment| (index, segment))
        })
        .with_context(|| format!("segment not found: {id}"))?;
    let source_kind = tracks[source_index]["type"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let source_name = tracks[source_index]["name"]
        .as_str()
        .unwrap_or("track")
        .to_owned();
    let source_material_id = source_segment["material_id"]
        .as_str()
        .context("source segment has no material_id")?
        .to_owned();
    let (primary_bucket, primary_material) =
        find_material_with_bucket(timeline, &source_material_id)
            .with_context(|| format!("material not found for segment: {id}"))?;

    let (target_index, created_track) = if let Some(name) = target_track {
        let index = tracks
            .iter()
            .position(|track| track["name"].as_str() == Some(name))
            .with_context(|| format!("track not found: {name}"))?;
        if tracks[index]["type"].as_str() != Some(&source_kind) {
            bail!("track {name} has incompatible type")
        }
        let start = source_segment["target_timerange"]["start"]
            .as_i64()
            .unwrap_or(0);
        let end = start
            + source_segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
        let occupied = tracks[index]["segments"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|segment| {
                let other_start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
                let other_end = other_start
                    + segment["target_timerange"]["duration"]
                        .as_i64()
                        .unwrap_or(0);
                other_start < end && other_end > start
            });
        if occupied {
            bail!("track {name} is occupied over the target range")
        }
        (index, false)
    } else {
        let names = tracks
            .iter()
            .filter_map(|track| track["name"].as_str())
            .collect::<std::collections::HashSet<_>>();
        let base = format!("{source_name}-copy");
        let mut name = base.clone();
        let mut suffix = 2;
        while names.contains(name.as_str()) {
            name = format!("{base}-{suffix}");
            suffix += 1;
        }
        let track_id = uuid::Uuid::new_v4().simple().to_string();
        timeline["tracks"].as_array_mut().unwrap().insert(
            source_index + 1,
            json!({
                "attribute":0,"flag":0,"id":track_id,"is_default_name":false,
                "name":name,"segments":[],"type":source_kind
            }),
        );
        (source_index + 1, true)
    };
    let target_id = timeline["tracks"][target_index]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let target_name = timeline["tracks"][target_index]["name"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let mut copy = source_segment;
    let new_segment_id = uuid::Uuid::new_v4().simple().to_string();
    copy["id"] = json!(new_segment_id);
    copy["raw_segment_id"] = json!(target_id);
    let mut cloned_materials = Vec::new();
    let new_primary_id = clone_material(
        timeline,
        &primary_bucket,
        &primary_material,
        &source_material_id,
    )?;
    copy["material_id"] = json!(new_primary_id);
    cloned_materials
        .push(json!({"type":primary_bucket,"id":new_primary_id,"source_id":source_material_id}));
    let old_refs = copy["extra_material_refs"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut new_refs = Vec::new();
    for reference in old_refs.iter().filter_map(Value::as_str) {
        let Some((bucket, material)) = find_material_with_bucket(timeline, reference) else {
            continue;
        };
        let new_id = clone_material(timeline, &bucket, &material, reference)?;
        new_refs.push(json!(new_id));
        cloned_materials.push(json!({"type":bucket,"id":new_id,"source_id":reference}));
    }
    copy["extra_material_refs"] = Value::Array(new_refs);
    if let Some(lists) = copy["common_keyframes"].as_array_mut() {
        for list in lists {
            if list["id"].is_string() {
                list["id"] = json!(uuid::Uuid::new_v4().simple().to_string());
            }
            for keyframe in list["keyframe_list"].as_array_mut().into_iter().flatten() {
                if keyframe["id"].is_string() {
                    keyframe["id"] = json!(uuid::Uuid::new_v4().simple().to_string());
                }
            }
        }
    }
    timeline["tracks"][target_index]["segments"]
        .as_array_mut()
        .context("target segments must be an array")?
        .push(copy);
    Ok(
        json!({"ok":true,"new_segment_id":new_segment_id,"source_segment_id":id,
        "material_id":new_primary_id,"track_id":target_id,"track_name":target_name,
        "new_track":created_track,"cloned_materials":cloned_materials}),
    )
}

fn find_material_with_bucket(timeline: &Value, id: &str) -> Option<(String, Value)> {
    timeline["materials"]
        .as_object()?
        .iter()
        .find_map(|(bucket, items)| {
            items
                .as_array()?
                .iter()
                .find(|material| material["id"].as_str() == Some(id))
                .cloned()
                .map(|material| (bucket.clone(), material))
        })
}

fn clone_material(
    timeline: &mut Value,
    bucket: &str,
    source: &Value,
    source_id: &str,
) -> Result<String> {
    let mut copy = source.clone();
    let new_id = uuid::Uuid::new_v4().simple().to_string();
    copy["id"] = json!(new_id);
    timeline["materials"][bucket]
        .as_array_mut()
        .with_context(|| format!("materials.{bucket} must be an array"))?
        .push(copy);
    debug_assert_ne!(new_id, source_id);
    Ok(new_id)
}

/// 删除片段，并按 surviving references 保守清扫孤儿素材。
pub fn remove(
    draft: &Path,
    id: &str,
    keep_track: bool,
    keep_materials: bool,
    dry_run: bool,
) -> Result<Value> {
    if dry_run {
        let mut timeline = crate::draft::load_timeline(draft)?;
        let mut result = remove_in_timeline(&mut timeline, id, keep_track, keep_materials)?;
        result["dryRun"] = json!(true);
        return Ok(result);
    }
    mutate(draft, |timeline| {
        remove_in_timeline(timeline, id, keep_track, keep_materials)
    })
}

/// 清扫未被任何 surviving segment 直接或间接引用的素材。
pub fn prune(draft: &Path, dry_run: bool) -> Result<Value> {
    if dry_run {
        let mut timeline = crate::draft::load_timeline(draft)?;
        let (removed, by_type) = prune_in_timeline(&mut timeline)?;
        return Ok(json!({"ok":true,"removed":removed,"by_type":by_type,"dryRun":true}));
    }
    mutate(draft, |timeline| {
        let (removed, by_type) = prune_in_timeline(timeline)?;
        Ok(json!({"ok":true,"removed":removed,"by_type":by_type}))
    })
}

pub(crate) fn remove_in_timeline(
    timeline: &mut Value,
    id: &str,
    keep_track: bool,
    keep_materials: bool,
) -> Result<Value> {
    let duration_before = timeline["duration"].as_i64().unwrap_or(0);
    let tracks = timeline["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?;
    let (track_index, segment_index) = tracks
        .iter()
        .enumerate()
        .find_map(|(track_index, track)| {
            track["segments"]
                .as_array()?
                .iter()
                .position(|segment| segment["id"].as_str() == Some(id))
                .map(|segment_index| (track_index, segment_index))
        })
        .with_context(|| format!("segment not found: {id}"))?;
    let track_id = tracks[track_index]["id"].clone();
    let track_name = tracks[track_index]["name"].clone();
    let track_type = tracks[track_index]["type"].clone();
    tracks[track_index]["segments"]
        .as_array_mut()
        .unwrap()
        .remove(segment_index);
    let track_removed = tracks[track_index]["segments"]
        .as_array()
        .unwrap()
        .is_empty()
        && !keep_track;
    if track_removed {
        tracks.remove(track_index);
    }
    let mut materials_removed = 0usize;
    let mut materials_by_type = serde_json::Map::new();
    if !keep_materials {
        (materials_removed, materials_by_type) = prune_in_timeline(timeline)?;
    }
    let duration_after = timeline_duration(timeline);
    timeline["duration"] = json!(duration_after);
    Ok(
        json!({"ok":true,"removed_segment_id":id,"track_id":track_id,"track_name":track_name,
        "track_type":track_type,"track_removed":track_removed,"materials_removed":materials_removed,
        "materials_by_type":materials_by_type,"duration_before_us":duration_before,
        "duration_after_us":duration_after}),
    )
}

fn prune_in_timeline(timeline: &mut Value) -> Result<(usize, serde_json::Map<String, Value>)> {
    let referenced = referenced_material_ids(timeline);
    let mut removed_total = 0usize;
    let mut by_type = serde_json::Map::new();
    for (bucket, items) in timeline["materials"]
        .as_object_mut()
        .context("materials must be an object")?
    {
        let Some(array) = items.as_array_mut() else {
            continue;
        };
        let before = array.len();
        array.retain(|material| {
            material["id"]
                .as_str()
                .is_none_or(|value| referenced.contains(value))
        });
        let removed = before - array.len();
        removed_total += removed;
        by_type.insert(
            bucket.clone(),
            json!({"removed":removed,"kept":array.len()}),
        );
    }
    Ok((removed_total, by_type))
}

fn referenced_material_ids(timeline: &Value) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        for segment in track["segments"].as_array().into_iter().flatten() {
            if let Some(id) = segment["material_id"].as_str() {
                ids.insert(id.to_owned());
            }
            for id in segment["extra_material_refs"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                ids.insert(id.to_owned());
            }
        }
    }
    ids
}

/// 为视觉片段设置混合模式素材，旧混合模式引用会被替换。
pub fn composite(draft: &Path, id: &str, mode: &str) -> Result<Value> {
    let entry = crate::catalogs::resolve(crate::catalogs::mix_modes(), "mix mode", mode, false)?;
    let effect_id = entry["effect_id"].clone();
    let resource_id = entry["resource_id"].clone();
    let display_name = entry["name"].clone();
    mutate(draft, |timeline| {
        let old_mix_ids: Vec<String> = timeline["materials"]["effects"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|item| item["type"].as_str() == Some("mix_mode"))
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect();
        let new_id = uuid::Uuid::new_v4().simple().to_string();
        let segment = find_segment_mut(timeline, id)?;
        let refs = segment["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?;
        refs.retain(|value| !old_mix_ids.iter().any(|old| value.as_str() == Some(old)));
        refs.push(json!(new_id));
        timeline["materials"]["effects"].as_array_mut().context("materials.effects must be an array")?.push(json!({
            "type":"mix_mode","name":display_name,"effect_id":effect_id,"resource_id":resource_id,
            "value":1.0,"apply_target_type":0,"platform":"all","source_platform":0,
            "category_id":"","category_name":"","sub_type":"none","time_range":null,"id":new_id
        }));
        Ok(json!({"ok":true,"id":id,"mode":mode,"material_id":new_id}))
    })
}

/// 切换视频素材级智能抠像，并保留应用生成的缓存与未知字段。
pub fn matting(draft: &Path, id: &str, off: bool) -> Result<Value> {
    mutate(draft, |timeline| {
        let tracks = timeline["tracks"]
            .as_array()
            .context("tracks must be an array")?;
        let (track_type, segment_id, material_id) = tracks
            .iter()
            .find_map(|track| {
                track["segments"]
                    .as_array()?
                    .iter()
                    .find(|segment| segment["id"].as_str() == Some(id))
                    .map(|segment| {
                        (
                            track["type"].as_str().unwrap_or_default().to_owned(),
                            segment["id"].as_str().unwrap_or_default().to_owned(),
                            segment["material_id"]
                                .as_str()
                                .unwrap_or_default()
                                .to_owned(),
                        )
                    })
            })
            .with_context(|| format!("Segment not found: {id}"))?;
        if track_type != "video" {
            bail!(
                "Smart matting only applies to video/photo segments (segment {id} is on a {track_type} track)"
            );
        }
        let shared_segments = tracks
            .iter()
            .flat_map(|track| track["segments"].as_array().into_iter().flatten())
            .filter(|segment| {
                segment["id"].as_str() != Some(segment_id.as_str())
                    && segment["material_id"].as_str() == Some(material_id.as_str())
            })
            .filter_map(|segment| segment["id"].as_str().map(str::to_owned))
            .collect::<Vec<_>>();
        let material = timeline["materials"]["videos"]
            .as_array_mut()
            .context("materials.videos must be an array")?
            .iter_mut()
            .find(|material| material["id"].as_str() == Some(material_id.as_str()))
            .with_context(|| {
                format!("Segment {id} references a missing video material: {material_id}")
            })?;
        let mut object = serde_json::Map::from_iter([
            ("has_use_quick_brush".to_owned(), json!(false)),
            ("has_use_quick_eraser".to_owned(), json!(false)),
            ("interactiveTime".to_owned(), json!([])),
            ("path".to_owned(), json!("")),
            ("strokes".to_owned(), json!([])),
        ]);
        if let Some(existing) = material["matting"].as_object() {
            object.extend(existing.clone());
        }
        let flag = if off { 0 } else { 3 };
        object.insert("flag".to_owned(), json!(flag));
        material["matting"] = Value::Object(object);
        Ok(json!({
            "ok":true,"segmentId":segment_id,"materialId":material_id,
            "flag":flag,"enabled":!off,"shared_segments":shared_segments
        }))
    })
}

/// 新增或移除视频片段的色度键素材引用。
pub fn chroma(
    draft: &Path,
    id: &str,
    color: Option<&str>,
    intensity: Option<f64>,
    off: bool,
) -> Result<Value> {
    mutate(draft, |timeline| {
        if off {
            let chroma_ids = timeline["materials"]["chromas"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| item["id"].as_str().map(str::to_owned))
                .collect::<std::collections::HashSet<_>>();
            let segment = find_segment_mut(timeline, id)?;
            let refs = segment["extra_material_refs"]
                .as_array_mut()
                .context("extra_material_refs must be an array")?;
            let mut removed = Vec::new();
            refs.retain(|value| {
                let should_remove = value
                    .as_str()
                    .is_some_and(|reference| chroma_ids.contains(reference));
                if should_remove {
                    removed.push(value.as_str().unwrap_or_default().to_owned());
                }
                !should_remove
            });
            if let Some(chromas) = timeline["materials"]["chromas"].as_array_mut() {
                chromas.retain(|item| {
                    item["id"]
                        .as_str()
                        .is_none_or(|material_id| !removed.iter().any(|id| id == material_id))
                });
            }
            return Ok(json!({"ok":true,"segmentId":id,"removed":removed}));
        }

        let color =
            color.context("Missing --color <#RRGGBB>. Pick the green-screen color to key out.")?;
        let hex = color.strip_prefix('#').unwrap_or(color);
        if hex.len() != 6 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("Invalid color: {color}. Expected #RRGGBB.");
        }
        let intensity = intensity.unwrap_or(0.5);
        if !intensity.is_finite() {
            bail!("intensity must be finite");
        }
        let intensity = intensity.clamp(0.0, 1.0);
        let track_type = timeline["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|track| {
                track["segments"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|segment| segment["id"].as_str() == Some(id))
            })
            .map(|track| track["type"].as_str().unwrap_or_default().to_owned())
            .with_context(|| format!("Segment not found: {id}"))?;
        if track_type != "video" {
            bail!(
                "Chroma key can only be applied to video segments (segment {id} is on a {track_type} track)"
            );
        }
        let material_id = uuid::Uuid::new_v4().to_string();
        find_segment_mut(timeline, id)?["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?
            .push(json!(material_id));
        timeline["materials"]["chromas"]
            .as_array_mut()
            .context("materials.chromas must be an array")?
            .push(json!({
                "id":material_id,"type":"chromas","color":color,
                "intensity":intensity,"shadow":0.0,"path":""
            }));
        Ok(json!({
            "ok":true,"segmentId":id,"materialId":material_id,
            "color":color,"intensity":intensity,"shadow":0.0
        }))
    })
}

/// 新增或关闭片段 mask，并兼容三种已知素材数组字段。
#[allow(clippy::too_many_arguments)]
pub fn mask(
    draft: &Path,
    id: &str,
    slug: Option<&str>,
    off: bool,
    jianying: bool,
    center_x: Option<f64>,
    center_y: Option<f64>,
    size: Option<f64>,
    rotation: Option<f64>,
    feather: Option<f64>,
    invert: bool,
    rect_width: Option<f64>,
    round_corner: Option<f64>,
    mask_field: Option<&str>,
) -> Result<Value> {
    const FIELDS: [&str; 3] = ["common_masks", "common_mask", "masks"];
    mutate(draft, |timeline| {
        if off {
            let mask_ids = FIELDS
                .iter()
                .flat_map(|field| {
                    timeline["materials"][field]
                        .as_array()
                        .into_iter()
                        .flatten()
                })
                .filter_map(|item| item["id"].as_str().map(str::to_owned))
                .collect::<std::collections::HashSet<_>>();
            let segment = find_segment_mut(timeline, id)?;
            let refs = segment["extra_material_refs"]
                .as_array_mut()
                .context("extra_material_refs must be an array")?;
            let before = refs.len();
            refs.retain(|value| {
                value
                    .as_str()
                    .is_none_or(|reference| !mask_ids.contains(reference))
            });
            return Ok(json!({"ok":true,"id":id,"removed":before - refs.len()}));
        }

        let slug = slug.context("mask slug is required unless --off is used")?;
        let resolved = match slug {
            "linear" => "split",
            "mirror" => "filmstrip",
            "star" => "stars",
            other => other,
        };
        let metadata = if jianying {
            crate::catalogs::masks()
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| {
                    item["name"].as_str() == Some(slug) || item["shape"].as_str() == Some(resolved)
                })
        } else {
            crate::catalogs::capcut_masks()
                .as_array()
                .into_iter()
                .flatten()
                .find(|item| item["slug"].as_str() == Some(resolved))
        }
        .with_context(|| format!("Unknown mask: {slug}"))?;

        let segment_exists = timeline["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|track| track["segments"].as_array().into_iter().flatten())
            .any(|segment| segment["id"].as_str() == Some(id));
        if !segment_exists {
            bail!("Segment not found: {id}");
        }
        let existing_ids = FIELDS
            .iter()
            .flat_map(|field| {
                timeline["materials"][field]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .filter_map(|item| item["id"].as_str())
            .collect::<std::collections::HashSet<_>>();
        if let Some(existing) = timeline["tracks"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|track| track["segments"].as_array().into_iter().flatten())
            .find(|segment| segment["id"].as_str() == Some(id))
            .and_then(|segment| {
                segment["extra_material_refs"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .find(|reference| existing_ids.contains(reference))
            })
        {
            bail!("Segment already has a mask (material {existing}). Remove it first.");
        }
        if (rect_width.is_some() || round_corner.is_some()) && resolved != "rectangle" {
            bail!("--rect-width / --round-corner only valid for rectangle mask");
        }

        let field = if let Some(field) = mask_field {
            if !FIELDS.contains(&field) {
                bail!("unsupported mask field: {field}");
            }
            field.to_owned()
        } else {
            mask_target_field(timeline)
        };
        if !timeline["materials"][&field].is_array() {
            timeline["materials"][&field] = json!([]);
        }
        let aspect_ratio = metadata["default_aspect_ratio"]
            .as_f64()
            .or_else(|| metadata["default_aspect"].as_f64())
            .unwrap_or(1.0);
        let size = size.unwrap_or(0.5);
        let canvas_width = timeline["canvas_config"]["width"]
            .as_f64()
            .unwrap_or(1920.0);
        let canvas_height = timeline["canvas_config"]["height"]
            .as_f64()
            .unwrap_or(1080.0);
        let width = if resolved == "rectangle" {
            rect_width.unwrap_or(size)
        } else {
            size * canvas_height * aspect_ratio / canvas_width
        };
        let material_id = uuid::Uuid::new_v4().to_string();
        let name = metadata["name"].as_str().unwrap_or_default().to_owned();
        let resource_type = metadata["resource_type"]
            .as_str()
            .or_else(|| metadata["shape"].as_str())
            .unwrap_or_default();
        let material = json!({
            "config":{
                "aspectRatio":aspect_ratio,"centerX":center_x.unwrap_or(0.0),
                "centerY":center_y.unwrap_or(0.0),"feather":feather.unwrap_or(0.0) / 100.0,
                "height":size,"invert":invert,"rotation":rotation.unwrap_or(0.0),
                "roundCorner":round_corner.unwrap_or(0.0) / 100.0,"width":width
            },
            "category":"video","category_id":"","category_name":"","id":material_id,
            "name":name,"platform":"all","position_info":"",
            "resource_type":resource_type,"resource_id":metadata["resource_id"],"type":"mask"
        });
        timeline["materials"][&field]
            .as_array_mut()
            .unwrap()
            .push(material);
        find_segment_mut(timeline, id)?["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?
            .push(json!(material_id));
        Ok(json!({
            "ok":true,"segmentId":id,"mask_id":material_id,"name":name,"field":field
        }))
    })
}

fn mask_target_field(timeline: &Value) -> String {
    let source = timeline["platform"]["app_source"].as_str();
    let version = timeline["platform"]["app_version"].as_str();
    if source == Some("lv") {
        if let Some(version) = version {
            let mut parts = version
                .split('.')
                .filter_map(|part| part.parse::<u64>().ok());
            let major = parts.next().unwrap_or(0);
            let minor = parts.next().unwrap_or(0);
            return if (major, minor) >= (9, 6) {
                "common_masks".to_owned()
            } else {
                "masks".to_owned()
            };
        }
    }
    for field in ["common_masks", "common_mask", "masks"] {
        if timeline["materials"][field]
            .as_array()
            .is_some_and(|items| !items.is_empty())
        {
            return field.to_owned();
        }
    }
    if source == Some("lv") {
        "masks".to_owned()
    } else {
        "common_mask".to_owned()
    }
}

/// 设置四级背景模糊，关闭时移除片段上所有 canvas 素材引用。
pub fn background_blur(draft: &Path, id: &str, level: Option<u8>, off: bool) -> Result<Value> {
    mutate(draft, |timeline| {
        let canvas_ids = timeline["materials"]["canvases"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect::<std::collections::HashSet<_>>();
        let segment = find_segment_mut(timeline, id)?;
        let refs = segment["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?;
        refs.retain(|value| {
            value
                .as_str()
                .is_none_or(|reference| !canvas_ids.contains(reference))
        });
        if off {
            return Ok(json!({"ok":true,"segmentId":id,"canvas_id":null,"blur":null}));
        }
        let level = level.context("bg-blur level must be 1, 2, 3, or 4 (or --off)")?;
        let blur = match level {
            1 => 0.0625,
            2 => 0.375,
            3 => 0.75,
            4 => 1.0,
            _ => bail!("bg-blur level must be 1, 2, 3, or 4 (or --off)"),
        };
        let canvas_id = uuid::Uuid::new_v4().to_string();
        timeline["materials"]["canvases"]
            .as_array_mut()
            .context("materials.canvases must be an array")?
            .push(json!({
                "album_image":"","blur":blur,"color":"","id":canvas_id,
                "image":"","image_id":"","image_name":"","source_platform":0,
                "team_id":"","type":"canvas_blur"
            }));
        find_segment_mut(timeline, id)?["extra_material_refs"]
            .as_array_mut()
            .unwrap()
            .push(json!(canvas_id));
        Ok(json!({"ok":true,"segmentId":id,"canvas_id":canvas_id,"blur":blur}))
    })
}

/// 为音频片段设置淡入淡出；重复应用会替换当前生效引用。
pub fn audio_fade(draft: &Path, id: &str, fade_in_us: i64, fade_out_us: i64) -> Result<Value> {
    if fade_in_us <= 0 && fade_out_us <= 0 {
        bail!("audio-fade requires at least one of --in or --out (> 0)");
    }
    mutate(draft, |timeline| {
        let wanted = id.to_ascii_lowercase();
        let (track_index, segment_index) = timeline["tracks"]
            .as_array()
            .context("tracks must be an array")?
            .iter()
            .enumerate()
            .find_map(|(track_index, track)| {
                track["segments"]
                    .as_array()?
                    .iter()
                    .position(|segment| {
                        segment["id"].as_str().is_some_and(|candidate| {
                            candidate == id || candidate.to_ascii_lowercase().starts_with(&wanted)
                        })
                    })
                    .map(|segment_index| (track_index, segment_index))
            })
            .with_context(|| format!("Segment not found: {id}"))?;
        let track_type = timeline["tracks"][track_index]["type"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        if track_type != "audio" {
            bail!("audio-fade only applies to audio segments (track type: {track_type})");
        }
        let segment_id = timeline["tracks"][track_index]["segments"][segment_index]["id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let fade_ids = timeline["materials"]["audio_fades"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect::<std::collections::HashSet<_>>();
        timeline["tracks"][track_index]["segments"][segment_index]["extra_material_refs"]
            .as_array_mut()
            .context("extra_material_refs must be an array")?
            .retain(|value| {
                value
                    .as_str()
                    .is_none_or(|reference| !fade_ids.contains(reference))
            });
        let fade_id = uuid::Uuid::new_v4().to_string();
        timeline["materials"]["audio_fades"]
            .as_array_mut()
            .context("materials.audio_fades must be an array")?
            .push(json!({
                "id":fade_id,"fade_in_duration":fade_in_us,
                "fade_out_duration":fade_out_us,"fade_type":0,"type":"audio_fade"
            }));
        timeline["tracks"][track_index]["segments"][segment_index]["extra_material_refs"]
            .as_array_mut()
            .unwrap()
            .push(json!(fade_id));
        Ok(json!({
            "ok":true,"segmentId":segment_id,"fade_id":fade_id,
            "fade_in_us":fade_in_us,"fade_out_us":fade_out_us
        }))
    })
}

/// 新增颜色滤镜轨道片段，对应 capcut-cli `addFilter`。
#[allow(clippy::too_many_arguments)]
pub fn add_filter(
    draft: &Path,
    slug: &str,
    start_us: Option<i64>,
    duration_us: Option<i64>,
    full: bool,
    intensity: Option<f64>,
    track_name: Option<&str>,
    jianying: bool,
    resource_id: Option<&str>,
    effect_id: Option<&str>,
) -> Result<Value> {
    if effect_id.is_some() && resource_id.is_none() {
        bail!("--effect-id requires --resource-id");
    }
    let value = intensity.unwrap_or(1.0);
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("--intensity must be a number in range 0..1");
    }
    let (name, resolved_effect_id, resolved_resource_id, source_platform) =
        resolve_filter(slug, jianying, resource_id, effect_id)?;
    let track_name = track_name.unwrap_or("filter").to_owned();
    mutate(draft, |timeline| {
        let (start, duration) = if full {
            let duration = timeline["duration"].as_i64().unwrap_or(0);
            if duration <= 0 {
                bail!("--full: draft has no duration");
            }
            (0, duration)
        } else {
            (
                start_us.context("add-filter requires start")?,
                duration_us.context("add-filter requires duration")?,
            )
        };
        let segment_id = uuid::Uuid::new_v4().to_string();
        let material_id = uuid::Uuid::new_v4().to_string();
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        let track_index = tracks.iter().position(|track| {
            track["type"].as_str() == Some("filter")
                && track["name"].as_str() == Some(track_name.as_str())
        });
        let track_index = match track_index {
            Some(index) => index,
            None => {
                let track_id = uuid::Uuid::new_v4().to_string();
                tracks.push(json!({
                    "id":track_id,"type":"filter","name":track_name,"attribute":0,
                    "segments":[],"is_default_name":track_name == "filter","flag":0
                }));
                tracks.len() - 1
            }
        };
        let track_id = tracks[track_index]["id"]
            .as_str()
            .context("filter track requires string id")?
            .to_owned();
        tracks[track_index]["segments"]
            .as_array_mut()
            .context("filter track segments must be an array")?
            .push(json!({
                "id":segment_id,"material_id":material_id,"raw_segment_id":track_id,
                "target_timerange":{"start":start,"duration":duration},
                "source_timerange":{"start":0,"duration":duration},
                "speed":1,"volume":1,"visible":true,"reverse":false,"clip":null,
                "render_index":11000,"track_render_index":0,"track_attribute":0,
                "extra_material_refs":[],"common_keyframes":[],"keyframe_refs":[]
            }));
        timeline["materials"]["video_effects"]
            .as_array_mut()
            .context("materials.video_effects must be an array")?
            .push(json!({
                "adjust_params":[],"apply_target_type":2,"apply_time_range":null,
                "category_id":"","category_name":"Filter","common_keyframes":[],
                "effect_id":resolved_effect_id,"formula_id":"","id":material_id,
                "name":name,"platform":"all","render_index":11000,
                "resource_id":resolved_resource_id,"source_platform":source_platform,
                "time_range":null,"track_render_index":0,"type":"filter","value":value,
                "version":""
            }));
        Ok(json!({
            "ok":true,"segmentId":segment_id,"materialId":material_id,
            "trackId":track_id,"name":name,"start_us":start,"duration_us":duration
        }))
    })
}

fn resolve_filter(
    slug: &str,
    jianying: bool,
    resource_id: Option<&str>,
    effect_id: Option<&str>,
) -> Result<(String, String, String, i64)> {
    if let Some(resource_id) = resource_id {
        return Ok((
            slug.to_owned(),
            effect_id.unwrap_or(resource_id).to_owned(),
            resource_id.to_owned(),
            1,
        ));
    }
    let catalog = if jianying {
        crate::catalogs::filters()
    } else {
        crate::catalogs::capcut_filters()
    };
    let entry = catalog
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry["slug"]
                .as_str()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(slug))
                || entry["name"]
                    .as_str()
                    .is_some_and(|candidate| candidate.eq_ignore_ascii_case(slug))
        })
        .with_context(|| {
            let hint = if jianying { " --jianying" } else { "" };
            format!("Unknown filter slug: {slug}. Run 'capcut enums --filters{hint}' for the full list.")
        })?;
    Ok((
        entry["name"].as_str().unwrap_or(slug).to_owned(),
        entry["effect_id"].as_str().unwrap_or_default().to_owned(),
        entry["resource_id"].as_str().unwrap_or_default().to_owned(),
        0,
    ))
}

/// 新增场景或人物特效轨道片段，对应 capcut-cli `addEffect`。
#[allow(clippy::too_many_arguments)]
pub fn add_effect(
    draft: &Path,
    slug: &str,
    start_us: Option<i64>,
    duration_us: Option<i64>,
    full: bool,
    params: Option<&[f64]>,
    intensity: Option<f64>,
    track_name: Option<&str>,
    jianying: bool,
    resource_id: Option<&str>,
    effect_id: Option<&str>,
    bind_segment_id: Option<&str>,
) -> Result<Value> {
    if effect_id.is_some() && resource_id.is_none() {
        bail!("--effect-id requires --resource-id");
    }
    let value = intensity.unwrap_or(1.0);
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("--intensity must be a number in range 0..1");
    }
    if params
        .into_iter()
        .flatten()
        .any(|parameter| !parameter.is_finite())
    {
        bail!("--params must contain only finite numbers");
    }
    let (name, resolved_effect_id, resolved_resource_id, effect_type, source_platform) =
        resolve_effect(slug, jianying, resource_id, effect_id)?;
    let track_name = track_name.unwrap_or("effect").to_owned();
    mutate(draft, |timeline| {
        let resolved_bind = bind_segment_id
            .map(|wanted| resolve_segment_id(timeline, wanted))
            .transpose()?;
        let (start, duration) = if full {
            let duration = timeline["duration"].as_i64().unwrap_or(0);
            if duration <= 0 {
                bail!("--full: draft has no duration");
            }
            (0, duration)
        } else {
            (
                start_us.context("add-effect requires start")?,
                duration_us.context("add-effect requires duration")?,
            )
        };
        let segment_id = uuid::Uuid::new_v4().to_string();
        let material_id = uuid::Uuid::new_v4().to_string();
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?;
        let track_index = tracks.iter().position(|track| {
            track["type"].as_str() == Some("effect")
                && track["name"].as_str() == Some(track_name.as_str())
        });
        let track_index = match track_index {
            Some(index) => index,
            None => {
                let track_id = uuid::Uuid::new_v4().to_string();
                tracks.push(json!({
                    "id":track_id,"type":"effect","name":track_name,"attribute":0,
                    "segments":[],"is_default_name":track_name == "effect","flag":0
                }));
                tracks.len() - 1
            }
        };
        let track_id = tracks[track_index]["id"]
            .as_str()
            .context("effect track requires string id")?
            .to_owned();
        tracks[track_index]["segments"]
            .as_array_mut()
            .context("effect track segments must be an array")?
            .push(json!({
                "id":segment_id,"material_id":material_id,"raw_segment_id":track_id,
                "target_timerange":{"start":start,"duration":duration},
                "source_timerange":{"start":0,"duration":duration},
                "speed":1,"volume":1,"visible":true,"reverse":false,"clip":null,
                "render_index":11000,"track_render_index":0,"track_attribute":0,
                "extra_material_refs":[],"common_keyframes":[],"keyframe_refs":[]
            }));
        let adjust_params: Vec<Value> = params
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                json!({"name":format!("param_{index}"),"value":parameter,"default_value":parameter})
            })
            .collect();
        let mut material = json!({
            "adjust_params":adjust_params,
            "apply_target_type":if resolved_bind.is_some() { 0 } else { 2 },
            "apply_time_range":null,"category_id":"","category_name":"",
            "common_keyframes":[],"disable_effect_faces":[],
            "effect_id":resolved_effect_id,"formula_id":"","id":material_id,
            "name":name,"platform":"all","render_index":11000,
            "resource_id":resolved_resource_id,"source_platform":source_platform,
            "time_range":null,"track_render_index":0,"type":effect_type,"value":value,
            "version":""
        });
        if let Some(bound) = resolved_bind {
            material["bind_segment_id"] = json!(bound);
        }
        timeline["materials"]["video_effects"]
            .as_array_mut()
            .context("materials.video_effects must be an array")?
            .push(material);
        Ok(json!({
            "ok":true,"segmentId":segment_id,"materialId":material_id,
            "trackId":track_id,"name":name,"start_us":start,"duration_us":duration
        }))
    })
}

fn resolve_segment_id(timeline: &Value, id: &str) -> Result<String> {
    let wanted = id.to_ascii_lowercase();
    timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .find_map(|segment| {
            segment["id"].as_str().and_then(|candidate| {
                (candidate == id || candidate.to_ascii_lowercase().starts_with(&wanted))
                    .then(|| candidate.to_owned())
            })
        })
        .with_context(|| format!("Segment not found: {id}"))
}

fn resolve_effect(
    slug: &str,
    jianying: bool,
    resource_id: Option<&str>,
    effect_id: Option<&str>,
) -> Result<(String, String, String, String, i64)> {
    if let Some(resource_id) = resource_id {
        return Ok((
            slug.to_owned(),
            effect_id.unwrap_or(resource_id).to_owned(),
            resource_id.to_owned(),
            "video_effect".to_owned(),
            1,
        ));
    }
    if !jianying {
        if let Some(entry) = find_catalog_entry(crate::catalogs::capcut_effects(), slug) {
            return Ok(effect_tuple(entry, "video_effect", 0));
        }
    }
    if let Some(entry) = find_catalog_entry(crate::catalogs::video_scene_effects(), slug) {
        return Ok(effect_tuple(entry, "video_effect", 0));
    }
    if let Some(entry) = find_catalog_entry(crate::catalogs::video_character_effects(), slug) {
        return Ok(effect_tuple(entry, "face_effect", 0));
    }
    let hint = if jianying { " --jianying" } else { "" };
    bail!("Unknown effect slug: {slug}. Run 'capcut enums --scene-effects{hint}' or '--character-effects{hint}' for the full list.")
}

fn find_catalog_entry<'a>(catalog: &'a Value, slug: &str) -> Option<&'a Value> {
    catalog.as_array()?.iter().find(|entry| {
        entry["slug"]
            .as_str()
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(slug))
            || entry["name"]
                .as_str()
                .is_some_and(|candidate| candidate.eq_ignore_ascii_case(slug))
    })
}

fn effect_tuple(
    entry: &Value,
    default_type: &str,
    source_platform: i64,
) -> (String, String, String, String, i64) {
    (
        entry["name"].as_str().unwrap_or_default().to_owned(),
        entry["effect_id"].as_str().unwrap_or_default().to_owned(),
        entry["resource_id"].as_str().unwrap_or_default().to_owned(),
        entry["effect_type"]
            .as_str()
            .unwrap_or(default_type)
            .to_owned(),
        source_platform,
    )
}

/// 读取或修改视频/图片素材的归一化裁剪矩形。
pub fn crop(
    draft: &Path,
    segment_id: &str,
    rect: Option<&str>,
    ratio: Option<&str>,
    reset: bool,
    dry_run: bool,
) -> Result<Value> {
    if rect.is_none() && ratio.is_none() && !reset {
        return crop_info(&crate::draft::load_timeline(draft)?, segment_id);
    }
    if dry_run {
        let mut timeline = crate::draft::load_timeline(draft)?;
        let mut output = crop_in_timeline(&mut timeline, segment_id, rect, ratio, reset)?;
        output["dryRun"] = json!(true);
        return Ok(output);
    }
    mutate(draft, |timeline| {
        crop_in_timeline(timeline, segment_id, rect, ratio, reset)
    })
}

/// 向片段写入一个或多个关键帧，并维护相邻缓动句柄。
pub fn keyframe(
    draft: &Path,
    segment_id: &str,
    inputs: &[KeyframeInput],
    default_easing: Option<&str>,
) -> Result<Value> {
    validate_easing(default_easing.unwrap_or("linear"))?;
    if inputs.is_empty() {
        bail!("at least one keyframe is required");
    }
    mutate(draft, |timeline| {
        let fps = timeline["fps"]
            .as_f64()
            .filter(|fps| *fps > 0.0)
            .unwrap_or(30.0);
        let wanted = segment_id.to_ascii_lowercase();
        let segment = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .flat_map(|track| track["segments"].as_array_mut().into_iter().flatten())
            .find(|segment| {
                segment["id"].as_str().is_some_and(|id| {
                    id == segment_id || id.to_ascii_lowercase().starts_with(&wanted)
                })
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        let resolved_id = segment["id"]
            .as_str()
            .context("segment id must be a string")?
            .to_owned();
        if !segment["common_keyframes"].is_array() {
            segment["common_keyframes"] = json!([]);
        }
        let lists = segment["common_keyframes"].as_array_mut().unwrap();
        let mut warnings = Vec::new();
        let mut holds: Vec<(usize, String, i64)> = Vec::new();
        for input in inputs {
            let (property, value) =
                parse_keyframe_value(&input.property, &input.value.to_string())?;
            let property_type = KEYFRAME_PROPERTIES
                .iter()
                .find(|(name, _)| *name == property)
                .unwrap()
                .1;
            let easing = input
                .easing
                .as_deref()
                .or(default_easing)
                .unwrap_or("linear");
            validate_easing(easing)?;
            let list_index = if let Some(index) = lists
                .iter()
                .position(|list| list["property_type"].as_str() == Some(property_type))
            {
                index
            } else {
                lists.push(json!({"id":uuid::Uuid::new_v4().simple().to_string(),"keyframe_list":[],"material_id":"","property_type":property_type}));
                lists.len() - 1
            };
            let entries = lists[list_index]["keyframe_list"].as_array_mut().unwrap();
            let (prev, next) = neighbour_indices(entries, input.time_us);
            let mut entry = json!({
                "curveType":"Line","graphID":"",
                "left_control":{"x":0.0,"y":0.0},"right_control":{"x":0.0,"y":0.0},
                "id":uuid::Uuid::new_v4().simple().to_string(),"time_offset":input.time_us,"values":[value]
            });
            match easing {
                "linear" | "hold" => clear_facing_handles(entries, prev, next),
                curve => {
                    if prev.is_none() && next.is_none() {
                        warnings.push(format!("easing '{curve}' for {} at {}us has no adjacent keyframe to ease against; wrote a linear keyframe — it picks up the curve when its pair keyframe is added with an easing", input.property, input.time_us));
                    } else {
                        apply_curve(entries, &mut entry, prev, next, curve)?;
                    }
                }
            }
            entries.push(entry);
            entries.sort_by_key(|entry| entry["time_offset"].as_i64().unwrap_or_default());
            if easing == "hold" {
                holds.push((list_index, property, input.time_us));
            }
        }
        let mut hold_count = 0;
        let frame_us = js_round(1_000_000.0 / fps);
        for (list_index, property, time_us) in holds {
            let entries = lists[list_index]["keyframe_list"].as_array_mut().unwrap();
            let Some(current_index) = entries
                .iter()
                .position(|entry| entry["time_offset"].as_i64() == Some(time_us))
            else {
                continue;
            };
            let next_index = (current_index + 1 < entries.len()).then_some(current_index + 1);
            let Some(next_index) = next_index else {
                warnings.push(format!("hold for {property} at {time_us}us has no later keyframe to hold until; wrote a plain keyframe"));
                continue;
            };
            if entries[next_index]["values"] == entries[current_index]["values"] {
                continue;
            }
            let hold_at = entries[next_index]["time_offset"].as_i64().unwrap() - frame_us;
            if hold_at <= time_us {
                warnings.push(format!("hold for {property} at {time_us}us: the next keyframe is within one frame, nothing to hold"));
                continue;
            }
            if entries
                .iter()
                .any(|entry| entry["time_offset"].as_i64() == Some(hold_at))
            {
                continue;
            }
            let values = entries[current_index]["values"].clone();
            entries.push(json!({"curveType":"Line","graphID":"","left_control":{"x":0.0,"y":0.0},"right_control":{"x":0.0,"y":0.0},"id":uuid::Uuid::new_v4().simple().to_string(),"time_offset":hold_at,"values":values}));
            entries.sort_by_key(|entry| entry["time_offset"].as_i64().unwrap_or_default());
            hold_count += 1;
        }
        let summaries: Vec<Value> = lists.iter().map(|list| {
            let property_type = list["property_type"].as_str().unwrap_or_default();
            let property = KEYFRAME_PROPERTIES.iter().find(|(_, kind)| *kind == property_type).map(|(name, _)| *name).unwrap_or(property_type);
            json!({"property":property,"count":list["keyframe_list"].as_array().map(Vec::len).unwrap_or(0)})
        }).collect();
        let mut output = json!({"ok":true,"id":resolved_id,"added":inputs.len(),"lists":summaries});
        if hold_count > 0 {
            output["hold_keyframes"] = json!(hold_count);
        }
        if !warnings.is_empty() {
            output["warnings"] = json!(warnings);
        }
        Ok(output)
    })
}

/// 为片段附加 CapCut/剪映转场素材。
pub fn transition(
    draft: &Path,
    segment_id: &str,
    slug_or_member: &str,
    duration_us: Option<i64>,
    jianying: bool,
) -> Result<Value> {
    let entry = if jianying {
        let wanted = slug_or_member.strip_prefix('_').unwrap_or(slug_or_member);
        crate::catalogs::transitions()
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| {
                entry["name"]
                    .as_str()
                    .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
            })
    } else {
        crate::catalogs::capcut_transitions()
            .as_array()
            .into_iter()
            .flatten()
            .find(|entry| {
                entry["slug"]
                    .as_str()
                    .is_some_and(|slug| slug.eq_ignore_ascii_case(slug_or_member))
                    || entry["member"]
                        .as_str()
                        .is_some_and(|member| member.eq_ignore_ascii_case(slug_or_member))
            })
    }
    .with_context(|| format!("Unknown transition: {slug_or_member}"))?;
    let name = entry["name"]
        .as_str()
        .context("transition catalogue name is missing")?
        .to_owned();
    let effect_id = entry["effect_id"]
        .as_str()
        .context("transition catalogue effect_id is missing")?
        .to_owned();
    let resource_id = entry["resource_id"]
        .as_str()
        .context("transition catalogue resource_id is missing")?
        .to_owned();
    let duration = duration_us.unwrap_or_else(|| {
        entry[if jianying {
            "duration_us"
        } else {
            "default_duration"
        }]
        .as_i64()
        .unwrap_or(500_000)
    });
    mutate(draft, |timeline| {
        if !timeline["materials"]["transitions"].is_array() {
            timeline["materials"]["transitions"] = json!([]);
        }
        let transition_ids: std::collections::HashSet<String> = timeline["materials"]
            ["transitions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect();
        let wanted = segment_id.to_ascii_lowercase();
        let segment = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .flat_map(|track| track["segments"].as_array_mut().into_iter().flatten())
            .find(|segment| {
                segment["id"].as_str().is_some_and(|id| {
                    id == segment_id || id.to_ascii_lowercase().starts_with(&wanted)
                })
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        if segment["extra_material_refs"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|id| transition_ids.contains(id))
        {
            bail!("Segment already has a transition. Remove it first.");
        }
        let resolved_id = segment["id"]
            .as_str()
            .context("segment id must be a string")?
            .to_owned();
        let id = uuid::Uuid::new_v4().to_string();
        if !segment["extra_material_refs"].is_array() {
            segment["extra_material_refs"] = json!([]);
        }
        segment["extra_material_refs"]
            .as_array_mut()
            .unwrap()
            .push(json!(id));
        timeline["materials"]["transitions"].as_array_mut().unwrap().push(json!({
            "category_id":"","category_name":"","duration":duration,
            "effect_id":effect_id,"id":id,"is_overlap":entry["is_overlap"].as_bool().unwrap_or(false),
            "name":name,"platform":"all","resource_id":resource_id,"type":"transition"
        }));
        Ok(
            json!({"ok":true,"segmentId":resolved_id,"transition_id":id,"name":name,"duration_us":duration}),
        )
    })
}

/// 为视频或图片片段写入 CapCut/剪映入场、出场与组合动画。
pub fn image_animation(
    draft: &Path,
    segment_id: &str,
    options: ImageAnimationOptions<'_>,
) -> Result<Value> {
    if options.intro.is_none() && options.outro.is_none() && options.combo.is_none() {
        bail!("at least one of --intro, --outro, --combo is required");
    }
    let intro = options
        .intro
        .map(|slug| resolve_image_animation(slug, "intros", options.jianying))
        .transpose()?;
    let outro = options
        .outro
        .map(|slug| resolve_image_animation(slug, "outros", options.jianying))
        .transpose()?;
    let combo = options
        .combo
        .map(|slug| resolve_image_animation(slug, "combos", options.jianying))
        .transpose()?;
    mutate(draft, |timeline| {
        if !timeline["materials"]["material_animations"].is_array() {
            timeline["materials"]["material_animations"] = json!([]);
        }
        let animation_ids: std::collections::HashSet<String> = timeline["materials"]
            ["material_animations"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|item| item["id"].as_str().map(str::to_owned))
            .collect();
        let wanted = segment_id.to_ascii_lowercase();
        let segment = timeline["tracks"]
            .as_array_mut()
            .context("tracks must be an array")?
            .iter_mut()
            .flat_map(|track| track["segments"].as_array_mut().into_iter().flatten())
            .find(|segment| {
                segment["id"].as_str().is_some_and(|id| {
                    id == segment_id || id.to_ascii_lowercase().starts_with(&wanted)
                })
            })
            .with_context(|| format!("Segment not found: {segment_id}"))?;
        let resolved_id = segment["id"]
            .as_str()
            .context("segment id must be a string")?
            .to_owned();
        let target_duration = segment["target_timerange"]["duration"]
            .as_i64()
            .context("segment duration must be an integer")?;
        if !segment["extra_material_refs"].is_array() {
            segment["extra_material_refs"] = json!([]);
        }
        let existing = segment["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .find(|id| animation_ids.contains(*id))
            .map(str::to_owned);
        let container_id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        if !segment["extra_material_refs"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item.as_str() == Some(&container_id))
        {
            segment["extra_material_refs"]
                .as_array_mut()
                .unwrap()
                .push(json!(container_id));
        }
        let containers = timeline["materials"]["material_animations"]
            .as_array_mut()
            .unwrap();
        let container_index = if let Some(index) = containers
            .iter()
            .position(|item| item["id"].as_str() == Some(&container_id))
        {
            index
        } else {
            containers.push(json!({"animations":[],"id":container_id,"multi_language_current":"none","type":"sticker_animation"}));
            containers.len() - 1
        };
        let animations = containers[container_index]["animations"]
            .as_array_mut()
            .context("animation container animations must be an array")?;
        let mut added = Vec::new();
        if let Some(entry) = intro.as_ref() {
            add_image_animation_entry(
                animations,
                entry,
                "in",
                options.intro_duration_us,
                target_duration,
                &mut added,
            )?;
        }
        if let Some(entry) = outro.as_ref() {
            add_image_animation_entry(
                animations,
                entry,
                "out",
                options.outro_duration_us,
                target_duration,
                &mut added,
            )?;
        }
        if let Some(entry) = combo.as_ref() {
            add_image_animation_entry(
                animations,
                entry,
                "group",
                options.combo_duration_us,
                target_duration,
                &mut added,
            )?;
        }
        Ok(json!({"ok":true,"segmentId":resolved_id,"added":added,"material_id":container_id}))
    })
}

fn resolve_image_animation(slug: &str, bucket: &str, jianying: bool) -> Result<Value> {
    if let Some(entry) = inline_image_animation(slug) {
        return Ok(entry);
    }
    let namespace = if jianying { "jianying" } else { "capcut" };
    crate::catalogs::capcut_image_animations()[namespace][bucket]
        .as_array()
        .into_iter()
        .flatten()
        .find(|entry| {
            entry["slug"]
                .as_str()
                .is_some_and(|value| !value.is_empty() && value.eq_ignore_ascii_case(slug))
                || entry["member"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(slug))
        })
        .cloned()
        .with_context(|| {
            format!(
                "Unknown image {} animation: {slug}",
                image_animation_type(bucket)
            )
        })
}

fn image_animation_type(bucket: &str) -> &str {
    match bucket {
        "intros" => "in",
        "outros" => "out",
        _ => "group",
    }
}

fn inline_image_animation(slug: &str) -> Option<Value> {
    let (name, effect_id, md5, duration, category_id, third_resource_id) = match slug {
        "fade-in" => (
            "Fade In",
            "6798320778182922760",
            "883ad04bd79b502aaa55b5d9b87175ea",
            500_000,
            "2037708296",
            "6798320778182922760",
        ),
        "flash-in" => (
            "Flash In",
            "7211044701367964162",
            "6a680c49cd11a05f3eb0e5a3fed165f7",
            433_333,
            "2037708312",
            "7211044701367964162",
        ),
        "pulsing-zooms" => (
            "Pulsing Zooms",
            "7530463994486820097",
            "c2223de4486ee5b2a5900d707e9a362b",
            3_000_000,
            "2037708296",
            "0",
        ),
        "scroll-up" => (
            "Scroll up",
            "7315336764636271105",
            "cb8899ed512a2d40bd27d9e03d039ec0",
            1_200_000,
            "in_fav",
            "7315336764636271105",
        ),
        "stripe-merge" => (
            "Stripe Merge",
            "7570497406203251973",
            "fc08875f779dae706387fb160dbaa898",
            633_333,
            "2037708296",
            "0",
        ),
        "zoom-out" => (
            "Zoom Out",
            "6798332584276267527",
            "0c736f993d36a7b1ef00cc73d2ba656f",
            2_000_000,
            "",
            "",
        ),
        "fade-out" => (
            "Fade Out",
            "6798320902548230669",
            "c6f05ce62355b537be762550040bfc08",
            500_000,
            "2037708296",
            "0",
        ),
        "blur-out" => (
            "Blur Out",
            "7507514531212479761",
            "78d0826a4aba60259f37acb30149b258",
            1_000_000,
            "out_fav",
            "0",
        ),
        "smoke" => (
            "Smoke",
            "7229983825080619522",
            "e70e26e7aa770d0deedca54e3eac0323",
            900_000,
            "out_fav",
            "7229983825080619522",
        ),
        _ => return None,
    };
    Some(json!({
        "title":name,"effect_id":effect_id,"resource_id":effect_id,"md5":md5,
        "duration":duration,"category_id":category_id,"third_resource_id":third_resource_id
    }))
}

fn add_image_animation_entry(
    animations: &mut Vec<Value>,
    entry: &Value,
    animation_type: &str,
    override_duration: Option<i64>,
    target_duration: i64,
    added: &mut Vec<Value>,
) -> Result<()> {
    if animations
        .iter()
        .any(|animation| animation["type"].as_str() == Some(animation_type))
    {
        bail!("segment already has a {animation_type} video animation");
    }
    let duration = override_duration.unwrap_or_else(|| {
        entry["duration"]
            .as_i64()
            .or_else(|| entry["default_duration"].as_i64())
            .unwrap_or(500_000)
    });
    if duration > target_duration {
        bail!("duration ({duration}us) exceeds segment duration ({target_duration}us)");
    }
    let start = if animation_type == "out" {
        target_duration - duration
    } else {
        0
    };
    let name = entry["title"]
        .as_str()
        .or_else(|| entry["name"].as_str())
        .unwrap_or_default();
    let category = entry["category_id"].as_str().unwrap_or_else(|| {
        if animation_type == "in" {
            "in_fav"
        } else if animation_type == "out" {
            "out_fav"
        } else {
            ""
        }
    });
    let effect_id = entry["effect_id"].as_str().unwrap_or_default();
    let md5 = entry["md5"].as_str().unwrap_or_default();
    let path = if md5.is_empty() {
        String::new()
    } else {
        format!(
            "{}/Library/Containers/com.lemon.lvoverseas/Data/Movies/CapCut/User Data/Cache/effect/{effect_id}/{md5}",
            std::env::var("HOME").unwrap_or_default()
        )
    };
    animations.push(json!({
        "anim_adjust_params":null,"category_id":category,"category_name":category,
        "duration":duration,"id":effect_id,"material_type":"video","name":name,
        "panel":"video","path":path,"platform":"all","request_id":"",
        "resource_id":entry["resource_id"],"source_platform":1,"start":start,
        "third_resource_id":entry["third_resource_id"].as_str().unwrap_or("0"),"type":animation_type
    }));
    added.push(json!({"type":animation_type,"name":name,"duration_us":duration,"start_us":start}));
    Ok(())
}

fn validate_easing(easing: &str) -> Result<()> {
    if !matches!(
        easing,
        "linear" | "ease-in" | "ease-out" | "ease-in-out" | "hold"
    ) {
        bail!("Unsupported keyframe easing: {easing}. Supported: linear, ease-in, ease-out, ease-in-out, hold");
    }
    Ok(())
}

fn neighbour_indices(entries: &[Value], time: i64) -> (Option<usize>, Option<usize>) {
    let mut prev: Option<usize> = None;
    let mut next: Option<usize> = None;
    for (index, entry) in entries.iter().enumerate() {
        let offset = entry["time_offset"].as_i64().unwrap_or_default();
        if offset < time
            && prev.is_none_or(|p| entries[p]["time_offset"].as_i64().unwrap_or_default() < offset)
        {
            prev = Some(index);
        }
        if offset > time
            && next.is_none_or(|n| entries[n]["time_offset"].as_i64().unwrap_or_default() > offset)
        {
            next = Some(index);
        }
    }
    (prev, next)
}

fn zero_control(value: &Value) -> bool {
    value["x"].as_f64() == Some(0.0) && value["y"].as_f64() == Some(0.0)
}

fn clear_facing_handles(entries: &mut [Value], prev: Option<usize>, next: Option<usize>) {
    if let Some(index) = prev {
        entries[index]["right_control"] = json!({"x":0,"y":0});
        if zero_control(&entries[index]["left_control"]) {
            entries[index]["curveType"] = json!("Line");
        }
    }
    if let Some(index) = next {
        entries[index]["left_control"] = json!({"x":0,"y":0});
        if zero_control(&entries[index]["right_control"]) {
            entries[index]["curveType"] = json!("Line");
        }
    }
}

fn apply_curve(
    entries: &mut [Value],
    entry: &mut Value,
    prev: Option<usize>,
    next: Option<usize>,
    easing: &str,
) -> Result<()> {
    let (right_ratio, left_ratio) = match easing {
        "ease-in" => (0.42, 0.0),
        "ease-out" => (0.32, -0.4),
        "ease-in-out" => (0.42, -0.42),
        _ => bail!("unsupported curve {easing}"),
    };
    entry["curveType"] = json!("FreeCurveInOut");
    let value = entry["values"][0].as_f64().unwrap();
    let time = entry["time_offset"].as_i64().unwrap();
    if let Some(index) = prev {
        let interval = time - entries[index]["time_offset"].as_i64().unwrap();
        let previous = entries[index]["values"][0].as_f64().unwrap();
        let y = if easing == "ease-out" {
            ((0.94 * (value - previous) * 1_000_000.0).round()) / 1_000_000.0
        } else {
            0.0
        };
        entries[index]["curveType"] = json!("FreeCurveInOut");
        entries[index]["right_control"] =
            json!({"x":js_round(right_ratio * interval as f64),"y":y});
        entry["left_control"] = json!({"x":js_round(left_ratio * interval as f64),"y":0.0});
    }
    if let Some(index) = next {
        let interval = entries[index]["time_offset"].as_i64().unwrap() - time;
        let following = entries[index]["values"][0].as_f64().unwrap();
        let y = if easing == "ease-out" {
            ((0.94 * (following - value) * 1_000_000.0).round()) / 1_000_000.0
        } else {
            0.0
        };
        entries[index]["curveType"] = json!("FreeCurveInOut");
        entries[index]["left_control"] =
            json!({"x":js_round(left_ratio * interval as f64),"y":0.0});
        entry["right_control"] = json!({"x":js_round(right_ratio * interval as f64),"y":y});
    }
    Ok(())
}

fn js_round(value: f64) -> i64 {
    (value + 0.5).floor() as i64
}

fn crop_info(timeline: &Value, segment_id: &str) -> Result<Value> {
    let (resolved_segment_id, material_id, material_index) = crop_target(timeline, segment_id)?;
    let material = &timeline["materials"]["videos"][material_index];
    Ok(json!({
        "segmentId":resolved_segment_id,"material_id":material_id,
        "width":material.get("width").cloned().unwrap_or(Value::Null),
        "height":material.get("height").cloned().unwrap_or(Value::Null),
        "crop":material.get("crop").cloned().unwrap_or(Value::Null)
    }))
}

fn crop_in_timeline(
    timeline: &mut Value,
    segment_id: &str,
    rect: Option<&str>,
    ratio: Option<&str>,
    reset: bool,
) -> Result<Value> {
    let (resolved_segment_id, material_id, material_index) = crop_target(timeline, segment_id)?;
    let rect = if let Some(rect) = rect {
        parse_crop_rect(rect)?
    } else if let Some(ratio) = ratio {
        let material = &timeline["materials"]["videos"][material_index];
        crop_for_ratio(
            material["width"].as_f64().unwrap_or(0.0),
            material["height"].as_f64().unwrap_or(0.0),
            ratio,
        )?
    } else if reset {
        [0.0, 0.0, 1.0, 1.0]
    } else {
        unreachable!("caller requires one crop mutation mode")
    };
    let [x, y, width, height] = rect;
    validate_crop_rect(x, y, width, height)?;
    let right = (x + width).min(1.0);
    let bottom = (y + height).min(1.0);
    let crop = json!({
        "lower_left_x":x,"lower_left_y":bottom,
        "lower_right_x":right,"lower_right_y":bottom,
        "upper_left_x":x,"upper_left_y":y,
        "upper_right_x":right,"upper_right_y":y
    });
    let material = &mut timeline["materials"]["videos"][material_index];
    material["crop"] = crop.clone();
    let mut output = json!({
        "ok":true,"segmentId":resolved_segment_id,"material_id":material_id,
        "rect":{"x":x,"y":y,"w":width,"h":height},"crop":crop
    });
    if material.get("crop_ratio").is_some() {
        material["crop_ratio"] = json!("free");
        output["crop_ratio"] = json!("free");
    }
    Ok(output)
}

fn crop_target(timeline: &Value, segment_id: &str) -> Result<(String, String, usize)> {
    let wanted = segment_id.to_ascii_lowercase();
    let segment = timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .find(|segment| {
            segment["id"].as_str().is_some_and(|candidate| {
                candidate == segment_id || candidate.to_ascii_lowercase().starts_with(&wanted)
            })
        })
        .with_context(|| format!("Segment not found: {segment_id}"))?;
    let resolved_segment_id = segment["id"]
        .as_str()
        .context("segment requires string id")?
        .to_owned();
    let material_id = segment["material_id"]
        .as_str()
        .context("segment requires material_id")?
        .to_owned();
    let videos = timeline["materials"]["videos"]
        .as_array()
        .context("materials.videos must be an array")?;
    let material_index = videos
        .iter()
        .position(|material| material["id"].as_str() == Some(material_id.as_str()))
        .with_context(|| {
            format!(
                "crop only applies to video/photo segments (no video material for {segment_id})"
            )
        })?;
    let material_type = videos[material_index]["type"].as_str().unwrap_or_default();
    if !matches!(material_type, "video" | "photo") {
        bail!("crop only applies to video/photo materials (got type={material_type})");
    }
    Ok((resolved_segment_id, material_id, material_index))
}

fn parse_crop_rect(raw: &str) -> Result<[f64; 4]> {
    let values: Vec<f64> = raw
        .split(',')
        .map(|part| part.trim().parse::<f64>())
        .collect::<std::result::Result<_, _>>()
        .with_context(|| {
            format!("--rect expects four comma-separated numbers: x,y,w,h (got {raw:?})")
        })?;
    values.try_into().map_err(|_| {
        anyhow::anyhow!("--rect expects four comma-separated numbers: x,y,w,h (got {raw:?})")
    })
}

fn crop_for_ratio(width: f64, height: f64, ratio: &str) -> Result<[f64; 4]> {
    if ratio == "free" {
        return Ok([0.0, 0.0, 1.0, 1.0]);
    }
    let target = match ratio {
        "1:1" => 1.0,
        "16:9" => 16.0 / 9.0,
        "9:16" => 9.0 / 16.0,
        "4:3" => 4.0 / 3.0,
        "3:4" => 3.0 / 4.0,
        _ => bail!("Unknown ratio: {ratio}. Valid: free, 1:1, 16:9, 9:16, 4:3, 3:4"),
    };
    if width <= 0.0 || height <= 0.0 {
        bail!("Source material has no stored width/height, so --ratio cannot be computed. Pass an explicit --rect <x,y,w,h> instead.");
    }
    let source = width / height;
    let crop_width = if target >= source {
        1.0
    } else {
        target / source
    };
    let crop_height = if target >= source {
        source / target
    } else {
        1.0
    };
    Ok([
        (1.0 - crop_width) / 2.0,
        (1.0 - crop_height) / 2.0,
        crop_width,
        crop_height,
    ])
}

fn validate_crop_rect(x: f64, y: f64, width: f64, height: f64) -> Result<()> {
    if [x, y, width, height].iter().any(|value| !value.is_finite()) {
        bail!("Crop rect values must be finite numbers");
    }
    if x < 0.0 || y < 0.0 {
        bail!("Crop rect x/y must be >= 0 (got x={x}, y={y})");
    }
    if width <= 0.0 || height <= 0.0 {
        bail!("Crop rect w/h must be > 0 (got w={width}, h={height})");
    }
    if x + width > 1.0 + 1e-9 || y + height > 1.0 + 1e-9 {
        bail!(
            "Crop rect must stay inside the frame: x+w <= 1 and y+h <= 1 (got x+w={}, y+h={})",
            x + width,
            y + height
        );
    }
    Ok(())
}

pub(crate) fn mutate<F>(draft: &Path, operation: F) -> Result<Value>
where
    F: FnOnce(&mut Value) -> Result<Value>,
{
    mutate_with_path(draft, |timeline, _, _| operation(timeline))
}

pub(crate) fn mutate_with_path<F>(draft: &Path, operation: F) -> Result<Value>
where
    F: FnOnce(&mut Value, &Path, &Path) -> Result<Value>,
{
    let running = crate::store::editors_running();
    if !running.is_empty() {
        bail!(
            "editor is running ({}); close it before mutating drafts",
            running.join(", ")
        );
    }
    let state_root = draft
        .parent()
        .unwrap_or(draft)
        .join(".jianying-transactions");
    let mut plan = jianying_store::MutationPlan::new(draft.to_path_buf(), state_root)?;
    plan.stage()?;
    let mut timeline = crate::draft::load_timeline(plan.work_copy())?;
    let output = operation(&mut timeline, plan.work_copy(), draft)?;
    timeline["duration"] = json!(timeline_duration(&timeline));
    crate::template::save_timeline_as(plan.work_copy(), &timeline, draft)?;
    let identity = plan.source().to_path_buf();
    plan.validate(|work| {
        crate::draft::validate_bundle_as(work, &identity)
            .map_err(|error| jianying_store::StoreError::Validation(format!("{error:#}")))
    })?;
    plan.commit()?;
    Ok(output)
}

fn find_segment_mut<'a>(timeline: &'a mut Value, id: &str) -> Result<&'a mut Value> {
    for track in timeline["tracks"]
        .as_array_mut()
        .context("tracks must be an array")?
    {
        if let Some(segment) = track["segments"]
            .as_array_mut()
            .into_iter()
            .flatten()
            .find(|segment| segment["id"].as_str() == Some(id))
        {
            return Ok(segment);
        }
    }
    bail!("segment not found: {id}")
}

fn segment_summary(timeline: &Value, track: &Value, segment: &Value) -> Value {
    let material = find_material(
        timeline,
        segment["material_id"].as_str().unwrap_or_default(),
    );
    let label = material["material_name"]
        .as_str()
        .or_else(|| material["name"].as_str())
        .map(str::to_owned)
        .or_else(|| material["content"].as_str().map(extract_text))
        .unwrap_or_default();
    json!({"id":segment["id"],"type":track["type"],
        "start_us":segment["target_timerange"]["start"],
        "duration_us":segment["target_timerange"]["duration"],
        "speed":segment["speed"],"volume":segment["volume"],
        "opacity":segment["clip"]["alpha"].as_f64().unwrap_or(1.0),"label":label})
}

fn extract_text(content: &str) -> String {
    serde_json::from_str::<Value>(content)
        .ok()
        .and_then(|value| value["text"].as_str().map(str::to_owned))
        .unwrap_or_else(|| content.to_owned())
}

fn find_material(timeline: &Value, id: &str) -> Value {
    for (kind, items) in timeline["materials"].as_object().into_iter().flatten() {
        if let Some(material) = items.as_array().and_then(|items| {
            items
                .iter()
                .find(|material| material["id"].as_str() == Some(id))
        }) {
            let mut value = material.clone();
            if let Some(object) = value.as_object_mut() {
                object.insert("_type".into(), json!(kind));
            }
            return value;
        }
    }
    Value::Null
}

fn segment_end(segment: &Value) -> i64 {
    segment["target_timerange"]["start"].as_i64().unwrap_or(0)
        + segment["target_timerange"]["duration"]
            .as_i64()
            .unwrap_or(0)
}

fn timeline_duration(timeline: &Value) -> i64 {
    timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .map(segment_end)
        .max()
        .unwrap_or(0)
}

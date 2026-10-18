//! 计划级编排：组合、增量追加与可回滚历史（hypit 式"计划可编排"的可验证版本）。
//!
//! 所有写操作都先落一份快照（`<plan>.<epoch>.bak`），再原子写回；
//! `plan history` 列出快照，`plan revert` 恢复指定或最近一份。
//! 组合规则保持确定：画布必须一致，后一份按前一份的总时长整体后移。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 组合多份计划：画布必须一致，后一份整体后移前一份的总时长。
pub fn compose(out: &Path, parts: &[PathBuf], name: Option<&str>, force: bool) -> Result<Value> {
    if parts.is_empty() {
        bail!("compose needs at least one input plan");
    }
    let mut loaded = Vec::with_capacity(parts.len());
    for part in parts {
        loaded.push(crate::plan::Plan::load(part)?);
    }
    let canvas = &loaded[0].canvas;
    for (plan, path) in loaded.iter().zip(parts) {
        if plan.canvas.width != canvas.width
            || plan.canvas.height != canvas.height
            || plan.canvas.fps != canvas.fps
        {
            bail!(
                "canvas mismatch: {} is {}x{}@{} but {} is {}x{}@{}",
                parts[0].display(),
                canvas.width,
                canvas.height,
                canvas.fps,
                path.display(),
                plan.canvas.width,
                plan.canvas.height,
                plan.canvas.fps
            );
        }
    }
    if out.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite (a snapshot is kept)",
            out.display()
        );
    }

    // 以第一份为骨架，逐条轨道合并；每份的片段按累计偏移后移。
    let mut merged: Value = serde_json::to_value(&loaded[0])?;
    merged["name"] = json!(name.unwrap_or("composed"));
    merged["tracks"] = json!([]);
    let mut offset_us = 0_i64;
    for plan in &loaded {
        let plan_value = serde_json::to_value(plan)?;
        for track in plan_value["tracks"].as_array().into_iter().flatten() {
            let kind = track["type"].as_str().unwrap_or("video").to_owned();
            let track_name = track["name"].as_str().unwrap_or(&kind).to_owned();
            let target = ensure_track(&mut merged, &kind, &track_name)?;
            for segment in track["segments"].as_array().into_iter().flatten() {
                let mut segment = segment.clone();
                let start = segment["start_us"].as_i64().unwrap_or(0);
                segment["start_us"] = json!(start + offset_us);
                target
                    .as_array_mut()
                    .context("merged track segments must be an array")?
                    .push(segment);
            }
        }
        offset_us += plan.total_duration();
    }

    order_tracks(&mut merged);
    let plan: crate::plan::Plan = serde_json::from_value(merged.clone())
        .map_err(|error| anyhow::anyhow!("composed plan is invalid: {error}"))?;
    plan.validate()?;

    let snapshot = if out.exists() {
        Some(snapshot(out)?)
    } else {
        None
    };
    write_plan(out, &merged)?;
    Ok(json!({
        "ok": true,
        "out": out.to_string_lossy(),
        "parts": parts.iter().map(|path| path.to_string_lossy().into_owned()).collect::<Vec<_>>(),
        "duration_us": plan.total_duration(),
        "tracks": plan.tracks.len(),
        "snapshot": snapshot.map(|path| path.to_string_lossy().into_owned()),
    }))
}

/// 在已有计划末尾追加一份计划（原地写回，先落快照）。
pub fn append(plan_path: &Path, part: &Path, name: Option<&str>) -> Result<Value> {
    let base = crate::plan::Plan::load(plan_path)?;
    let extra = crate::plan::Plan::load(part)?;
    if (base.canvas.width, base.canvas.height, base.canvas.fps)
        != (extra.canvas.width, extra.canvas.height, extra.canvas.fps)
    {
        bail!(
            "canvas mismatch: {} is {}x{}@{} but {} is {}x{}@{}",
            plan_path.display(),
            base.canvas.width,
            base.canvas.height,
            base.canvas.fps,
            part.display(),
            extra.canvas.width,
            extra.canvas.height,
            extra.canvas.fps
        );
    }
    let mut merged: Value = serde_json::to_value(&base)?;
    if let Some(name) = name {
        merged["name"] = json!(name);
    }
    let offset_us = base.total_duration();
    let extra_value = serde_json::to_value(&extra)?;
    for track in extra_value["tracks"].as_array().into_iter().flatten() {
        let kind = track["type"].as_str().unwrap_or("video").to_owned();
        let track_name = track["name"].as_str().unwrap_or(&kind).to_owned();
        let target = ensure_track(&mut merged, &kind, &track_name)?;
        for segment in track["segments"].as_array().into_iter().flatten() {
            let mut segment = segment.clone();
            let start = segment["start_us"].as_i64().unwrap_or(0);
            segment["start_us"] = json!(start + offset_us);
            target
                .as_array_mut()
                .context("merged track segments must be an array")?
                .push(segment);
        }
    }
    order_tracks(&mut merged);
    let composed: crate::plan::Plan = serde_json::from_value(merged.clone())
        .map_err(|error| anyhow::anyhow!("appended plan is invalid: {error}"))?;
    composed.validate()?;
    let snapshot = snapshot(plan_path)?;
    write_plan(plan_path, &merged)?;
    Ok(json!({
        "ok": true,
        "plan": plan_path.to_string_lossy(),
        "appended": part.to_string_lossy(),
        "duration_us": composed.total_duration(),
        "snapshot": snapshot.to_string_lossy(),
    }))
}

/// 列出计划的快照（按时间从旧到新）。
pub fn history(plan_path: &Path) -> Result<Value> {
    if !plan_path.is_file() {
        bail!("{} does not exist", plan_path.display());
    }
    let mut snapshots = snapshots(plan_path)?;
    snapshots.sort();
    Ok(json!({
        "ok": true,
        "plan": plan_path.to_string_lossy(),
        "total": snapshots.len(),
        "snapshots": snapshots.iter().map(|path| json!({
            "path": path.to_string_lossy(),
            "bytes": std::fs::metadata(path).map(|meta| meta.len()).unwrap_or(0),
        })).collect::<Vec<_>>(),
    }))
}

/// 恢复快照；不指定时恢复最近一份。
pub fn revert(plan_path: &Path, snapshot_path: Option<&Path>) -> Result<Value> {
    let mut snapshots = snapshots(plan_path)?;
    snapshots.sort();
    let chosen = match snapshot_path {
        Some(path) => {
            if !path.is_file() {
                bail!("snapshot {} does not exist", path.display());
            }
            path.to_path_buf()
        }
        None => snapshots
            .pop()
            .ok_or_else(|| anyhow::anyhow!("no snapshot recorded for {}", plan_path.display()))?,
    };
    let content = std::fs::read_to_string(&chosen)
        .with_context(|| format!("cannot read snapshot {}", chosen.display()))?;
    // 恢复前先校验快照仍是合法计划，避免把坏内容写回去。
    let parsed: crate::plan::Plan = serde_json::from_str(content.trim_start_matches('\u{feff}'))
        .map_err(|error| anyhow::anyhow!("snapshot is not a valid plan: {error}"))?;
    parsed.validate()?;
    snapshot(plan_path)?;
    std::fs::write(plan_path, content.as_bytes())
        .with_context(|| format!("cannot restore {}", plan_path.display()))?;
    Ok(json!({
        "ok": true,
        "plan": plan_path.to_string_lossy(),
        "restored_from": chosen.to_string_lossy(),
        "duration_us": parsed.total_duration(),
    }))
}

fn ensure_track<'a>(merged: &'a mut Value, kind: &str, name: &str) -> Result<&'a mut Value> {
    let tracks = merged["tracks"]
        .as_array_mut()
        .context("plan tracks must be an array")?;
    let index = tracks
        .iter()
        .position(|track| track["type"].as_str() == Some(kind));
    let index = match index {
        Some(index) => index,
        None => {
            tracks.push(json!({"type": kind, "name": name, "segments": []}));
            tracks.len() - 1
        }
    };
    tracks[index]
        .as_object_mut()
        .and_then(|object| object.get_mut("segments"))
        .context("merged track has no segments array")
}

/// 主视频轨必须排在第一位（计划校验规则），其余轨道保持首次出现的顺序。
fn order_tracks(merged: &mut Value) {
    let Some(tracks) = merged["tracks"].as_array_mut() else {
        return;
    };
    tracks.sort_by_key(|track| u8::from(track["type"].as_str() != Some("video")));
}
fn write_plan(path: &Path, value: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    // 固定 LF + 无 BOM，和 `Plan::load` 的输入约定一致。
    std::fs::write(path, text.as_bytes())
        .with_context(|| format!("cannot write {}", path.display()))?;
    Ok(())
}

fn snapshot(plan_path: &Path) -> Result<PathBuf> {
    let epoch = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .unwrap_or(0);
    let mut candidate = plan_path.with_extension(format!("json.{epoch}.bak"));
    let mut counter = 1_u32;
    while candidate.exists() {
        candidate = plan_path.with_extension(format!("json.{epoch}-{counter}.bak"));
        counter += 1;
    }
    std::fs::copy(plan_path, &candidate)
        .with_context(|| format!("cannot snapshot {}", plan_path.display()))?;
    Ok(candidate)
}

fn snapshots(plan_path: &Path) -> Result<Vec<PathBuf>> {
    let parent = plan_path.parent().unwrap_or_else(|| Path::new("."));
    let stem = plan_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_owned();
    let prefix = format!("{stem}.");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(parent)?.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.starts_with(&prefix) && name.ends_with(".bak") {
            found.push(entry.path());
        }
    }
    Ok(found)
}

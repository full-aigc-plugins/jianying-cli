//! capcut-cli 声明式 `compile` 规范的 Rust 实现与 Job v2 兼容入口。

use crate::plan::{Canvas, Plan, Segment as PlanSegment, Track};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const US: f64 = 1_000_000.0;

/// `compile --data --continue-on-error` 的稳定部分失败。
#[derive(Debug, Error)]
pub enum CompileError {
    #[error("compile --data completed with {failed} failed row(s)")]
    BatchPartial { failed: usize, results: Vec<Value> },
}

/// 声明式草稿编译规范。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileSpec {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub width: Option<u64>,
    #[serde(default)]
    pub height: Option<u64>,
    #[serde(default)]
    pub fps: Option<u64>,
    #[serde(default)]
    pub ratio: Option<String>,
    pub tracks: Vec<CompileTrack>,
    #[serde(default)]
    pub operations: Vec<CompileOperation>,
}

/// 声明式轨道。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileTrack {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    pub items: Vec<CompileItem>,
}

/// 声明式轨道项；时间单位为秒。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileItem {
    #[serde(default)]
    pub r#ref: Option<String>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub text: Option<String>,
    pub start: f64,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub font_size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub width: Option<u64>,
    #[serde(default)]
    pub height: Option<u64>,
    #[serde(default, rename = "type")]
    pub media_type: Option<String>,
    #[serde(default)]
    pub source_start: Option<f64>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub rotation: Option<f64>,
    #[serde(default)]
    pub scale: Option<f64>,
}

/// 编译后顺序执行的声明式操作。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", rename_all_fields = "camelCase")]
pub enum CompileOperation {
    Transition {
        target: String,
        slug: String,
        #[serde(default)]
        duration: Option<f64>,
        #[serde(default)]
        jianying: bool,
    },
    Filter {
        slug: String,
        start: f64,
        duration: f64,
        #[serde(default)]
        intensity: Option<f64>,
        #[serde(default)]
        track_name: Option<String>,
        #[serde(default)]
        jianying: bool,
    },
    Effect {
        slug: String,
        start: f64,
        duration: f64,
        #[serde(default)]
        params: Vec<f64>,
        #[serde(default)]
        track_name: Option<String>,
        #[serde(default)]
        jianying: bool,
    },
    Keyframe {
        target: String,
        property: String,
        time: f64,
        value: f64,
        #[serde(default)]
        easing: Option<String>,
    },
    AudioFade {
        target: String,
        #[serde(default)]
        fade_in: Option<f64>,
        #[serde(default)]
        fade_out: Option<f64>,
    },
    TextStyle {
        target: String,
        style: Value,
    },
    TextRanges {
        target: String,
        ranges: Vec<Value>,
    },
    Template {
        path: PathBuf,
        start: f64,
        duration: f64,
        #[serde(default)]
        text: Option<String>,
        #[serde(default)]
        r#ref: Option<String>,
    },
    Captions {
        path: PathBuf,
        #[serde(default)]
        track_name: Option<String>,
        #[serde(default)]
        style_ref: Option<String>,
        #[serde(default)]
        time_offset: Option<f64>,
    },
}

/// 从文件加载、去 BOM 并校验 compile 规范。
pub fn load(path: &Path) -> Result<CompileSpec> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("cannot read compile spec {}", path.display()))?;
    parse(raw.trim_start_matches('\u{feff}'))
}

/// 从 JSON 字符串解析并校验 compile 规范。
pub fn parse(raw: &str) -> Result<CompileSpec> {
    let spec: CompileSpec = serde_json::from_str(raw)
        .map_err(|error| anyhow!("compile: spec is not valid JSON: {error}"))?;
    validate(&spec)?;
    Ok(spec)
}

/// 对规范执行完整、无写入的形状校验。
pub fn validate(spec: &CompileSpec) -> Result<()> {
    validate_name(spec.name.as_deref())?;
    if spec.tracks.is_empty() {
        bail!("compile: spec.tracks must be a non-empty array");
    }
    if spec.width.is_some() ^ spec.height.is_some() {
        bail!("compile: width and height must be provided together");
    }
    if spec
        .width
        .is_some_and(|value| !(16..=8192).contains(&value))
        || spec
            .height
            .is_some_and(|value| !(16..=8192).contains(&value))
    {
        bail!("compile: canvas dimensions must be 16..8192");
    }
    if spec
        .fps
        .is_some_and(|fps| !crate::plan::FPS_VALUES.contains(&fps))
    {
        bail!("compile: fps must be one of {:?}", crate::plan::FPS_VALUES);
    }
    let mut refs = HashSet::new();
    for (track_index, track) in spec.tracks.iter().enumerate() {
        if !matches!(track.kind.as_str(), "video" | "audio" | "text") {
            bail!(
                "compile: tracks[{track_index}].type must be one of video|audio|text (got {})",
                track.kind
            );
        }
        if track.items.is_empty() {
            bail!("compile: tracks[{track_index}].items must be a non-empty array");
        }
        for (item_index, item) in track.items.iter().enumerate() {
            let where_ = format!("tracks[{track_index}].items[{item_index}]");
            finite_nonnegative(item.start, &format!("{where_}.start"))?;
            for (field, value) in [
                ("sourceStart", item.source_start),
                ("rotation", item.rotation),
                ("scale", item.scale),
                ("opacity", item.opacity),
                ("x", item.x),
                ("y", item.y),
                ("volume", item.volume),
            ] {
                if value.is_some_and(|value| !value.is_finite()) {
                    bail!("compile: {where_}.{field} must be a finite number");
                }
            }
            if item
                .speed
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                bail!("compile: {where_}.speed must be > 0");
            }
            if item
                .duration
                .is_some_and(|value| !value.is_finite() || value <= 0.0)
            {
                bail!("compile: {where_}.duration must be > 0 when provided");
            }
            if let Some(reference) = &item.r#ref {
                if reference.is_empty() {
                    bail!("compile: {where_}.ref must be a non-empty string");
                }
                if !refs.insert(reference.clone()) {
                    bail!("compile: duplicate ref '{reference}'");
                }
            }
            if track.kind == "text" {
                if item.text.as_deref().is_none_or(str::is_empty) {
                    bail!("compile: {where_}.text is required for text tracks");
                }
                if item.duration.is_none() {
                    bail!("compile: {where_}.duration (seconds) is required for text tracks");
                }
            } else {
                if item
                    .path
                    .as_ref()
                    .is_none_or(|path| path.as_os_str().is_empty())
                {
                    bail!(
                        "compile: {where_}.path is required for {} tracks",
                        track.kind
                    );
                }
                if item.media_type.as_deref() == Some("photo") && item.duration.is_none() {
                    bail!("compile: {where_}.duration (seconds) is required for photos");
                }
                if item
                    .media_type
                    .as_deref()
                    .is_some_and(|kind| kind != "video" && kind != "photo")
                {
                    bail!("compile: {where_}.type must be video or photo");
                }
            }
        }
    }
    for (index, operation) in spec.operations.iter().enumerate() {
        match operation {
            CompileOperation::Transition {
                target, duration, ..
            } => {
                require_ref(&refs, target, index)?;
                if duration.is_some_and(|value| !value.is_finite() || value <= 0.0) {
                    bail!("compile: operations[{index}].duration must be > 0");
                }
            }
            CompileOperation::Filter {
                start,
                duration,
                intensity,
                ..
            } => {
                finite_nonnegative(*start, &format!("operations[{index}].start"))?;
                finite_positive(*duration, &format!("operations[{index}].duration"))?;
                if intensity
                    .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
                {
                    bail!("compile: operations[{index}].intensity must be in 0..1");
                }
            }
            CompileOperation::Effect {
                start,
                duration,
                params,
                ..
            } => {
                finite_nonnegative(*start, &format!("operations[{index}].start"))?;
                finite_positive(*duration, &format!("operations[{index}].duration"))?;
                if params.iter().any(|value| !value.is_finite()) {
                    bail!("compile: operations[{index}].params must contain finite numbers");
                }
            }
            CompileOperation::Keyframe {
                target,
                property,
                time,
                value,
                easing,
            } => {
                require_ref(&refs, target, index)?;
                finite_nonnegative(*time, &format!("operations[{index}].time"))?;
                crate::timeline_ops::parse_keyframe_value(property, &value.to_string())?;
                if easing.as_deref().is_some_and(|value| {
                    !matches!(
                        value,
                        "linear" | "ease-in" | "ease-out" | "ease-in-out" | "hold"
                    )
                }) {
                    bail!(
                        "compile: operations[{index}]: Unsupported keyframe easing: {}",
                        easing.as_deref().unwrap_or_default()
                    );
                }
            }
            CompileOperation::AudioFade {
                target,
                fade_in,
                fade_out,
            } => {
                require_ref(&refs, target, index)?;
                for value in [fade_in, fade_out].into_iter().flatten() {
                    finite_nonnegative(*value, &format!("operations[{index}] fade"))?;
                }
            }
            CompileOperation::TextStyle { target, style } => {
                require_ref(&refs, target, index)?;
                if !style.is_object() {
                    bail!("compile: operations[{index}].style must be an object of styling keys");
                }
            }
            CompileOperation::TextRanges { target, ranges } => {
                require_ref(&refs, target, index)?;
                if ranges.is_empty() || ranges.iter().any(|range| !range.is_object()) {
                    bail!("compile: operations[{index}].ranges must be a non-empty array of range objects");
                }
            }
            CompileOperation::Template {
                start, duration, ..
            } => {
                finite_nonnegative(*start, &format!("operations[{index}].start"))?;
                finite_positive(*duration, &format!("operations[{index}].duration"))?;
            }
            CompileOperation::Captions { time_offset, .. } => {
                if time_offset.is_some_and(|value| !value.is_finite()) {
                    bail!("compile: operations[{index}].timeOffset must be finite");
                }
            }
        }
    }
    Ok(())
}

/// 返回无写入预检结果，并验证所有媒体和操作文件。
pub fn plan(spec: &CompileSpec, spec_dir: &Path) -> Result<Value> {
    validate(spec)?;
    let mut media = Vec::new();
    let mut refs = Vec::new();
    let mut items = 0usize;
    for track in &spec.tracks {
        for item in &track.items {
            items += 1;
            if let Some(reference) = &item.r#ref {
                refs.push(reference.clone());
            }
            if track.kind == "text" {
                continue;
            }
            let source_path = item
                .path
                .as_deref()
                .context("compile: media track item requires path")?;
            let path = resolve_path(spec_dir, source_path);
            if !path.is_file() {
                bail!(
                    "compile: media file not found: {} (resolved: {})",
                    source_path.display(),
                    path.display()
                );
            }
            if item.duration.is_none()
                && crate::probe::probe(&path)
                    .ok()
                    .is_none_or(|info| info.duration_us <= 0)
            {
                bail!(
                    "compile: duration omitted for {}, but ffprobe could not determine it",
                    source_path.display()
                );
            }
            media.push(path);
        }
    }
    for operation in &spec.operations {
        let path = match operation {
            CompileOperation::Template { path, .. } | CompileOperation::Captions { path, .. } => {
                Some(path)
            }
            _ => None,
        };
        if let Some(path) = path {
            let resolved = resolve_path(spec_dir, path);
            if !resolved.is_file() {
                bail!(
                    "compile: operation file not found: {} (resolved: {})",
                    path.display(),
                    resolved.display()
                );
            }
            media.push(resolved);
        }
    }
    Ok(json!({
        "ok":true,
        "name":spec.name.as_deref().unwrap_or("compiled-draft"),
        "canvas":{
            "width":spec.width.unwrap_or(1920),"height":spec.height.unwrap_or(1080),
            "fps":spec.fps.unwrap_or(30),"ratio":spec.ratio.as_deref().unwrap_or("original")
        },
        "tracks":spec.tracks.len(),"items":items,"operations":spec.operations.len(),
        "refs":refs,"media":media
    }))
}

/// 执行单个 compile 规范；所有写入先发生在隔离 staging，再原子提交。
pub fn compile(spec: &CompileSpec, spec_dir: &Path, output: &Path) -> Result<Value> {
    plan(spec, spec_dir)?;
    if output.exists() {
        bail!(
            "compile: draft directory already exists: {}",
            output.display()
        );
    }
    let parent = output
        .parent()
        .context("compile output requires a parent directory")?;
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".compile-{}.tmp", Uuid::new_v4().simple()));
    let result = compile_staged(spec, spec_dir, &staging, output);
    if result.is_err() && staging.exists() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    result
}

fn compile_staged(
    spec: &CompileSpec,
    spec_dir: &Path,
    staging: &Path,
    output: &Path,
) -> Result<Value> {
    let probes = build_probe_map(spec, spec_dir)?;
    let plan = to_plan(spec, spec_dir, &probes)?;
    let build_report = crate::draft::build(&plan, spec_dir, staging, None, &|path| {
        probes
            .get(path)
            .cloned()
            .or_else(|| crate::probe::probe(path).ok())
            .with_context(|| format!("compile: cannot probe {}", path.display()))
    })?;
    let mut refs = collect_refs(spec, staging)?;
    let mut warnings = Vec::<String>::new();
    let mut extra_segments = 0usize;
    let mut max_end = spec_max_end(spec);
    for operation in &spec.operations {
        match operation {
            CompileOperation::Transition {
                target,
                slug,
                duration,
                jianying,
            } => {
                crate::timeline_ops::transition(
                    staging,
                    resolve_ref(&refs, target)?,
                    slug,
                    duration.map(seconds_us).transpose()?,
                    *jianying,
                )?;
            }
            CompileOperation::Filter {
                slug,
                start,
                duration,
                intensity,
                track_name,
                jianying,
            } => {
                crate::timeline_ops::add_filter(
                    staging,
                    slug,
                    Some(seconds_us(*start)?),
                    Some(seconds_us(*duration)?),
                    false,
                    *intensity,
                    track_name.as_deref(),
                    *jianying,
                    None,
                    None,
                )?;
                max_end = max_end.max(seconds_us(start + duration)?);
                extra_segments += 1;
            }
            CompileOperation::Effect {
                slug,
                start,
                duration,
                params,
                track_name,
                jianying,
            } => {
                crate::timeline_ops::add_effect(
                    staging,
                    slug,
                    Some(seconds_us(*start)?),
                    Some(seconds_us(*duration)?),
                    false,
                    Some(params),
                    None,
                    track_name.as_deref(),
                    *jianying,
                    None,
                    None,
                    None,
                )?;
                max_end = max_end.max(seconds_us(start + duration)?);
                extra_segments += 1;
            }
            CompileOperation::Keyframe {
                target,
                property,
                time,
                value,
                easing,
            } => {
                let response = crate::timeline_ops::keyframe(
                    staging,
                    resolve_ref(&refs, target)?,
                    &[crate::timeline_ops::KeyframeInput {
                        property: property.clone(),
                        time_us: seconds_us(*time)?,
                        value: *value,
                        easing: easing.clone(),
                    }],
                    None,
                )?;
                warnings.extend(
                    response["warnings"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(str::to_owned),
                );
            }
            CompileOperation::AudioFade {
                target,
                fade_in,
                fade_out,
            } => {
                crate::timeline_ops::audio_fade(
                    staging,
                    resolve_ref(&refs, target)?,
                    fade_in.map(seconds_us).transpose()?.unwrap_or(0),
                    fade_out.map(seconds_us).transpose()?.unwrap_or(0),
                )?;
            }
            CompileOperation::TextStyle { target, style } => {
                apply_text_style(staging, resolve_ref(&refs, target)?, style)?;
            }
            CompileOperation::TextRanges { target, ranges } => {
                crate::caption_ops::style_ranges(staging, resolve_ref(&refs, target)?, ranges)?;
            }
            CompileOperation::Template {
                path,
                start,
                duration,
                text,
                r#ref,
            } => {
                let segment_id = apply_template(
                    staging,
                    &resolve_path(spec_dir, path),
                    seconds_us(*start)?,
                    seconds_us(*duration)?,
                    text.as_deref(),
                )?;
                if let Some(reference) = r#ref {
                    refs.insert(reference.clone(), segment_id);
                }
                max_end = max_end.max(seconds_us(start + duration)?);
                extra_segments += 1;
            }
            CompileOperation::Captions {
                path,
                track_name,
                style_ref,
                time_offset,
            } => {
                let before = caption_segment_ids(staging)?;
                crate::caption_ops::import_srt(
                    staging,
                    &resolve_path(spec_dir, path),
                    track_name.as_deref().unwrap_or("captions"),
                    seconds_us(time_offset.unwrap_or(0.0))?,
                    None,
                    None,
                    None,
                )?;
                let after = caption_segment_ids(staging)?;
                let new_ids: Vec<String> = after.difference(&before).cloned().collect();
                if let Some(style_ref) = style_ref {
                    let source = resolve_ref(&refs, style_ref)?.to_owned();
                    for target in &new_ids {
                        copy_text_style(staging, &source, target)?;
                    }
                }
                extra_segments += new_ids.len();
                max_end = max_end.max(
                    crate::draft::load_timeline(staging)?["duration"]
                        .as_i64()
                        .unwrap_or(0),
                );
            }
        }
    }
    crate::timeline_ops::mutate(staging, |timeline| {
        timeline["duration"] = json!(max_end);
        timeline["canvas_config"]["ratio"] = json!(spec.ratio.as_deref().unwrap_or("original"));
        Ok(json!({"ok":true}))
    })?;
    let mut timeline = crate::draft::load_timeline(staging)?;
    crate::draft::relocate_material_paths(&mut timeline, staging, output);
    crate::template::save_timeline_as(staging, &timeline, output)?;
    crate::draft::validate_bundle_as(staging, output)?;
    std::fs::rename(staging, output)?;
    let verification = crate::draft::verify(output)?;
    if verification["ok"] != true {
        bail!(
            "compiled draft failed verification: {}",
            verification["issues"]
        );
    }
    Ok(json!({
        "ok":true,"name":spec.name.as_deref().unwrap_or_else(|| output.file_name().and_then(|v|v.to_str()).unwrap_or("compiled-draft")),
        "draft_path":output,"file_path":output.join("draft_content.json"),
        "tracks":spec.tracks.len(),
        "segments":build_report.segments + extra_segments,
        "duration_us":max_end,"warnings":warnings,"refs":refs,
        "template":{"source":"rust-independent","seeded":false},
        "verification":verification
    }))
}

/// CLI 单稿、预检和 JSONL 批量入口。
#[allow(clippy::too_many_arguments)]
pub fn run_cli(
    spec_path: &Path,
    output: Option<&Path>,
    drafts: Option<&Path>,
    data: Option<&str>,
    check: bool,
    continue_on_error: bool,
) -> Result<Value> {
    let spec_dir = spec_path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(data) = data {
        if check || output.is_some() {
            bail!("compile: --data cannot be combined with --check/--plan or --out");
        }
        return compile_data(spec_path, drafts, data, continue_on_error);
    }
    let spec = load(spec_path)?;
    if check {
        let mut report = plan(&spec, spec_dir)?;
        report["checked"] = json!(true);
        report["write"] = json!(false);
        return Ok(report);
    }
    let output = output.map(Path::to_path_buf).unwrap_or_else(|| {
        drafts
            .unwrap_or_else(|| Path::new("."))
            .join(spec.name.as_deref().unwrap_or("compiled-draft"))
    });
    compile(&spec, spec_dir, &output)
}

fn compile_data(
    spec_path: &Path,
    drafts: Option<&Path>,
    data: &str,
    continue_on_error: bool,
) -> Result<Value> {
    let root = drafts.context("compile: --data requires --drafts <dir>")?;
    let raw_template: Value =
        serde_json::from_str(std::fs::read_to_string(spec_path)?.trim_start_matches('\u{feff}'))
            .map_err(|error| anyhow!("compile: spec is not valid JSON: {error}"))?;
    let rows = if data == "-" {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input)?;
        input
    } else {
        std::fs::read_to_string(data).with_context(|| format!("Rows file not found: {data}"))?
    };
    if rows.trim().is_empty() {
        bail!("compile --data: no rows to build");
    }
    let spec_dir = spec_path.parent().unwrap_or_else(|| Path::new("."));
    let mut plans = Vec::<(usize, Option<CompileSpec>, Option<PathBuf>, Option<String>)>::new();
    let mut names = HashSet::new();
    for (index, line) in rows.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let row_number = index + 1;
        let planned = (|| -> Result<(CompileSpec, PathBuf)> {
            let row: Value = serde_json::from_str(line)?;
            let row = row.as_object().context("row must be a JSON object")?;
            let substituted = substitute(&raw_template, row)?;
            let spec: CompileSpec = serde_json::from_value(substituted)?;
            validate(&spec)?;
            plan(&spec, spec_dir)?;
            let name = spec.name.as_deref().unwrap_or("compiled-draft");
            if !names.insert(name.to_owned()) {
                bail!("duplicate draft name '{name}'");
            }
            let output = root.join(name);
            if output.exists() {
                bail!("draft directory already exists: {}", output.display());
            }
            Ok((spec, output))
        })();
        match planned {
            Ok((spec, output)) => plans.push((row_number, Some(spec), Some(output), None)),
            Err(error) if continue_on_error => {
                plans.push((row_number, None, None, Some(format!("{error:#}"))))
            }
            Err(error) => {
                bail!("compile --data aborted at row {row_number}; no drafts written: {error:#}")
            }
        }
    }
    if plans.is_empty() {
        bail!("compile --data: no rows to build");
    }
    let mut results = Vec::new();
    let mut failed = 0usize;
    for (row, spec, output, error) in plans {
        if let Some(error) = error {
            failed += 1;
            results.push(json!({"row":row,"ok":false,"error":error}));
            continue;
        }
        let spec = spec.expect("validated row has spec");
        let output = output.expect("validated row has output");
        match compile(&spec, spec_dir, &output) {
            Ok(_) => results.push(json!({"row":row,"ok":true,"name":spec.name.as_deref().unwrap_or("compiled-draft"),"draft_path":output})),
            Err(error) => {
                failed += 1;
                results.push(json!({"row":row,"ok":false,"error":format!("{error:#}")}));
                if !continue_on_error {
                    bail!("compile --data aborted at row {row}: {error:#}");
                }
            }
        }
    }
    if failed > 0 {
        return Err(CompileError::BatchPartial { failed, results }.into());
    }
    Ok(Value::Array(results))
}

/// 从 Job v2 compatibility payload 执行 compile。
pub fn compile_payload(payload: &Value, base_dir: &Path, output: &Path) -> Result<Value> {
    let spec: CompileSpec = serde_json::from_value(payload.clone())?;
    validate(&spec)?;
    compile(&spec, base_dir, output)
}

fn validate_name(name: Option<&str>) -> Result<()> {
    let Some(name) = name else { return Ok(()) };
    if name.trim().is_empty() || matches!(name, "." | "..") {
        bail!("compile: spec.name must be a non-empty folder name");
    }
    let path = Path::new(name);
    if path.is_absolute()
        || name.contains(['/', '\\'])
        || (name.len() >= 2 && name.as_bytes()[1] == b':')
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        bail!("compile: spec.name takes a plain folder name, not a path (got \"{name}\")");
    }
    Ok(())
}

fn require_ref(refs: &HashSet<String>, target: &str, index: usize) -> Result<()> {
    if !refs.contains(target) {
        bail!("compile: operations[{index}].target must reference a declared item ref");
    }
    Ok(())
}

fn finite_nonnegative(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("compile: {field} must be a number >= 0 (seconds)");
    }
    Ok(())
}

fn finite_positive(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("compile: {field} must be a number > 0 (seconds)");
    }
    Ok(())
}

fn seconds_us(value: f64) -> Result<i64> {
    if !value.is_finite() || value < 0.0 || value * US > i64::MAX as f64 {
        bail!("compile: time is outside the supported range");
    }
    Ok((value * US).round() as i64)
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn build_probe_map(
    spec: &CompileSpec,
    base: &Path,
) -> Result<HashMap<PathBuf, crate::probe::MediaInfo>> {
    let mut output = HashMap::new();
    for track in &spec.tracks {
        if track.kind == "text" {
            continue;
        }
        for item in &track.items {
            let source_path = item
                .path
                .as_deref()
                .context("compile: media track item requires path")?;
            let path = resolve_path(base, source_path);
            let probed = crate::probe::probe(&path).ok();
            let duration_us = match (item.duration, probed.as_ref()) {
                (Some(duration), Some(info)) if item.media_type.as_deref() != Some("photo") => {
                    let requested = seconds_us(duration)?;
                    if info.duration_us > 0 && requested > info.duration_us + 10_000 {
                        bail!(
                            "compile: duration for {} exceeds source duration ({} > {}us)",
                            path.display(),
                            requested,
                            info.duration_us
                        );
                    }
                    requested
                }
                (Some(duration), _) => seconds_us(duration)?,
                (None, Some(info)) if info.duration_us > 0 => info.duration_us,
                _ => bail!(
                    "compile: duration omitted for {}, but ffprobe could not determine it",
                    path.display()
                ),
            };
            output.insert(
                path.clone(),
                crate::probe::MediaInfo {
                    path: path.to_string_lossy().into_owned(),
                    duration_us,
                    width: item.width.unwrap_or_else(|| {
                        probed
                            .as_ref()
                            .map(|info| info.width)
                            .unwrap_or(spec.width.unwrap_or(1920))
                    }),
                    height: item.height.unwrap_or_else(|| {
                        probed
                            .as_ref()
                            .map(|info| info.height)
                            .unwrap_or(spec.height.unwrap_or(1080))
                    }),
                    has_video: track.kind == "video",
                    has_audio: track.kind == "audio",
                    frame_rate: None,
                    streams: Vec::new(),
                    is_image: item.media_type.as_deref() == Some("photo"),
                },
            );
        }
    }
    Ok(output)
}

fn to_plan(
    spec: &CompileSpec,
    base: &Path,
    probes: &HashMap<PathBuf, crate::probe::MediaInfo>,
) -> Result<Plan> {
    let mut tracks = Vec::new();
    for track in &spec.tracks {
        let mut segments = Vec::new();
        for item in &track.items {
            let source = item.path.as_ref().map(|path| resolve_path(base, path));
            let duration_us = match item.duration {
                Some(duration) => seconds_us(duration)?,
                None => {
                    probes
                        .get(source.as_ref().context("media path missing")?)
                        .context("probe missing")?
                        .duration_us
                }
            };
            segments.push(PlanSegment {
                start_us: seconds_us(item.start)?,
                duration_us,
                source,
                source_start_us: seconds_us(item.source_start.unwrap_or(0.0))?,
                source_duration_us: Some(
                    (duration_us as f64 * item.speed.unwrap_or(1.0)).round() as i64
                ),
                speed: item.speed,
                volume: item.volume,
                photo: item.media_type.as_deref() == Some("photo"),
                scale: item.scale,
                x: item.x,
                y: item.y,
                rotation: item.rotation,
                opacity: item.opacity,
                text: item.text.clone(),
                size: item.font_size,
                color: item.color.clone(),
                ..PlanSegment::default()
            });
        }
        tracks.push(Track {
            kind: track.kind.clone(),
            name: track.name.clone(),
            segments,
        });
    }
    Ok(Plan {
        schema: crate::plan::SCHEMA.to_owned(),
        name: spec
            .name
            .clone()
            .unwrap_or_else(|| "compiled-draft".to_owned()),
        canvas: Canvas {
            width: spec.width.unwrap_or(1920),
            height: spec.height.unwrap_or(1080),
            fps: spec.fps.unwrap_or(30),
        },
        tracks,
        allow_vip: false,
        parent: Some(base.to_path_buf()),
    })
}

fn collect_refs(spec: &CompileSpec, draft: &Path) -> Result<BTreeMap<String, String>> {
    let timeline = crate::draft::load_timeline(draft)?;
    let tracks = timeline["tracks"]
        .as_array()
        .context("compiled tracks missing")?;
    let mut refs = BTreeMap::new();
    for (track_index, track) in spec.tracks.iter().enumerate() {
        let segments = tracks
            .get(track_index)
            .and_then(|track| track["segments"].as_array())
            .context("compiled track item mismatch")?;
        for (item_index, item) in track.items.iter().enumerate() {
            if let Some(reference) = &item.r#ref {
                let id = segments
                    .get(item_index)
                    .and_then(|segment| segment["id"].as_str())
                    .context("compiled ref item mismatch")?;
                refs.insert(reference.clone(), id.to_owned());
            }
        }
    }
    Ok(refs)
}

fn resolve_ref<'a>(refs: &'a BTreeMap<String, String>, name: &str) -> Result<&'a str> {
    refs.get(name)
        .map(String::as_str)
        .with_context(|| format!("compile: unresolved ref '{name}'"))
}

fn spec_max_end(spec: &CompileSpec) -> i64 {
    spec.tracks
        .iter()
        .flat_map(|track| &track.items)
        .filter_map(|item| {
            item.duration
                .map(|duration| ((item.start + duration) * US).round() as i64)
        })
        .max()
        .unwrap_or(0)
}

fn substitute(value: &Value, row: &Map<String, Value>) -> Result<Value> {
    match value {
        Value::String(text) => {
            let pattern = regex::Regex::new(r#"\{\{\s*([^{}]+?)\s*\}\}"#)?;
            let mut error = None;
            let replaced = pattern.replace_all(text, |capture: &regex::Captures<'_>| {
                let key = capture.get(1).map(|value| value.as_str().trim()).unwrap_or_default();
                match row.get(key) {
                    Some(Value::String(value)) => value.clone(),
                    Some(Value::Number(value)) => value.to_string(),
                    Some(Value::Bool(value)) => value.to_string(),
                    Some(_) => { error = Some(anyhow!("compile: row value for {{{{{key}}}}} must be a string, number, or boolean")); String::new() }
                    None => { error = Some(anyhow!("compile: no value for placeholder {{{{{key}}}}} in row")); String::new() }
                }
            });
            if let Some(error) = error {
                return Err(error);
            }
            Ok(Value::String(replaced.into_owned()))
        }
        Value::Array(values) => Ok(Value::Array(
            values
                .iter()
                .map(|value| substitute(value, row))
                .collect::<Result<_>>()?,
        )),
        Value::Object(values) => Ok(Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.clone(), substitute(value, row)?)))
                .collect::<Result<_>>()?,
        )),
        other => Ok(other.clone()),
    }
}

fn caption_segment_ids(draft: &Path) -> Result<BTreeSet<String>> {
    Ok(crate::draft::load_timeline(draft)?["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|track| track["type"] == "text")
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .filter_map(|segment| segment["id"].as_str().map(str::to_owned))
        .collect())
}

fn apply_text_style(draft: &Path, segment_id: &str, style: &Value) -> Result<Value> {
    let style = style
        .as_object()
        .context("text style must be an object")?
        .clone();
    crate::timeline_ops::mutate(draft, |timeline| {
        let material_id = segment_material_id(timeline, segment_id)?;
        let material = timeline["materials"]["texts"]
            .as_array_mut()
            .context("materials.texts missing")?
            .iter_mut()
            .find(|material| material["id"].as_str() == Some(&material_id))
            .context("text material missing")?;
        let mappings = [
            ("alpha", "text_alpha"),
            ("vertical", "typesetting"),
            ("fixedWidth", "fixed_width"),
            ("fixedHeight", "fixed_height"),
            ("shadowAlpha", "shadow_alpha"),
            ("shadowAngle", "shadow_angle"),
            ("shadowColor", "shadow_color"),
            ("shadowDistance", "shadow_distance"),
            ("shadowSmoothing", "shadow_smoothing"),
            ("borderWidth", "border_width"),
            ("borderColor", "border_color"),
            ("borderAlpha", "border_alpha"),
            ("bgColor", "background_color"),
            ("bgAlpha", "background_alpha"),
            ("bgStyle", "background_style"),
            ("bgRoundRadius", "background_round_radius"),
            ("bgWidth", "background_width"),
            ("bgHeight", "background_height"),
            ("bgHOffset", "background_horizontal_offset"),
            ("bgVOffset", "background_vertical_offset"),
        ];
        for (source, target) in mappings {
            if let Some(value) = style.get(source) {
                material[target] = if source == "vertical" {
                    json!(value.as_bool().unwrap_or(false) as u8)
                } else {
                    value.clone()
                };
            }
        }
        if let Some(value) = style.get("shadow").and_then(Value::as_bool) {
            material["has_shadow"] = json!(value);
        }
        if style.keys().any(|key| key.starts_with("border")) {
            material["has_border"] = json!(true);
        }
        if style.keys().any(|key| key.starts_with("bg")) {
            material["has_text_shadow_config"] = json!(true);
        }
        Ok(json!({"ok":true,"segment_id":segment_id,"material_id":material_id}))
    })
}

fn segment_material_id(timeline: &Value, segment_id: &str) -> Result<String> {
    timeline["tracks"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|track| track["segments"].as_array().into_iter().flatten())
        .find(|segment| segment["id"].as_str() == Some(segment_id))
        .and_then(|segment| segment["material_id"].as_str())
        .map(str::to_owned)
        .with_context(|| format!("Segment not found: {segment_id}"))
}

fn copy_text_style(draft: &Path, source_segment: &str, target_segment: &str) -> Result<Value> {
    crate::timeline_ops::mutate(draft, |timeline| {
        let source_id = segment_material_id(timeline, source_segment)?;
        let target_id = segment_material_id(timeline, target_segment)?;
        let texts = timeline["materials"]["texts"]
            .as_array_mut()
            .context("materials.texts missing")?;
        let source = texts
            .iter()
            .find(|material| material["id"].as_str() == Some(&source_id))
            .cloned()
            .context("source text material missing")?;
        let target = texts
            .iter_mut()
            .find(|material| material["id"].as_str() == Some(&target_id))
            .context("target text material missing")?;
        for key in [
            "alignment",
            "font_size",
            "text_color",
            "typesetting",
            "letter_spacing",
            "line_spacing",
            "line_feed",
            "line_max_width",
            "force_apply_line_max_width",
            "fixed_width",
            "fixed_height",
            "has_shadow",
            "shadow_alpha",
            "shadow_angle",
            "shadow_color",
            "shadow_distance",
            "shadow_smoothing",
            "border_width",
            "border_color",
            "border_alpha",
            "has_border",
            "has_text_shadow_config",
            "background_color",
            "background_alpha",
            "background_style",
            "background_round_radius",
            "background_width",
            "background_height",
            "background_horizontal_offset",
            "background_vertical_offset",
        ] {
            if let Some(value) = source.get(key) {
                target[key] = value.clone();
            }
        }
        Ok(json!({"ok":true,"source":source_segment,"target":target_segment}))
    })
}

fn apply_template(
    draft: &Path,
    path: &Path,
    start_us: i64,
    duration_us: i64,
    text: Option<&str>,
) -> Result<String> {
    let template: Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let kind = template["type"]
        .as_str()
        .context("template.type missing")?
        .to_owned();
    let mut segment = template["segment"].clone();
    let mut material = template["material"]["data"].clone();
    let bucket = template["material"]["type"]
        .as_str()
        .context("template.material.type missing")?
        .to_owned();
    let segment_id = Uuid::new_v4().to_string();
    let material_id = Uuid::new_v4().to_string();
    segment["id"] = json!(segment_id);
    segment["material_id"] = json!(material_id);
    segment["target_timerange"] = json!({"start":start_us,"duration":duration_us});
    if segment["source_timerange"].is_object() {
        segment["source_timerange"] = json!({"start":0,"duration":duration_us});
    }
    material["id"] = json!(material_id);
    if let (Some(text), Some(content)) = (text, material["content"].as_str()) {
        if let Ok(mut parsed) = serde_json::from_str::<Value>(content) {
            parsed["text"] = json!(text);
            if let Some(first) = parsed["styles"]
                .as_array_mut()
                .and_then(|styles| styles.first_mut())
            {
                first["range"] = json!([0, text.encode_utf16().count()]);
            }
            material["content"] = json!(serde_json::to_string(&parsed)?);
        }
    }
    crate::timeline_ops::mutate(draft, |timeline| {
        let materials = timeline["materials"]
            .as_object_mut()
            .context("materials missing")?;
        materials
            .entry(bucket.clone())
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .context("template material bucket not array")?
            .push(material.clone());
        let mut extra_ids = Vec::new();
        for extra in template["extra_materials"].as_array().into_iter().flatten() {
            let extra_bucket = extra["type"]
                .as_str()
                .context("extra material type missing")?
                .to_owned();
            let mut data = extra["data"].clone();
            let id = Uuid::new_v4().to_string();
            data["id"] = json!(id);
            materials
                .entry(extra_bucket)
                .or_insert_with(|| json!([]))
                .as_array_mut()
                .context("extra material bucket not array")?
                .push(data);
            extra_ids.push(id);
        }
        segment["extra_material_refs"] = json!(extra_ids);
        let tracks = timeline["tracks"]
            .as_array_mut()
            .context("tracks missing")?;
        let index = if let Some(index) = tracks
            .iter()
            .position(|track| track["type"].as_str() == Some(&kind))
        {
            index
        } else {
            tracks.push(json!({"id":Uuid::new_v4().to_string(),"type":kind,"name":template["name"].as_str().unwrap_or(&kind),"attribute":0,"segments":[],"is_default_name":true,"flag":0}));
            tracks.len() - 1
        };
        segment["raw_segment_id"] = tracks[index]["id"].clone();
        tracks[index]["segments"]
            .as_array_mut()
            .context("template target segments missing")?
            .push(segment.clone());
        Ok(json!({"ok":true,"segment_id":segment_id}))
    })?;
    Ok(segment_id)
}

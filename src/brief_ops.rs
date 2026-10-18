//! 简报（brief）驱动的初稿生成：素材 + 时长 + 平台 → 可编辑计划。
//!
//! 这是"生成"的诚实形态：**确定性装配**，不是模型创作。给定素材与目标时长，
//! 它决定用哪些片段、各取多长、放第几秒，并如实报告缺口（素材不够、无音轨等）。
//! 生成结果是普通 `plan/v1`，可以继续被 `plan`/`timeline`/`eval`/`loop` 处理。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 平台画布预设（与剪映/抖音常用比例一致）。
fn platform_canvas(platform: &str) -> Option<(u64, u64, u64)> {
    match platform {
        "douyin" | "vertical" => Some((1080, 1920, 30)),
        "bilibili" | "landscape" => Some((1920, 1080, 30)),
        "xiaohongshu" => Some((1080, 1440, 30)),
        _ => None,
    }
}

struct Media {
    path: PathBuf,
    duration_us: i64,
    is_image: bool,
}

/// 按简报生成一份计划，返回报告。
pub fn generate(brief_path: &Path, out: &Path, force: bool) -> Result<Value> {
    let raw = std::fs::read_to_string(brief_path)
        .with_context(|| format!("cannot read brief {}", brief_path.display()))?;
    let brief: Value = serde_json::from_str(raw.trim_start_matches('\u{feff}'))
        .with_context(|| format!("brief {} is not valid JSON", brief_path.display()))?;
    if brief["schema"].as_str() != Some("jianying-brief/v1") {
        bail!(
            "unsupported brief schema {:?} (expected jianying-brief/v1)",
            brief["schema"]
        );
    }
    let base = brief_path.parent().unwrap_or_else(|| Path::new("."));

    let (width, height, fps) = match brief["canvas"].as_object() {
        Some(canvas) => (
            canvas
                .get("width")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow::anyhow!("canvas.width is required"))?,
            canvas
                .get("height")
                .and_then(Value::as_u64)
                .ok_or_else(|| anyhow::anyhow!("canvas.height is required"))?,
            canvas.get("fps").and_then(Value::as_u64).unwrap_or(30),
        ),
        None => {
            let platform = brief["platform"].as_str().unwrap_or("bilibili");
            platform_canvas(platform).ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown platform {platform:?} and no canvas given; \
                     use douyin/bilibili/xiaohongshu/vertical/landscape or an explicit canvas"
                )
            })?
        }
    };

    let target_us = brief["target_duration_us"]
        .as_i64()
        .filter(|value| *value > 0)
        .ok_or_else(|| anyhow::anyhow!("target_duration_us must be a positive integer"))?;

    // 素材：文件直接收，目录按名字排序展开。
    let mut media_paths: Vec<PathBuf> = Vec::new();
    for entry in brief["media"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("media must be an array"))?
    {
        let raw_path = entry
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("media entries must be strings"))?;
        let path = resolve(base, raw_path);
        if path.is_dir() {
            let mut found: Vec<PathBuf> = std::fs::read_dir(&path)?
                .flatten()
                .map(|item| item.path())
                .filter(|item| item.is_file())
                .collect();
            found.sort();
            media_paths.extend(found);
        } else if path.is_file() {
            media_paths.push(path);
        } else {
            bail!("media {} does not exist", path.display());
        }
    }
    if media_paths.is_empty() {
        bail!("brief resolves to no media files");
    }

    let mut warnings: Vec<String> = Vec::new();
    let mut media: Vec<Media> = Vec::new();
    for path in media_paths {
        match crate::probe::probe(&path) {
            Ok(info) if info.has_video || info.is_image => media.push(Media {
                path,
                duration_us: if info.is_image { 0 } else { info.duration_us },
                is_image: info.is_image,
            }),
            Ok(_) => warnings.push(format!("skipped {}: no video stream", path.display())),
            Err(error) => warnings.push(format!("skipped {}: {error:#}", path.display())),
        }
    }
    if media.is_empty() {
        bail!("none of the media files can be used as video");
    }

    // 装配：按顺序取素材，直到达到目标时长；最后一段按剩余时长裁剪。
    let mut segments: Vec<Value> = Vec::new();
    let mut cursor = 0_i64;
    for item in &media {
        if cursor >= target_us {
            break;
        }
        let remaining = target_us - cursor;
        let available = if item.is_image {
            remaining
        } else {
            item.duration_us.max(0)
        };
        if available <= 0 {
            continue;
        }
        let take = available.min(remaining);
        let mut segment = json!({
            "start_us": cursor,
            "duration_us": take,
            "source": relative_or_absolute(base, &item.path),
        });
        if item.is_image {
            segment["photo"] = json!(true);
        }
        segments.push(segment);
        cursor += take;
    }
    if segments.is_empty() {
        bail!("could not build any shot from the given media");
    }
    if cursor < target_us {
        warnings.push(format!(
            "short-of-target: only {}us of media available for a {}us target",
            cursor, target_us
        ));
    }

    let mut tracks = vec![json!({"type": "video", "name": "main", "segments": segments})];

    // 背景音乐：从 0 起铺到成片时长（可短于成片）。
    if let Some(music) = brief["music"].as_str() {
        let path = resolve(base, music);
        let info = crate::probe::probe(&path)
            .with_context(|| format!("cannot probe music {}", path.display()))?;
        let take = info.duration_us.min(cursor);
        if take > 0 {
            tracks.push(json!({
                "type": "audio", "name": "music",
                "segments": [{
                    "start_us": 0,
                    "duration_us": take,
                    "source": relative_or_absolute(base, &path),
                }]
            }));
        } else {
            warnings.push(format!("music {} has no usable duration", path.display()));
        }
    }

    // 字幕：SRT 直接转文本轨。
    if let Some(captions) = brief["captions"].as_str() {
        let path = resolve(base, captions);
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("cannot read captions {}", path.display()))?;
        let cues = crate::srt::parse(&text)?;
        if cues.is_empty() {
            warnings.push(format!("captions {} has no cues", path.display()));
        } else {
            let options = crate::srt::SrtOptions {
                offset_us: 0,
                size: brief["captions_size"].as_f64(),
                align: Some(1),
                color: None,
                border_width: None,
                y: None,
                bold: None,
            };
            let track = crate::srt::cues_to_text_track(cues, &options);
            tracks.push(serde_json::to_value(track)?);
        }
    }

    let plan = json!({
        "schema": "jianying-cli-plan/v1",
        "name": brief["name"].as_str().unwrap_or("generated"),
        "canvas": {"width": width, "height": height, "fps": fps},
        "tracks": tracks,
    });
    let parsed: crate::plan::Plan = serde_json::from_value(plan.clone())
        .map_err(|error| anyhow::anyhow!("generated plan is invalid: {error}"))?;
    parsed.validate()?;
    if out.exists() && !force {
        bail!(
            "{} already exists; pass --force to overwrite",
            out.display()
        );
    }
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut text = serde_json::to_string_pretty(&plan)?;
    text.push('\n');
    std::fs::write(out, text.as_bytes())
        .with_context(|| format!("cannot write {}", out.display()))?;

    Ok(json!({
        "ok": true,
        "out": out.to_string_lossy(),
        "canvas": {"width": width, "height": height, "fps": fps},
        "duration_us": parsed.total_duration(),
        "target_duration_us": target_us,
        "shots": parsed.tracks.first().map(|track| track.segments.len()).unwrap_or(0),
        "tracks": parsed.tracks.len(),
        "warnings": warnings,
    }))
}

fn resolve(base: &Path, raw: &str) -> PathBuf {
    let path = PathBuf::from(raw);
    if path.is_absolute() {
        path
    } else {
        base.join(path)
    }
}

/// 计划内优先写相对路径，便于整个目录搬移；跨盘或上级目录时退回绝对路径。
fn relative_or_absolute(base: &Path, path: &Path) -> String {
    match path.strip_prefix(base) {
        Ok(relative) => relative.to_string_lossy().into_owned(),
        Err(_) => path.to_string_lossy().into_owned(),
    }
}

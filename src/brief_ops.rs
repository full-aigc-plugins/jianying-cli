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
    has_audio: bool,
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
                has_audio: info.has_audio,
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
    let mut original_audio: Vec<Value> = Vec::new();
    let keep_original_audio = brief["keep_original_audio"].as_bool().unwrap_or(true);
    // `max_shot_us` 给了就把同一段素材切成多个镜头（源窗口连续前移），
    // 这是"整段素材铺满"与"真的剪辑"之间最小的一步。
    let max_shot_us = brief["max_shot_us"].as_i64().filter(|value| *value > 0);
    let mut cursor = 0_i64;
    for item in &media {
        if cursor >= target_us {
            break;
        }
        let mut available = if item.is_image {
            target_us - cursor
        } else {
            item.duration_us.max(0)
        };
        let mut source_offset = 0_i64;
        while available > 0 && cursor < target_us {
            let remaining = target_us - cursor;
            let mut take = available.min(remaining);
            if let Some(limit) = max_shot_us {
                take = take.min(limit);
            }
            if take <= 0 {
                break;
            }
            let mut segment = json!({
                "start_us": cursor,
                "duration_us": take,
                "source": relative_or_absolute(base, &item.path),
            });
            if source_offset > 0 {
                segment["source_start_us"] = json!(source_offset);
            }
            if item.is_image {
                segment["photo"] = json!(true);
            }
            segments.push(segment);
            if keep_original_audio && item.has_audio && !item.is_image {
                original_audio.push(json!({
                    "start_us": cursor,
                    "duration_us": take,
                    "source": relative_or_absolute(base, &item.path),
                    "source_start_us": source_offset,
                }));
            }
            cursor += take;
            source_offset += take;
            available -= take;
            if item.is_image {
                // 图片没有"剩余素材"，切完一段即可。
                available = 0;
            }
        }
    }
    // min_shot_us：过短的收尾镜头并入同源的前一个镜头，避免切出碎片；
    // 上限因此可能被略微突破（下限关乎可用性，上限只是节奏偏好），合并后给出提示。
    if let Some(min_shot) = brief["min_shot_us"].as_i64().filter(|value| *value > 0) {
        while segments.len() >= 2 {
            let last_duration = segments
                .last()
                .and_then(|segment| segment["duration_us"].as_i64())
                .unwrap_or(0);
            if last_duration >= min_shot {
                break;
            }
            let last = segments.pop().expect("segments is not empty");
            let previous = segments.last_mut().expect("segments keeps at least one");
            if previous["source"] != last["source"] {
                segments.push(last);
                break;
            }
            let merged = previous["duration_us"].as_i64().unwrap_or(0) + last_duration;
            previous["duration_us"] = json!(merged);
            if let Some(max_shot) = max_shot_us {
                if merged > max_shot {
                    warnings.push(format!(
                        "merged-short-tail: a shot was extended to {merged}us to honour min_shot_us (max_shot_us is {max_shot})"
                    ));
                }
            }
        }
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
    // 画面自带的音轨由代理渲染直接混入，这里不再重复落一条 audio 轨，
    // 避免同一段声音被混两遍、也避免计划里出现与画面重复的时间线。
    let _ = &original_audio;

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

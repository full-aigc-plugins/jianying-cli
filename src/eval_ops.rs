//! 草稿评估与修正循环（dream-loop 式"评估 → 修正 → 再评估"的可判定版本）。
//!
//! 只读草稿 JSON，不渲染、不探测媒体，因此结论是确定性的、可复现的。
//! 所有指标与问题都带结构化字段，便于 Agent 直接消费；评分公式写死在本文件里，
//! 不允许调用方传入自定义权重，避免"自己给自己打分"。

use anyhow::{Context, Result};
use serde_json::{json, Map, Value};
use std::path::Path;

/// 评分权重：主轨覆盖 40、连续性 20、字幕覆盖 20、音频覆盖 20。
const W_MAIN: f64 = 40.0;
const W_CONTINUITY: f64 = 20.0;
const W_CAPTION: f64 = 20.0;
const W_AUDIO: f64 = 20.0;

/// 评估一份草稿，返回指标、问题与 0–100 分。
pub fn evaluate(draft: &Path) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)
        .with_context(|| format!("cannot read draft {}", draft.display()))?;
    let duration = timeline["duration"].as_i64().unwrap_or(0);
    let mut main_segments: Vec<(i64, i64)> = Vec::new();
    let mut counts: Map<String, Value> = Map::new();
    let mut track_spans: Vec<(String, i64)> = Vec::new();
    let mut issues: Vec<Value> = Vec::new();

    for track in timeline["tracks"].as_array().into_iter().flatten() {
        let kind = track["type"].as_str().unwrap_or("unknown").to_owned();
        let segments = track["segments"].as_array().into_iter().flatten();
        let mut segment_count = 0_i64;
        let mut covered = 0_i64;
        let mut end = 0_i64;
        for segment in segments {
            let start = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
            let length = segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            segment_count += 1;
            covered += length.max(0);
            end = end.max(start + length.max(0));
            if kind == "video" {
                main_segments.push((start, start + length.max(0)));
            }
        }
        track_spans.push((kind.clone(), end));
        counts.insert(kind.clone(), json!(segment_count));
        if segment_count == 0 {
            issues.push(json!({"code":"empty-track","track":kind}));
        }
        if kind == "text" && segment_count > 0 {
            counts.insert(
                "caption_coverage_permille".into(),
                json!(permille(covered, duration)),
            );
        }
    }

    // 主轨连续性：按起点排序后检查空洞与重叠。
    main_segments.sort_by_key(|(start, _)| *start);
    let main_coverage = if duration > 0 {
        let covered: i64 = main_segments.iter().map(|(s, e)| (e - s).max(0)).sum();
        (covered as f64 / duration as f64).min(1.0)
    } else {
        0.0
    };
    let mut cursor = 0_i64;
    let mut gap_total = 0_i64;
    for (start, end) in &main_segments {
        if *start > cursor {
            gap_total += *start - cursor;
            issues.push(json!({
                "code": "main-track-gap",
                "start_us": cursor,
                "duration_us": start - cursor
            }));
        } else if *start < cursor {
            issues.push(json!({
                "code": "main-track-overlap",
                "start_us": *start,
                "overlap_us": cursor - *start
            }));
        }
        cursor = cursor.max(*end);
    }
    if main_segments.is_empty() {
        issues.push(json!({"code": "main-track-missing"}));
    }
    let continuity = if duration > 0 {
        (1.0 - (gap_total as f64 / duration as f64)).clamp(0.0, 1.0)
    } else {
        0.0
    };

    let declared_end = track_spans.iter().map(|(_, end)| *end).max().unwrap_or(0);
    if duration > 0 && declared_end > 0 && duration != declared_end {
        issues.push(json!({
            "code": "duration-mismatch",
            "declared_us": duration,
            "computed_us": declared_end
        }));
    }

    let width = timeline["canvas_config"]["width"].as_u64().unwrap_or(0);
    let height = timeline["canvas_config"]["height"].as_u64().unwrap_or(0);
    if width < 16 || height < 16 {
        issues.push(json!({"code":"canvas-invalid","width":width,"height":height}));
    }

    let caption_permille = caption_coverage(&timeline, duration);
    let audio_coverage = track_coverage(&timeline, "audio", duration);

    // 只有存在的维度才参与评分，缺失维度不用 0 分惩罚。
    let mut total_weight = W_MAIN + W_CONTINUITY;
    let mut score = main_coverage * W_MAIN + continuity * W_CONTINUITY;
    if let Some(value) = caption_permille {
        total_weight += W_CAPTION;
        score += (value as f64 / 1000.0) * W_CAPTION;
    }
    if let Some(value) = audio_coverage {
        total_weight += W_AUDIO;
        score += value * W_AUDIO;
    }
    let score = if total_weight > 0.0 {
        (score / total_weight * 100.0).round().clamp(0.0, 100.0)
    } else {
        0.0
    };
    let score = score as i64;

    Ok(json!({
        "draft": draft.to_string_lossy(),
        "duration_us": duration,
        "canvas": {"width": width, "height": height},
        "segments": counts,
        "main_track": {
            "segments": main_segments.len(),
            "coverage_permille": permille(
                main_segments.iter().map(|(s, e)| (e - s).max(0)).sum::<i64>(),
                duration
            ),
            "gap_us": gap_total
        },
        "caption_coverage_permille": caption_permille,
        "audio_coverage_permille": audio_coverage.map(|value| (value * 1000.0).round() as i64),
        "score": score,
        "issues": issues,
    }))
}

fn permille(part: i64, whole: i64) -> i64 {
    if whole <= 0 {
        return 0;
    }
    ((part as f64 / whole as f64) * 1000.0).round() as i64
}

fn caption_coverage(timeline: &Value, duration: i64) -> Option<i64> {
    let mut total = 0_i64;
    let mut seen = false;
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        if track["type"] != "text" {
            continue;
        }
        for segment in track["segments"].as_array().into_iter().flatten() {
            seen = true;
            total += segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0)
                .max(0);
        }
    }
    seen.then(|| permille(total, duration))
}

fn track_coverage(timeline: &Value, kind: &str, duration: i64) -> Option<f64> {
    let mut total = 0_i64;
    let mut seen = false;
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        if track["type"].as_str() != Some(kind) {
            continue;
        }
        for segment in track["segments"].as_array().into_iter().flatten() {
            seen = true;
            total += segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0)
                .max(0);
        }
    }
    if !seen || duration <= 0 {
        return None;
    }
    Some((total as f64 / duration as f64).clamp(0.0, 1.0))
}

/// 依据问题挑一个可安全自动执行的修复；返回 `None` 表示没有可自动修的问题。
pub fn next_repair(report: &Value) -> Option<&'static str> {
    let issues = report["issues"].as_array()?;
    if issues.iter().any(|issue| issue["code"] == "main-track-gap") {
        return Some("close-main-track-gaps");
    }
    if issues.iter().any(|issue| issue["code"] == "empty-track") {
        return Some("drop-empty-tracks");
    }
    None
}

/// 执行一个具名修复。所有修复都经过事务（快照 → 校验 → 原子提交）。
pub fn repair(draft: &Path, kind: &str) -> Result<Value> {
    match kind {
        "close-main-track-gaps" => crate::timeline_ops::close_main_track_gaps(draft),
        "drop-empty-tracks" => crate::timeline_ops::drop_empty_tracks(draft),
        other => anyhow::bail!("unknown repair {other:?}"),
    }
}

/// 深度评估：在结构评分之外，检查每个片段引用的媒体是否真的可用。
///
/// 只做两件"错了就一定出问题"的判断，避免把评估变成昂贵的转码：
/// 1. `media-missing`：素材文件不存在（时间线会黑屏/静音）
/// 2. `source-exceeds-media`：源窗口超出素材真实时长（末尾会冻结）
///
/// 每个问题按固定分值扣分，扣分项写进报告的 `penalties`，便于审计。
pub fn evaluate_deep(draft: &Path) -> Result<Value> {
    let mut report = evaluate(draft)?;
    let timeline = crate::draft::load_timeline(draft)?;
    let mut paths: std::collections::BTreeMap<String, String> = std::collections::BTreeMap::new();
    for bucket in ["videos", "audios"] {
        for material in timeline["materials"][bucket]
            .as_array()
            .into_iter()
            .flatten()
        {
            if let (Some(id), Some(path)) = (
                material["id"].as_str(),
                material["path"].as_str().filter(|path| !path.is_empty()),
            ) {
                paths.insert(id.to_owned(), path.to_owned());
            }
        }
    }

    let mut probed: std::collections::BTreeMap<String, Option<i64>> =
        std::collections::BTreeMap::new();
    let mut checked = 0_usize;
    let mut media_issues: Vec<Value> = Vec::new();
    for track in timeline["tracks"].as_array().into_iter().flatten() {
        for segment in track["segments"].as_array().into_iter().flatten() {
            let Some(material_id) = segment["material_id"].as_str() else {
                continue;
            };
            let Some(path) = paths.get(material_id) else {
                continue;
            };
            checked += 1;
            if !std::path::Path::new(path).is_file() {
                media_issues.push(json!({
                    "code": "media-missing",
                    "material_id": material_id,
                    "path": path
                }));
                continue;
            }
            let duration = probed
                .entry(path.clone())
                .or_insert_with(|| {
                    crate::probe::probe(std::path::Path::new(path))
                        .ok()
                        .map(|info| info.duration_us)
                })
                .to_owned();
            let Some(duration) = duration else { continue };
            let source_start = segment["source_timerange"]["start"].as_i64().unwrap_or(0);
            let source_length = segment["source_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            if duration > 0 && source_start + source_length > duration {
                media_issues.push(json!({
                    "code": "source-exceeds-media",
                    "material_id": material_id,
                    "path": path,
                    "requested_end_us": source_start + source_length,
                    "media_duration_us": duration
                }));
            }
        }
    }

    // 固定扣分：缺失素材 30/处，源窗口越界 15/处。
    let mut penalty = 0_i64;
    let mut penalties: Vec<Value> = Vec::new();
    for issue in &media_issues {
        let delta = match issue["code"].as_str() {
            Some("media-missing") => 30,
            Some("source-exceeds-media") => 15,
            _ => 0,
        };
        penalty += delta;
        penalties.push(json!({"code": issue["code"], "delta": delta}));
    }
    let base = report["score"].as_i64().unwrap_or(0);
    let score = (base - penalty).max(0);
    if let Some(list) = report["issues"].as_array_mut() {
        list.extend(media_issues.iter().cloned());
    }
    report["score"] = json!(score);
    report["deep"] = json!(true);
    report["media_checked"] = json!(checked);
    report["penalties"] = json!(penalties);
    report["score_before_media"] = json!(base);
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::{next_repair, permille};
    use serde_json::json;

    #[test]
    fn permille_handles_zero_and_rounds() {
        assert_eq!(permille(0, 0), 0);
        assert_eq!(permille(1, 3), 333);
        assert_eq!(permille(3, 3), 1000);
    }

    #[test]
    fn gaps_take_priority_over_empty_tracks() {
        let report = json!({"issues": [
            {"code":"empty-track","track":"effect"},
            {"code":"main-track-gap","start_us":0,"duration_us":10}
        ]});
        assert_eq!(next_repair(&report), Some("close-main-track-gaps"));
        let clean = json!({"issues": []});
        assert_eq!(next_repair(&clean), None);
    }
}

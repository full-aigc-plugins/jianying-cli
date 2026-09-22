use crate::{extend_mode::ExtendMode, shrink_mode::ShrinkMode};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

/// 按固定上游策略更新单个片段的素材区间与相邻片段位置。
pub fn apply_replacement_timing(
    segments: &mut [Value],
    segment_index: usize,
    source_start_us: i64,
    source_duration_us: i64,
    shrink_mode: ShrinkMode,
    extend_modes: &[ExtendMode],
) -> Result<Value> {
    if segment_index >= segments.len() {
        bail!(
            "segment index {segment_index} exceeds track length {}",
            segments.len()
        );
    }
    if source_start_us < 0 || source_duration_us <= 0 {
        bail!("replacement source range must be non-negative and non-empty");
    }
    let speed = segments[segment_index]["speed"].as_f64().unwrap_or(1.0);
    if (speed - 1.0).abs() > f64::EPSILON {
        bail!("replacement timing does not support speed-adjusted segments");
    }
    let original_start = segments[segment_index]["target_timerange"]["start"]
        .as_i64()
        .context("target start is missing")?;
    let original_duration = segments[segment_index]["target_timerange"]["duration"]
        .as_i64()
        .context("target duration is missing")?;
    if original_start < 0 || original_duration <= 0 {
        bail!("target range is invalid");
    }
    let delta = (source_duration_us - original_duration).abs();
    let mut effective_source_duration = source_duration_us;
    let mut applied_mode = "unchanged";

    if source_duration_us < original_duration {
        match shrink_mode {
            ShrinkMode::CutHead => {
                segments[segment_index]["target_timerange"]["start"] =
                    json!(original_start + delta);
            }
            ShrinkMode::CutTail => {
                segments[segment_index]["target_timerange"]["duration"] =
                    json!(original_duration - delta);
            }
            ShrinkMode::CutTailAlign => {
                segments[segment_index]["target_timerange"]["duration"] =
                    json!(original_duration - delta);
                for segment in segments.iter_mut().skip(segment_index + 1) {
                    let start = segment["target_timerange"]["start"]
                        .as_i64()
                        .context("following target start is missing")?;
                    segment["target_timerange"]["start"] = json!(start - delta);
                }
            }
            ShrinkMode::Shrink => {
                segments[segment_index]["target_timerange"]["duration"] =
                    json!(original_duration - delta);
                segments[segment_index]["target_timerange"]["start"] =
                    json!(original_start + delta / 2);
            }
        }
        applied_mode = shrink_mode.as_str();
    } else if source_duration_us > original_duration {
        let previous_end = if segment_index == 0 {
            0
        } else {
            range_end(&segments[segment_index - 1])?
        };
        let next_start = if segment_index + 1 == segments.len() {
            1_000_000_000_000_000_i64
        } else {
            segments[segment_index + 1]["target_timerange"]["start"]
                .as_i64()
                .context("following target start is missing")?
        };
        let mut applied = None;
        for mode in extend_modes {
            match mode {
                ExtendMode::ExtendHead if original_start - delta >= previous_end => {
                    segments[segment_index]["target_timerange"]["start"] =
                        json!(original_start - delta);
                    applied = Some(*mode);
                }
                ExtendMode::ExtendTail
                    if original_start + original_duration + delta <= next_start =>
                {
                    segments[segment_index]["target_timerange"]["duration"] =
                        json!(original_duration + delta);
                    applied = Some(*mode);
                }
                ExtendMode::PushTail => {
                    let shift = (original_start + original_duration + delta - next_start).max(0);
                    segments[segment_index]["target_timerange"]["duration"] =
                        json!(original_duration + delta);
                    if shift > 0 {
                        for segment in segments.iter_mut().skip(segment_index + 1) {
                            let start = segment["target_timerange"]["start"]
                                .as_i64()
                                .context("following target start is missing")?;
                            segment["target_timerange"]["start"] = json!(start + shift);
                        }
                    }
                    applied = Some(*mode);
                }
                ExtendMode::CutMaterialTail => {
                    effective_source_duration = original_duration;
                    applied = Some(*mode);
                }
                ExtendMode::ExtendHead | ExtendMode::ExtendTail => {}
            }
            if applied.is_some() {
                break;
            }
        }
        let Some(mode) = applied else {
            bail!("replacement extension failed for every declared strategy");
        };
        applied_mode = mode.as_str();
    }

    segments[segment_index]["source_timerange"] =
        json!({"start":source_start_us,"duration":effective_source_duration});
    Ok(json!({
        "mode":applied_mode,
        "segment_index":segment_index,
        "source_timerange":segments[segment_index]["source_timerange"],
        "target_timerange":segments[segment_index]["target_timerange"]
    }))
}

fn range_end(segment: &Value) -> Result<i64> {
    let start = segment["target_timerange"]["start"]
        .as_i64()
        .context("target start is missing")?;
    let duration = segment["target_timerange"]["duration"]
        .as_i64()
        .context("target duration is missing")?;
    start.checked_add(duration).context("target range overflow")
}

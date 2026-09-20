use crate::{DraftError, DraftTimelineWire};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// 字幕、样式、速度、音量、裁剪、变换和组合编辑 wire 语义校验器。
pub struct DraftEditSemantics;

impl DraftEditSemantics {
    /// 校验已迁移编辑字段的值域、内部一致性和素材引用关系。
    pub fn validate(timeline: &DraftTimelineWire) -> Result<(), DraftError> {
        let materials = timeline
            .materials
            .as_ref()
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        validate_crops(&materials)?;
        validate_texts(&materials)?;
        let speeds = speed_map(&materials)?;
        let mix_modes = validate_mix_modes(&materials)?;
        let mut referenced_mix_modes = BTreeSet::new();

        for (track_index, track) in timeline
            .tracks
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            for (segment_index, segment) in track
                .segments
                .as_deref()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                let context = format!("track {track_index} segment {segment_index}");
                if let Some(speed) = segment.speed {
                    require_range(&context, "speed", speed, 0.1, 8.0)?;
                }
                if let Some(volume) = segment.volume {
                    require_range(&context, "volume", volume, 0.0, 4.0)?;
                }
                validate_clip(&context, segment.clip.as_ref())?;
                validate_uniform_scale(&context, segment.uniform_scale.as_ref())?;
                for reference in segment.extra_material_refs.as_deref().unwrap_or_default() {
                    if let Some(companion_speed) = speeds.get(reference) {
                        let segment_speed = segment.speed.unwrap_or(1.0);
                        if (segment_speed - companion_speed).abs() > f64::EPSILON {
                            return invalid(
                                &context,
                                &format!(
                                    "segment speed {segment_speed} differs from speed resource {companion_speed}"
                                ),
                            );
                        }
                    }
                    if mix_modes.contains(reference) {
                        referenced_mix_modes.insert(reference.clone());
                    }
                }
            }
        }
        if let Some(orphan) = mix_modes.difference(&referenced_mix_modes).next() {
            return invalid(
                "materials.effects",
                &format!("mix mode {orphan} is not referenced"),
            );
        }
        Ok(())
    }
}

fn validate_crops(materials: &Map<String, Value>) -> Result<(), DraftError> {
    for (index, video) in bucket(materials, "videos").iter().enumerate() {
        let Some(crop) = video.get("crop").and_then(Value::as_object) else {
            continue;
        };
        for field in [
            "upper_left_x",
            "upper_left_y",
            "upper_right_x",
            "upper_right_y",
            "lower_left_x",
            "lower_left_y",
            "lower_right_x",
            "lower_right_y",
        ] {
            let value = crop.get(field).and_then(Value::as_f64).ok_or_else(|| {
                DraftError::InvalidEditSemantic {
                    context: format!("materials.videos[{index}].crop"),
                    reason: format!("{field} must be numeric"),
                }
            })?;
            require_range(
                &format!("materials.videos[{index}].crop"),
                field,
                value,
                0.0,
                1.0,
            )?;
        }
    }
    Ok(())
}

fn validate_texts(materials: &Map<String, Value>) -> Result<(), DraftError> {
    for (index, text_material) in bucket(materials, "texts").iter().enumerate() {
        let context = format!("materials.texts[{index}]");
        let content = text_material
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| DraftError::InvalidEditSemantic {
                context: context.clone(),
                reason: "content must be a JSON string".to_owned(),
            })?;
        let content: Value =
            serde_json::from_str(content).map_err(|error| DraftError::InvalidEditSemantic {
                context: context.clone(),
                reason: format!("content is not valid JSON: {error}"),
            })?;
        let text = content.get("text").and_then(Value::as_str).ok_or_else(|| {
            DraftError::InvalidEditSemantic {
                context: context.clone(),
                reason: "content.text must be a string".to_owned(),
            }
        })?;
        let utf16_len = text.encode_utf16().count() as u64;
        let styles = content
            .get("styles")
            .and_then(Value::as_array)
            .filter(|styles| !styles.is_empty())
            .ok_or_else(|| DraftError::InvalidEditSemantic {
                context: context.clone(),
                reason: "content.styles must be a non-empty array".to_owned(),
            })?;
        for (style_index, style) in styles.iter().enumerate() {
            let range = style
                .get("range")
                .and_then(Value::as_array)
                .ok_or_else(|| DraftError::InvalidEditSemantic {
                    context: context.clone(),
                    reason: format!("style {style_index} range must be an array"),
                })?;
            let start = range.first().and_then(Value::as_u64).unwrap_or(u64::MAX);
            let end = range.get(1).and_then(Value::as_u64).unwrap_or(u64::MAX);
            if range.len() != 2 || start > end || end > utf16_len {
                return invalid(
                    &context,
                    &format!("style {style_index} range must fit UTF-16 length {utf16_len}"),
                );
            }
            if let Some(size) = style.get("size").and_then(Value::as_f64) {
                require_range(&context, "style size", size, 0.01, 1000.0)?;
            }
        }
        if let Some(alignment) = text_material.get("alignment").and_then(Value::as_u64) {
            if alignment > 2 {
                return invalid(&context, "alignment must be 0, 1 or 2");
            }
        }
    }
    Ok(())
}

fn speed_map(materials: &Map<String, Value>) -> Result<BTreeMap<String, f64>, DraftError> {
    let mut speeds = BTreeMap::new();
    for (index, speed) in bucket(materials, "speeds").iter().enumerate() {
        let Some(id) = speed.get("id").and_then(Value::as_str) else {
            continue;
        };
        let value = speed.get("speed").and_then(Value::as_f64).ok_or_else(|| {
            DraftError::InvalidEditSemantic {
                context: format!("materials.speeds[{index}]"),
                reason: "speed must be numeric".to_owned(),
            }
        })?;
        require_range("materials.speeds", "speed", value, 0.1, 8.0)?;
        speeds.insert(id.to_owned(), value);
    }
    Ok(speeds)
}

fn validate_mix_modes(materials: &Map<String, Value>) -> Result<BTreeSet<String>, DraftError> {
    let mut ids = BTreeSet::new();
    for (index, effect) in bucket(materials, "effects").iter().enumerate() {
        if effect.get("type").and_then(Value::as_str) != Some("mix_mode") {
            continue;
        }
        let context = format!("materials.effects[{index}]");
        let id = effect.get("id").and_then(Value::as_str).unwrap_or_default();
        if id.is_empty() {
            return invalid(&context, "mix mode id must not be empty");
        }
        let value = effect.get("value").and_then(Value::as_f64).unwrap_or(1.0);
        require_range(&context, "mix mode value", value, 0.0, 1.0)?;
        ids.insert(id.to_owned());
    }
    Ok(ids)
}

fn validate_clip(context: &str, clip: Option<&Value>) -> Result<(), DraftError> {
    let Some(clip) = clip else { return Ok(()) };
    let clip = clip
        .as_object()
        .ok_or_else(|| DraftError::InvalidEditSemantic {
            context: context.to_owned(),
            reason: "clip must be an object".to_owned(),
        })?;
    if let Some(alpha) = clip.get("alpha").and_then(Value::as_f64) {
        require_range(context, "clip alpha", alpha, 0.0, 1.0)?;
    }
    if let Some(rotation) = clip.get("rotation").and_then(Value::as_f64) {
        require_range(context, "clip rotation", rotation, -360.0, 360.0)?;
    }
    validate_pair(context, clip.get("scale"), "scale", 0.01, 10.0)?;
    validate_pair(context, clip.get("transform"), "transform", -2.0, 2.0)
}

fn validate_uniform_scale(context: &str, value: Option<&Value>) -> Result<(), DraftError> {
    let Some(value) = value else { return Ok(()) };
    let object = value
        .as_object()
        .ok_or_else(|| DraftError::InvalidEditSemantic {
            context: context.to_owned(),
            reason: "uniform_scale must be an object".to_owned(),
        })?;
    if !object.get("on").is_some_and(Value::is_boolean) {
        return invalid(context, "uniform_scale.on must be boolean");
    }
    let scale = object.get("value").and_then(Value::as_f64).unwrap_or(1.0);
    require_range(context, "uniform_scale.value", scale, 0.01, 10.0)
}

fn validate_pair(
    context: &str,
    value: Option<&Value>,
    name: &str,
    min: f64,
    max: f64,
) -> Result<(), DraftError> {
    let Some(value) = value else { return Ok(()) };
    let object = value
        .as_object()
        .ok_or_else(|| DraftError::InvalidEditSemantic {
            context: context.to_owned(),
            reason: format!("clip {name} must be an object"),
        })?;
    for axis in ["x", "y"] {
        let coordinate = object.get(axis).and_then(Value::as_f64).ok_or_else(|| {
            DraftError::InvalidEditSemantic {
                context: context.to_owned(),
                reason: format!("clip {name}.{axis} must be numeric"),
            }
        })?;
        require_range(
            context,
            &format!("clip {name}.{axis}"),
            coordinate,
            min,
            max,
        )?;
    }
    Ok(())
}

fn bucket<'a>(materials: &'a Map<String, Value>, name: &str) -> &'a Vec<Value> {
    static EMPTY: std::sync::OnceLock<Vec<Value>> = std::sync::OnceLock::new();
    materials
        .get(name)
        .and_then(Value::as_array)
        .unwrap_or_else(|| EMPTY.get_or_init(Vec::new))
}

fn require_range(
    context: &str,
    field: &str,
    value: f64,
    min: f64,
    max: f64,
) -> Result<(), DraftError> {
    if value.is_finite() && (min..=max).contains(&value) {
        Ok(())
    } else {
        invalid(context, &format!("{field} must be within {min}..={max}"))
    }
}

fn invalid<T>(context: &str, reason: &str) -> Result<T, DraftError> {
    Err(DraftError::InvalidEditSemantic {
        context: context.to_owned(),
        reason: reason.to_owned(),
    })
}

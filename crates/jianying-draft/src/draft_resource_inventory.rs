use crate::{
    DraftError, DraftKeyframeWire, DraftResourceKind, DraftResourceWire, DraftTimelineWire,
};
use serde_json::{Map, Value};

/// 草稿内已迁移资源的统一、只读 inventory。
#[derive(Debug, Clone, PartialEq)]
pub struct DraftResourceInventory {
    resources: Vec<DraftResourceWire>,
}

impl DraftResourceInventory {
    pub(crate) fn from_timeline(timeline: &DraftTimelineWire) -> Result<Self, DraftError> {
        let mut resources = Vec::new();
        if let Some(materials) = &timeline.materials {
            let materials = materials
                .as_object()
                .ok_or(DraftError::MaterialsMustBeObject)?;
            for (bucket, base_kind) in [
                ("videos", DraftResourceKind::Video),
                ("audios", DraftResourceKind::Audio),
                ("texts", DraftResourceKind::Text),
                ("stickers", DraftResourceKind::Sticker),
                ("effects", DraftResourceKind::Effect),
                ("video_effects", DraftResourceKind::Effect),
                ("filters", DraftResourceKind::Filter),
                ("transitions", DraftResourceKind::Transition),
                ("masks", DraftResourceKind::Mask),
                ("material_animations", DraftResourceKind::Animation),
            ] {
                let Some(entries) = materials.get(bucket) else {
                    continue;
                };
                let entries = entries
                    .as_array()
                    .ok_or_else(|| DraftError::ResourceBucketMustBeArray(bucket.to_owned()))?;
                for entry in entries {
                    let object = entry
                        .as_object()
                        .ok_or_else(|| DraftError::ResourceMustBeObject(bucket.to_owned()))?;
                    let id = object
                        .get("id")
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                        .ok_or_else(|| DraftError::ResourceMissingId(bucket.to_owned()))?;
                    let wire_type = object
                        .get("type")
                        .and_then(Value::as_str)
                        .map(ToOwned::to_owned);
                    let kind = if matches!(bucket, "effects" | "video_effects")
                        && wire_type.as_deref() == Some("filter")
                    {
                        DraftResourceKind::Filter
                    } else {
                        base_kind
                    };
                    validate_resource_semantics(bucket, kind, object)?;
                    resources.push(DraftResourceWire {
                        id: id.to_owned(),
                        kind,
                        bucket: bucket.to_owned(),
                        wire_type,
                        raw: entry.clone(),
                    });
                }
            }
        }

        for track in timeline.tracks.as_deref().unwrap_or_default() {
            for segment in track.segments.as_deref().unwrap_or_default() {
                for keyframe in segment.common_keyframes.as_deref().unwrap_or_default() {
                    keyframe.validate()?;
                    resources.push(Self::keyframe_resource(keyframe)?);
                }
            }
        }
        Ok(Self { resources })
    }

    fn keyframe_resource(keyframe: &DraftKeyframeWire) -> Result<DraftResourceWire, DraftError> {
        Ok(DraftResourceWire {
            id: keyframe.id.clone(),
            kind: DraftResourceKind::Keyframe,
            bucket: "common_keyframes".to_owned(),
            wire_type: Some(keyframe.property_type.clone()),
            raw: serde_json::to_value(keyframe)?,
        })
    }

    /// 返回 inventory 中的资源总数。
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// 返回 inventory 是否为空。
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }

    /// 返回所有已迁移资源视图。
    pub fn resources(&self) -> &[DraftResourceWire] {
        &self.resources
    }

    /// 返回指定统一资源类别的数量。
    pub fn count(&self, kind: DraftResourceKind) -> usize {
        self.resources
            .iter()
            .filter(|resource| resource.kind == kind)
            .count()
    }
}

fn validate_resource_semantics(
    bucket: &str,
    kind: DraftResourceKind,
    object: &Map<String, Value>,
) -> Result<(), DraftError> {
    let wire_type = required_string(bucket, object, "type")?;
    match kind {
        DraftResourceKind::Video => {
            if !matches!(wire_type, "video" | "photo") {
                return invalid_semantic(bucket, "type must be video or photo");
            }
            required_number(bucket, object, "duration")?;
        }
        DraftResourceKind::Audio => {
            required_number(bucket, object, "duration")?;
        }
        DraftResourceKind::Text => {
            if !matches!(wire_type, "text" | "subtitle") {
                return invalid_semantic(bucket, "type must be text or subtitle");
            }
            required_string(bucket, object, "content")?;
        }
        DraftResourceKind::Sticker => {
            if wire_type != "sticker" {
                return invalid_semantic(bucket, "type must be sticker");
            }
            required_string(bucket, object, "resource_id")?;
        }
        DraftResourceKind::Filter => {
            if wire_type != "filter" {
                return invalid_semantic(bucket, "filter resource must have type=filter");
            }
            required_string(bucket, object, "effect_id")?;
            required_string(bucket, object, "resource_id")?;
        }
        DraftResourceKind::Effect => {
            required_string(bucket, object, "effect_id")?;
            required_string(bucket, object, "resource_id")?;
        }
        DraftResourceKind::Transition => {
            if wire_type != "transition" {
                return invalid_semantic(bucket, "type must be transition");
            }
            required_number(bucket, object, "duration")?;
            required_string(bucket, object, "effect_id")?;
            required_string(bucket, object, "resource_id")?;
        }
        DraftResourceKind::Mask => {
            if wire_type != "mask" {
                return invalid_semantic(bucket, "type must be mask");
            }
            required_string(bucket, object, "resource_id")?;
            if !object.get("config").is_some_and(Value::is_object) {
                return invalid_semantic(bucket, "config must be an object");
            }
        }
        DraftResourceKind::Animation => {
            if wire_type != "sticker_animation" {
                return invalid_semantic(bucket, "type must be sticker_animation");
            }
            let animations = object
                .get("animations")
                .and_then(Value::as_array)
                .filter(|animations| !animations.is_empty())
                .ok_or_else(|| DraftError::InvalidResourceSemantic {
                    bucket: bucket.to_owned(),
                    reason: "animations must be a non-empty array".to_owned(),
                })?;
            for animation in animations {
                let item =
                    animation
                        .as_object()
                        .ok_or_else(|| DraftError::InvalidResourceSemantic {
                            bucket: bucket.to_owned(),
                            reason: "animation item must be an object".to_owned(),
                        })?;
                required_string(bucket, item, "type")?;
                required_string(bucket, item, "resource_id")?;
                required_number(bucket, item, "start")?;
                required_number(bucket, item, "duration")?;
            }
        }
        DraftResourceKind::Keyframe => unreachable!("keyframes use their typed validator"),
    }
    Ok(())
}

fn required_string<'a>(
    bucket: &str,
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<&'a str, DraftError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| DraftError::InvalidResourceSemantic {
            bucket: bucket.to_owned(),
            reason: format!("{field} must be a non-empty string"),
        })
}

fn required_number(
    bucket: &str,
    object: &Map<String, Value>,
    field: &str,
) -> Result<(), DraftError> {
    if object.get(field).is_some_and(Value::is_number) {
        Ok(())
    } else {
        invalid_semantic(bucket, &format!("{field} must be a number"))
    }
}

fn invalid_semantic<T>(bucket: &str, reason: &str) -> Result<T, DraftError> {
    Err(DraftError::InvalidResourceSemantic {
        bucket: bucket.to_owned(),
        reason: reason.to_owned(),
    })
}

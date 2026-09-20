use crate::{DraftError, DraftTimelineWire};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// 草稿素材标识和片段引用闭包校验器。
pub struct DraftReferenceIntegrity;

impl DraftReferenceIntegrity {
    /// 校验素材 ID 唯一性、轨道/片段 ID 唯一性和所有片段素材引用。
    pub fn validate(timeline: &DraftTimelineWire) -> Result<(), DraftError> {
        let mut material_buckets = BTreeMap::<String, String>::new();
        if let Some(materials) = &timeline.materials {
            let materials = materials
                .as_object()
                .ok_or(DraftError::MaterialsMustBeObject)?;
            for (bucket, entries) in materials {
                let entries = entries
                    .as_array()
                    .ok_or_else(|| DraftError::ResourceBucketMustBeArray(bucket.clone()))?;
                for entry in entries {
                    let Some(id) = entry
                        .as_object()
                        .and_then(|object| object.get("id"))
                        .and_then(Value::as_str)
                        .filter(|id| !id.trim().is_empty())
                    else {
                        continue;
                    };
                    if let Some(first_bucket) =
                        material_buckets.insert(id.to_owned(), bucket.clone())
                    {
                        return Err(DraftError::DuplicateMaterialId {
                            id: id.to_owned(),
                            first_bucket,
                            second_bucket: bucket.clone(),
                        });
                    }
                }
            }
        }

        let mut track_ids = BTreeSet::new();
        let mut segment_ids = BTreeSet::new();
        for (track_index, track) in timeline
            .tracks
            .as_deref()
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            insert_unique(&mut track_ids, track.id.as_deref(), "track")?;
            for (segment_index, segment) in track
                .segments
                .as_deref()
                .unwrap_or_default()
                .iter()
                .enumerate()
            {
                insert_unique(&mut segment_ids, segment.id.as_deref(), "segment")?;
                if let Some(bucket) = primary_bucket(track.track_type.as_deref()) {
                    let material_id = segment
                        .material_id
                        .as_deref()
                        .filter(|id| !id.trim().is_empty())
                        .unwrap_or_default();
                    let actual_bucket = material_buckets.get(material_id).map(String::as_str);
                    let compatible = actual_bucket == Some(bucket)
                        || (track.track_type.as_deref() == Some("audio")
                            && actual_bucket == Some("audio_effects"))
                        || (track.track_type.as_deref() == Some("filter")
                            && actual_bucket == Some("video_effects"));
                    if !compatible {
                        return Err(DraftError::MissingPrimaryMaterial {
                            track_index,
                            segment_index,
                            material_id: material_id.to_owned(),
                            bucket: bucket.to_owned(),
                        });
                    }
                }
                for material_id in segment.extra_material_refs.as_deref().unwrap_or_default() {
                    if material_id.trim().is_empty() || !material_buckets.contains_key(material_id)
                    {
                        return Err(DraftError::DanglingMaterialReference {
                            track_index,
                            segment_index,
                            material_id: material_id.clone(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}

fn insert_unique(
    ids: &mut BTreeSet<String>,
    id: Option<&str>,
    kind: &str,
) -> Result<(), DraftError> {
    if let Some(id) = id.filter(|id| !id.trim().is_empty()) {
        if !ids.insert(id.to_owned()) {
            return Err(DraftError::DuplicateObjectId {
                kind: kind.to_owned(),
                id: id.to_owned(),
            });
        }
    }
    Ok(())
}

fn primary_bucket(track_type: Option<&str>) -> Option<&'static str> {
    match track_type {
        Some("video") => Some("videos"),
        Some("audio") => Some("audios"),
        Some("text") => Some("texts"),
        Some("sticker") => Some("stickers"),
        Some("filter") => Some("effects"),
        Some("effect") => Some("video_effects"),
        _ => None,
    }
}

use crate::{DraftError, DraftMetadataWire, DraftTimelineWire};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// 草稿时间线镜像、元数据路径和本地素材注册关系校验器。
pub struct DraftBundleIntegrity;

impl DraftBundleIntegrity {
    /// 校验两个时间线镜像、元数据字段、草稿目录关系和本地素材注册表。
    pub fn validate(
        content: &DraftTimelineWire,
        info: &DraftTimelineWire,
        metadata: &DraftMetadataWire,
        draft_dir: Option<&Path>,
    ) -> Result<(), DraftError> {
        Self::validate_with_roots(content, info, metadata, draft_dir, draft_dir)
    }

    /// 分别按最终草稿身份路径和事务工作副本路径校验。
    pub fn validate_with_roots(
        content: &DraftTimelineWire,
        info: &DraftTimelineWire,
        metadata: &DraftMetadataWire,
        identity_dir: Option<&Path>,
        resource_dir: Option<&Path>,
    ) -> Result<(), DraftError> {
        content.validate_references()?;
        content.validate_edit_semantics()?;
        info.validate_references()?;
        info.validate_edit_semantics()?;
        if content.to_value()? != info.to_value()? {
            return Err(DraftError::TimelineMirrorMismatch);
        }
        compare_optional(
            "draft_name",
            content.name.as_deref(),
            metadata.draft_name.as_deref(),
        )?;
        compare_optional_i64("tm_duration", content.duration, metadata.tm_duration)?;
        if let Some(draft_dir) = identity_dir {
            validate_path(
                "draft_fold_path",
                draft_dir,
                metadata.draft_fold_path.as_deref(),
            )?;
            if let Some(parent) = draft_dir.parent() {
                validate_path(
                    "draft_root_path",
                    parent,
                    metadata.draft_root_path.as_deref(),
                )?;
            }
            validate_path(
                "draft_json_file",
                &draft_dir.join("draft_content.json"),
                metadata.draft_json_file.as_deref(),
            )?;
        }
        validate_registrations(content, metadata, identity_dir, resource_dir)
    }
}

fn compare_optional(
    field: &str,
    expected: Option<&str>,
    actual: Option<&str>,
) -> Result<(), DraftError> {
    if let (Some(expected), Some(actual)) = (expected, actual) {
        if expected != actual {
            return Err(DraftError::MetadataMismatch {
                field: field.to_owned(),
                expected: expected.to_owned(),
                actual: actual.to_owned(),
            });
        }
    }
    Ok(())
}

fn compare_optional_i64(
    field: &str,
    expected: Option<i64>,
    actual: Option<i64>,
) -> Result<(), DraftError> {
    if let (Some(expected), Some(actual)) = (expected, actual) {
        if expected != actual {
            return Err(DraftError::MetadataMismatch {
                field: field.to_owned(),
                expected: expected.to_string(),
                actual: actual.to_string(),
            });
        }
    }
    Ok(())
}

fn validate_path(field: &str, expected: &Path, actual: Option<&str>) -> Result<(), DraftError> {
    let actual = actual.unwrap_or_default();
    let expected_normalized = expected
        .canonicalize()
        .unwrap_or_else(|_| expected.to_path_buf());
    let actual_path = Path::new(actual);
    let actual_normalized = actual_path
        .canonicalize()
        .unwrap_or_else(|_| actual_path.to_path_buf());
    if actual_normalized != expected_normalized {
        return Err(DraftError::MetadataPathMismatch {
            field: field.to_owned(),
            expected: expected_normalized.to_string_lossy().into_owned(),
            actual: actual.to_owned(),
        });
    }
    Ok(())
}

fn validate_registrations(
    timeline: &DraftTimelineWire,
    metadata: &DraftMetadataWire,
    identity_dir: Option<&Path>,
    resource_dir: Option<&Path>,
) -> Result<(), DraftError> {
    let registrations = registration_map(metadata);
    let Some(materials) = timeline.materials.as_ref().and_then(Value::as_object) else {
        return Ok(());
    };
    for (bucket, expected_kind) in [("videos", "video"), ("audios", "music")] {
        for material in materials
            .get(bucket)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(object) = material.as_object() else {
                continue;
            };
            let Some(path) = object
                .get("path")
                .or_else(|| object.get("media_path"))
                .and_then(Value::as_str)
                .filter(|path| !path.trim().is_empty())
            else {
                continue;
            };
            let Some(actual_kind) = registrations.get(path) else {
                return Err(DraftError::MaterialNotRegistered(path.to_owned()));
            };
            validate_local_material_path(path, identity_dir, resource_dir)?;
            let compatible = if bucket == "videos" {
                let wire_type = object
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or(expected_kind);
                actual_kind == wire_type || (wire_type == "video" && actual_kind == expected_kind)
            } else {
                matches!(actual_kind.as_str(), "music" | "audio")
            };
            if !compatible {
                return Err(DraftError::RegisteredMaterialKindMismatch {
                    path: path.to_owned(),
                    expected: expected_kind.to_owned(),
                    actual: actual_kind.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_local_material_path(
    path: &str,
    identity_dir: Option<&Path>,
    resource_dir: Option<&Path>,
) -> Result<(), DraftError> {
    let Some(resource_dir) = resource_dir.filter(|draft_dir| draft_dir.exists()) else {
        return Ok(());
    };
    let material_path = Path::new(path);
    let material_path = if material_path.is_absolute() {
        if let Some(identity_dir) = identity_dir {
            material_path
                .strip_prefix(identity_dir)
                .map(|relative| resource_dir.join(relative))
                .unwrap_or_else(|_| material_path.to_path_buf())
        } else {
            material_path.to_path_buf()
        }
    } else {
        resource_dir.join(material_path)
    };
    if !material_path.is_file() {
        return Err(DraftError::MaterialFileMissing(
            material_path.to_string_lossy().into_owned(),
        ));
    }
    let draft_dir = resource_dir
        .canonicalize()
        .unwrap_or_else(|_| resource_dir.to_path_buf());
    let material_path = material_path
        .canonicalize()
        .unwrap_or_else(|_| material_path.to_path_buf());
    if !material_path.starts_with(&draft_dir) {
        return Err(DraftError::MaterialPathOutsideDraft(
            material_path.to_string_lossy().into_owned(),
        ));
    }
    Ok(())
}

fn registration_map(metadata: &DraftMetadataWire) -> BTreeMap<String, String> {
    let mut registrations = BTreeMap::new();
    for group in metadata
        .draft_materials
        .as_ref()
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        for entry in group
            .get("value")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let (Some(path), Some(kind)) = (
                entry.get("file_Path").and_then(Value::as_str),
                entry.get("metetype").and_then(Value::as_str),
            ) {
                registrations.insert(path.to_owned(), kind.to_owned());
            }
        }
    }
    registrations
}

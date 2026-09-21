use crate::{RuntimeError, RuntimeFileIdentity};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;

/// 官方资源的内容身份：独立下载文件或草稿内嵌资源节点。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum OfficialResourceIdentity {
    DownloadedFile {
        file: RuntimeFileIdentity,
    },
    DraftResource {
        resource_id: String,
        resource_sha256: String,
        observed_draft_sha256: String,
    },
}

impl OfficialResourceIdentity {
    /// 从一个已下载常规文件创建身份。
    pub fn downloaded_file(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        Ok(Self::DownloadedFile {
            file: RuntimeFileIdentity::from_path(path)?,
        })
    }

    /// 从草稿中的唯一资源节点创建规范化身份。
    pub fn draft_resource(
        draft: impl AsRef<Path>,
        resource_id: impl Into<String>,
    ) -> Result<Self, RuntimeError> {
        let resource_id = resource_id.into();
        let (resource_sha256, observed_draft_sha256) =
            inspect_draft_resource(&draft, &resource_id)?;
        Ok(Self::DraftResource {
            resource_id,
            resource_sha256,
            observed_draft_sha256,
        })
    }

    /// 校验独立下载文件身份。
    pub fn verify_file(&self, path: impl AsRef<Path>) -> Result<(), RuntimeError> {
        let path = path.as_ref();
        let Self::DownloadedFile { file } = self else {
            return Err(RuntimeError::ResourceIdentityKindMismatch);
        };
        let observed = RuntimeFileIdentity::from_path(path)?;
        if observed.sha256() != file.sha256() || observed.byte_length() != file.byte_length() {
            return Err(RuntimeError::AssetIdentityMismatch(path.to_path_buf()));
        }
        Ok(())
    }

    /// 校验草稿中资源节点的稳定 ID 与规范化内容摘要。
    pub fn verify_draft(&self, draft: impl AsRef<Path>) -> Result<(), RuntimeError> {
        let Self::DraftResource {
            resource_id,
            resource_sha256,
            ..
        } = self
        else {
            return Err(RuntimeError::ResourceIdentityKindMismatch);
        };
        let (observed_resource_sha256, _) = inspect_draft_resource(&draft, resource_id)?;
        if observed_resource_sha256 != *resource_sha256 {
            return Err(RuntimeError::AssetIdentityMismatch(
                draft.as_ref().to_path_buf(),
            ));
        }
        Ok(())
    }
}

fn inspect_draft_resource(
    draft: impl AsRef<Path>,
    resource_id: &str,
) -> Result<(String, String), RuntimeError> {
    if resource_id.trim().is_empty() {
        return Err(RuntimeError::DraftResourceUnavailable(
            "resource id must not be empty".to_owned(),
        ));
    }
    let content_path = draft.as_ref().join("draft_content.json");
    let bytes = fs::read(&content_path).map_err(|error| RuntimeError::Io {
        path: content_path.clone(),
        message: error.to_string(),
    })?;
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| RuntimeError::Io {
        path: content_path.clone(),
        message: error.to_string(),
    })?;
    let mut matches = Vec::new();
    find_resource_nodes(&document, resource_id, &mut matches);
    if matches.len() != 1 {
        return Err(RuntimeError::DraftResourceUnavailable(format!(
            "resource {resource_id} matched {} nodes",
            matches.len()
        )));
    }
    let normalized = serde_json::to_vec(matches[0])
        .map_err(|error| RuntimeError::DraftResourceUnavailable(error.to_string()))?;
    Ok((
        format!("{:x}", Sha256::digest(normalized)),
        format!("{:x}", Sha256::digest(bytes)),
    ))
}

fn find_resource_nodes<'a>(value: &'a Value, resource_id: &str, matches: &mut Vec<&'a Value>) {
    match value {
        Value::Object(object) => {
            if object.get("id").and_then(Value::as_str) == Some(resource_id) {
                matches.push(value);
            }
            for child in object.values() {
                find_resource_nodes(child, resource_id, matches);
            }
        }
        Value::Array(values) => {
            for child in values {
                find_resource_nodes(child, resource_id, matches);
            }
        }
        _ => {}
    }
}

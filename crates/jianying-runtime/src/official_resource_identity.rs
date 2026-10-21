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
            observed_draft_sha256,
        } = self
        else {
            return Err(RuntimeError::ResourceIdentityKindMismatch);
        };
        let (observed_resource_sha256, _) = match inspect_draft_resource(&draft, resource_id) {
            Ok(pair) => pair,
            Err(RuntimeError::DraftResourceUnavailable(detail)) => {
                // 节点消失最常见的原因是编辑器保存后重新生成了 id；此时草稿摘要也已改变，
                // 报“草稿被改写过”比报“资源不可用”更贴近事实。
                if let Ok(Some(observed)) = read_draft_content_sha256(&draft) {
                    if observed != *observed_draft_sha256 {
                        return Err(RuntimeError::DraftContentChanged {
                            recorded: observed_draft_sha256.clone(),
                            observed,
                        });
                    }
                }
                return Err(RuntimeError::DraftResourceUnavailable(detail));
            }
            Err(other) => return Err(other),
        };
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
    let document: Value = serde_json::from_slice(&bytes).map_err(|error| {
        if looks_like_encrypted_draft_content(&bytes) {
            RuntimeError::DraftContentEncrypted(content_path.clone())
        } else {
            RuntimeError::Io {
                path: content_path.clone(),
                message: error.to_string(),
            }
        }
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

/// 判断 `draft_content.json` 是否是剪映自己加密后的信封（Base64 文本，解码后不是 JSON）。
///
/// 剪映专业版会加密它自己创建的草稿；CLI 创建的草稿保持明文。这个判断只用于给出可行动的
/// 错误，而不是假装读懂了加密内容。
fn looks_like_encrypted_draft_content(bytes: &[u8]) -> bool {
    let trimmed = bytes
        .iter()
        .copied()
        .skip_while(|byte| byte.is_ascii_whitespace())
        .collect::<Vec<u8>>();
    if trimmed.is_empty() {
        return false;
    }
    // 明文 JSON 一定以 { 或 [ 开头，走到这里说明解析已失败；只剩“像 Base64 的信封”这一种可能。
    let all_envelope_chars = trimmed.iter().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'/' | b'=' | b'\r' | b'\n')
    });
    all_envelope_chars && trimmed.len() >= 16
}

/// 读取草稿 `draft_content.json` 的整体摘要；读不到时返回 `None`，由调用方保留原错误。
fn read_draft_content_sha256(draft: impl AsRef<Path>) -> Result<Option<String>, RuntimeError> {
    let path = draft.as_ref().join("draft_content.json");
    match fs::read(&path) {
        Ok(bytes) => Ok(Some(format!("{:x}", Sha256::digest(&bytes)))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(RuntimeError::Io {
            path,
            message: error.to_string(),
        }),
    }
}

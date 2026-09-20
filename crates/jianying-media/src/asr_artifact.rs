use crate::{AsrError, AsrOutputFormat};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 与源内容哈希绑定、可供账本验证的 ASR 产物。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrArtifact {
    path: PathBuf,
    output_format: AsrOutputFormat,
    provider_id: String,
    source_sha256: String,
    byte_length: u64,
    request_id: Option<String>,
}

impl AsrArtifact {
    /// 从已落盘的非空转录文件创建产物记录。
    pub fn from_path(
        path: impl AsRef<Path>,
        output_format: AsrOutputFormat,
        provider_id: impl Into<String>,
        source_sha256: impl Into<String>,
    ) -> Result<Self, AsrError> {
        let path = path.as_ref();
        let byte_length = std::fs::metadata(path)
            .map_err(|_| AsrError::EmptyArtifact(path.to_path_buf()))?
            .len();
        if byte_length == 0 {
            return Err(AsrError::EmptyArtifact(path.to_path_buf()));
        }
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(AsrError::EmptyProviderId);
        }
        let source_sha256 = source_sha256.into();
        if source_sha256.len() != 64 || !source_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(AsrError::InvalidSourceDigest);
        }
        Ok(Self {
            path: path.to_path_buf(),
            output_format,
            provider_id,
            source_sha256: source_sha256.to_ascii_lowercase(),
            byte_length,
            request_id: None,
        })
    }

    /// 记录外部执行器请求 ID。
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// 返回转录文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回输出格式。
    pub fn output_format(&self) -> AsrOutputFormat {
        self.output_format
    }

    /// 返回实际 Provider 标识。
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// 返回源内容 SHA-256。
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// 返回产物字节数。
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

use crate::{TtsAudioFormat, TtsError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TTS Provider 生成并可纳入草稿事务的音频产物。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TtsArtifact {
    path: PathBuf,
    format: TtsAudioFormat,
    provider_id: String,
    byte_length: u64,
    request_id: Option<String>,
}

impl TtsArtifact {
    /// 创建产物记录，校验路径、Provider 和非空字节数。
    pub fn new(
        path: PathBuf,
        format: TtsAudioFormat,
        provider_id: impl Into<String>,
        byte_length: u64,
    ) -> Result<Self, TtsError> {
        if path.as_os_str().is_empty() {
            return Err(TtsError::EmptyArtifactPath);
        }
        if byte_length == 0 {
            return Err(TtsError::EmptyArtifact);
        }
        let provider_id = provider_id.into();
        if provider_id.trim().is_empty() {
            return Err(TtsError::EmptyProviderId);
        }
        Ok(Self {
            path,
            format,
            provider_id,
            byte_length,
            request_id: None,
        })
    }

    /// 记录云端或本地服务返回的请求 ID，供审计和状态查询使用。
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// 返回产物路径。
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// 返回音频格式。
    pub fn format(&self) -> TtsAudioFormat {
        self.format
    }

    /// 返回实际 Provider 标识。
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// 返回产物字节数。
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// 返回 Provider 请求 ID，用于远端状态查询和审计。
    pub fn request_id(&self) -> Option<&str> {
        self.request_id.as_deref()
    }
}

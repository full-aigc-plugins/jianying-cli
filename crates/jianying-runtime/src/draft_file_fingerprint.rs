use serde::{Deserialize, Serialize};

/// 草稿树单个文件的全内容指纹。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftFileFingerprint {
    sha256: String,
    byte_length: u64,
}

impl DraftFileFingerprint {
    pub(crate) fn new(sha256: String, byte_length: u64) -> Self {
        Self {
            sha256,
            byte_length,
        }
    }

    /// 返回内容 SHA-256。
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// 返回字节数。
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

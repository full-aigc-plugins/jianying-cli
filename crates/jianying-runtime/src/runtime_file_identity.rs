use crate::RuntimeError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// 可执行文件的稳定内容身份，用于阻止路径相同但内容已变化的运行时。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFileIdentity {
    path: PathBuf,
    sha256: String,
    byte_length: u64,
}

impl RuntimeFileIdentity {
    /// 从文件内容计算 SHA-256 身份。
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(RuntimeError::FileMissing(path.to_path_buf()));
        }
        let bytes = fs::read(path).map_err(|error| RuntimeError::Io {
            path: path.to_path_buf(),
            message: error.to_string(),
        })?;
        Ok(Self {
            path: path.to_path_buf(),
            sha256: format!("{:x}", Sha256::digest(&bytes)),
            byte_length: bytes.len() as u64,
        })
    }

    /// 返回身份对应的文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回十六进制 SHA-256。
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// 返回文件字节数。
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }
}

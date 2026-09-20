use crate::{DraftFileFingerprint, RuntimeError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// 草稿目录全部常规文件的相对路径、长度与内容哈希快照。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DraftTreeSnapshot {
    files: BTreeMap<PathBuf, DraftFileFingerprint>,
}

impl DraftTreeSnapshot {
    /// 扫描目录并拒绝符号链接，生成确定性全文件快照。
    pub fn capture(root: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(RuntimeError::NotDirectory(root.to_path_buf()));
        }
        let mut files = BTreeMap::new();
        capture_directory(root, root, &mut files)?;
        Ok(Self { files })
    }

    /// 返回按相对路径排序的文件指纹。
    pub fn files(&self) -> &BTreeMap<PathBuf, DraftFileFingerprint> {
        &self.files
    }
}

fn capture_directory(
    root: &Path,
    current: &Path,
    files: &mut BTreeMap<PathBuf, DraftFileFingerprint>,
) -> Result<(), RuntimeError> {
    let entries = fs::read_dir(current).map_err(|error| RuntimeError::Io {
        path: current.to_path_buf(),
        message: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| RuntimeError::Io {
            path: current.to_path_buf(),
            message: error.to_string(),
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| RuntimeError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        if file_type.is_symlink() {
            return Err(RuntimeError::SymbolicLink(path));
        }
        if file_type.is_dir() {
            capture_directory(root, &path, files)?;
        } else if file_type.is_file() {
            let bytes = fs::read(&path).map_err(|error| RuntimeError::Io {
                path: path.clone(),
                message: error.to_string(),
            })?;
            let relative = path
                .strip_prefix(root)
                .map_err(|error| RuntimeError::Io {
                    path: path.clone(),
                    message: error.to_string(),
                })?
                .to_path_buf();
            files.insert(
                relative,
                DraftFileFingerprint::new(
                    format!("{:x}", Sha256::digest(&bytes)),
                    bytes.len() as u64,
                ),
            );
        }
    }
    Ok(())
}

use crate::{DraftIsolationReport, DraftTreeSnapshot, RuntimeError};
use std::fs;
use std::path::{Path, PathBuf};

/// 已有草稿的独立副本编辑会话。
#[derive(Debug, Clone)]
pub struct DraftCopySession {
    source: PathBuf,
    copy: PathBuf,
    source_before: DraftTreeSnapshot,
}

impl DraftCopySession {
    /// 将源草稿完整复制到全新目的地，并冻结源目录初始快照。
    pub fn prepare(source: impl AsRef<Path>, copy: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        let source = source.as_ref();
        let copy = copy.as_ref();
        if !source.is_dir() {
            return Err(RuntimeError::NotDirectory(source.to_path_buf()));
        }
        if copy.exists() {
            return Err(RuntimeError::DestinationExists(copy.to_path_buf()));
        }
        let canonical_source = source.canonicalize().map_err(|error| RuntimeError::Io {
            path: source.to_path_buf(),
            message: error.to_string(),
        })?;
        let copy_parent = copy.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(copy_parent).map_err(|error| RuntimeError::Io {
            path: copy_parent.to_path_buf(),
            message: error.to_string(),
        })?;
        let canonical_parent = copy_parent
            .canonicalize()
            .map_err(|error| RuntimeError::Io {
                path: copy_parent.to_path_buf(),
                message: error.to_string(),
            })?;
        if canonical_parent.starts_with(&canonical_source) {
            return Err(RuntimeError::DestinationInsideSource(copy.to_path_buf()));
        }
        let source_before = DraftTreeSnapshot::capture(source)?;
        copy_directory(source, copy)?;
        Ok(Self {
            source: source.to_path_buf(),
            copy: copy.to_path_buf(),
            source_before,
        })
    }

    /// 重新扫描源与副本，生成逐文件隔离差分证据。
    pub fn verify(&self) -> Result<DraftIsolationReport, RuntimeError> {
        Ok(DraftIsolationReport::new(
            self.source_before.clone(),
            DraftTreeSnapshot::capture(&self.source)?,
            DraftTreeSnapshot::capture(&self.copy)?,
        ))
    }

    /// 返回唯一允许本次编辑写入的副本路径。
    pub fn copy_path(&self) -> &Path {
        &self.copy
    }
}

fn copy_directory(source: &Path, destination: &Path) -> Result<(), RuntimeError> {
    fs::create_dir(destination).map_err(|error| RuntimeError::Io {
        path: destination.to_path_buf(),
        message: error.to_string(),
    })?;
    for entry in fs::read_dir(source).map_err(|error| RuntimeError::Io {
        path: source.to_path_buf(),
        message: error.to_string(),
    })? {
        let entry = entry.map_err(|error| RuntimeError::Io {
            path: source.to_path_buf(),
            message: error.to_string(),
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| RuntimeError::Io {
            path: source_path.clone(),
            message: error.to_string(),
        })?;
        if file_type.is_symlink() {
            return Err(RuntimeError::SymbolicLink(source_path));
        }
        if file_type.is_dir() {
            copy_directory(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path).map_err(|error| RuntimeError::Io {
                path: source_path,
                message: error.to_string(),
            })?;
        }
    }
    Ok(())
}

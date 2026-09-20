use crate::DraftTreeSnapshot;
use std::path::PathBuf;

/// 独立副本编辑后的源目录不变证明与副本差分。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftIsolationReport {
    source_before: DraftTreeSnapshot,
    source_after: DraftTreeSnapshot,
    copy_after: DraftTreeSnapshot,
    changed_copy_files: Vec<PathBuf>,
}

impl DraftIsolationReport {
    pub(crate) fn new(
        source_before: DraftTreeSnapshot,
        source_after: DraftTreeSnapshot,
        copy_after: DraftTreeSnapshot,
    ) -> Self {
        let changed_copy_files = source_before
            .files()
            .keys()
            .chain(copy_after.files().keys())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|path| source_before.files().get(*path) != copy_after.files().get(*path))
            .cloned()
            .collect();
        Self {
            source_before,
            source_after,
            copy_after,
            changed_copy_files,
        }
    }

    /// 源目录在准备副本后是否保持逐文件完全一致。
    pub fn source_unchanged(&self) -> bool {
        self.source_before == self.source_after
    }

    /// 返回复制前的源目录快照。
    pub fn source_before(&self) -> &DraftTreeSnapshot {
        &self.source_before
    }

    /// 返回编辑后的源目录快照。
    pub fn source_after(&self) -> &DraftTreeSnapshot {
        &self.source_after
    }

    /// 返回编辑后的副本快照。
    pub fn copy_after(&self) -> &DraftTreeSnapshot {
        &self.copy_after
    }

    /// 返回相对初始源快照发生变化、增加或删除的副本文件。
    pub fn changed_copy_files(&self) -> &[PathBuf] {
        &self.changed_copy_files
    }
}

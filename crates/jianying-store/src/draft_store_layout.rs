use crate::StoreError;
use std::path::{Component, PathBuf};

/// 将逻辑草稿名安全映射到本地草稿根目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftStoreLayout {
    root: PathBuf,
}

impl DraftStoreLayout {
    /// 创建草稿存储布局。
    pub fn new(root: PathBuf) -> Result<Self, StoreError> {
        if root.as_os_str().is_empty() {
            return Err(StoreError::EmptyRoot);
        }
        Ok(Self { root })
    }

    /// 返回安全草稿名对应路径，拒绝绝对路径和目录穿越。
    pub fn draft_path(&self, name: &str) -> Result<PathBuf, StoreError> {
        let path = PathBuf::from(name);
        let mut components = path.components();
        let safe = !name.trim().is_empty()
            && !name.contains(['/', '\\', '\0', '\n', '\r'])
            && matches!(components.next(), Some(Component::Normal(_)))
            && components.next().is_none()
            && name != "."
            && name != "..";
        if !safe {
            return Err(StoreError::UnsafeDraftName);
        }
        Ok(self.root.join(name))
    }
}

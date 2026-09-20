use serde::Serialize;
use std::path::{Path, PathBuf};

/// 本机已知的剪映或 CapCut 草稿根候选及其存在状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeDraftRoot {
    product_id: String,
    path: PathBuf,
    exists: bool,
}

impl RuntimeDraftRoot {
    /// 创建一个只读草稿根发现结果。
    pub(crate) fn new(product_id: impl Into<String>, path: PathBuf) -> Self {
        let exists = path.is_dir();
        Self {
            product_id: product_id.into(),
            path,
            exists,
        }
    }

    /// 返回产品标识。
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    /// 返回草稿根路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回草稿根当前是否存在。
    pub fn exists(&self) -> bool {
        self.exists
    }
}

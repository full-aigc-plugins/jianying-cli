use jianying_schema::ExportKind;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 经非空与 SHA-256 验证的真实原生导出制品记录。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeExportArtifact {
    export_kind: ExportKind,
    path: PathBuf,
    byte_length: u64,
    sha256: String,
}

impl NativeExportArtifact {
    pub(crate) fn new(path: PathBuf, byte_length: u64, sha256: String) -> Self {
        Self {
            export_kind: ExportKind::Native,
            path,
            byte_length,
            sha256,
        }
    }

    /// 始终返回 Native，代理渲染无法构造此类型。
    pub fn export_kind(&self) -> ExportKind {
        self.export_kind
    }

    /// 返回导出文件路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 返回导出文件字节数。
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// 返回导出文件 SHA-256。
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

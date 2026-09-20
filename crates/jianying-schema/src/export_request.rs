use crate::{ExportKind, SchemaError};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 导出请求，包含产物类型、目标路径和覆盖策略。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExportRequest {
    kind: ExportKind,
    output: PathBuf,
    overwrite: bool,
}

impl ExportRequest {
    /// 创建并校验导出请求。
    pub fn new(kind: ExportKind, output: PathBuf, overwrite: bool) -> Result<Self, SchemaError> {
        if output.as_os_str().is_empty() {
            return Err(SchemaError::InvalidField("export.output must not be empty"));
        }
        Ok(Self {
            kind,
            output,
            overwrite,
        })
    }

    /// 返回导出类型。
    pub fn kind(&self) -> ExportKind {
        self.kind
    }

    /// 返回导出目标路径。
    pub fn output(&self) -> &PathBuf {
        &self.output
    }

    /// 返回是否允许覆盖既有文件。
    pub fn overwrite(&self) -> bool {
        self.overwrite
    }
}

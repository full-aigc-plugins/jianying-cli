use serde::{Deserialize, Serialize};

/// 导出产物类型；原生导出与代理导出由能力探测决定是否可执行。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Native,
    Proxy,
    DraftArchive,
}

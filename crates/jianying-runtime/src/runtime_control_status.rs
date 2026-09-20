use serde::{Deserialize, Serialize};

/// 本 adapter 自己持有的编辑器子进程状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeControlStatus {
    Running,
    Stopped,
}

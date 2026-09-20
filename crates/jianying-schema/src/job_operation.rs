use serde::{Deserialize, Serialize};

/// `jianying-job/v2` 支持的顶层操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobOperation {
    Create,
    Edit,
    Inspect,
    Verify,
    Publish,
    Export,
    Batch,
}

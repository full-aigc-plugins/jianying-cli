use serde::{Deserialize, Serialize};

/// 草稿写事务所处的显式阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MutationPhase {
    /// 仅形成计划，尚未复制任何数据。
    Planned,
    /// 已创建不可变快照。
    Snapshotted,
    /// 已创建隔离工作副本，可在其中修改。
    WorkCopy,
    /// 工作副本已通过调用方验证。
    Validated,
    /// 工作副本验证失败，源草稿保持不变。
    Rejected,
    /// 已通过同文件系统 rename 提交。
    Committed,
}

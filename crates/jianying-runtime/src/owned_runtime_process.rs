use crate::RuntimeControlStatus;
use serde::{Deserialize, Serialize};

/// 持久运行时控制命令返回的最小进程状态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedRuntimeProcess {
    status: RuntimeControlStatus,
    pid: u32,
    profile_id: String,
    owned: bool,
}

impl OwnedRuntimeProcess {
    /// 创建运行时进程状态结果。
    pub(crate) fn new(
        status: RuntimeControlStatus,
        pid: u32,
        profile_id: impl Into<String>,
        owned: bool,
    ) -> Self {
        Self {
            status,
            pid,
            profile_id: profile_id.into(),
            owned,
        }
    }

    /// 返回运行状态。
    pub fn status(&self) -> RuntimeControlStatus {
        self.status
    }

    /// 返回进程号；停止且无记录时为零。
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 返回运行时档案标识。
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// 是否存在通过完整身份检查的所有权记录。
    pub fn owned(&self) -> bool {
        self.owned
    }
}

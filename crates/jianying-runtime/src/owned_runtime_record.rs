use crate::RuntimeFileIdentity;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 跨 CLI 调用保存的运行时进程所有权记录，仅允许控制由本工具启动的精确进程。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedRuntimeRecord {
    profile_id: String,
    pid: u32,
    process_start_time: u64,
    executable: PathBuf,
    executable_identity: RuntimeFileIdentity,
    arguments: Vec<String>,
    created_at: u64,
}

impl OwnedRuntimeRecord {
    /// 创建已验证子进程的持久所有权记录。
    pub(crate) fn new(
        profile_id: String,
        pid: u32,
        process_start_time: u64,
        executable: PathBuf,
        executable_identity: RuntimeFileIdentity,
        arguments: Vec<String>,
        created_at: u64,
    ) -> Self {
        Self {
            profile_id,
            pid,
            process_start_time,
            executable,
            executable_identity,
            arguments,
            created_at,
        }
    }

    /// 返回运行时档案标识。
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }

    /// 返回操作系统进程号。
    pub fn pid(&self) -> u32 {
        self.pid
    }

    /// 返回操作系统报告的进程启动时间。
    pub fn process_start_time(&self) -> u64 {
        self.process_start_time
    }

    /// 返回启动时验证过的可执行路径。
    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    /// 返回启动时计算的可执行文件身份。
    pub fn executable_identity(&self) -> &RuntimeFileIdentity {
        &self.executable_identity
    }

    /// 返回未经过 shell 拼接的启动参数。
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }

    /// 返回记录创建时间。
    pub fn created_at(&self) -> u64 {
        self.created_at
    }
}

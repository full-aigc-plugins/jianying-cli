use crate::{RuntimeFileIdentity, RuntimePermissions};
use serde::{Deserialize, Serialize};

/// 通过全部 fail-closed 检查后产生的运行时证据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProbeReport {
    supported: bool,
    editor_running: bool,
    materials_available: bool,
    executable_identity: RuntimeFileIdentity,
    permissions: RuntimePermissions,
}

impl RuntimeProbeReport {
    pub(crate) fn new(
        editor_running: bool,
        executable_identity: RuntimeFileIdentity,
        permissions: RuntimePermissions,
    ) -> Self {
        Self {
            supported: true,
            editor_running,
            materials_available: true,
            executable_identity,
            permissions,
        }
    }

    /// 是否通过全部兼容性门禁。
    pub fn supported(&self) -> bool {
        self.supported
    }

    /// 是否观察到匹配档案的编辑器进程。
    pub fn editor_running(&self) -> bool {
        self.editor_running
    }

    /// 所有声明素材是否均可用。
    pub fn materials_available(&self) -> bool {
        self.materials_available
    }

    /// 返回本次实际计算的可执行文件身份。
    pub fn executable_identity(&self) -> &RuntimeFileIdentity {
        &self.executable_identity
    }

    /// 返回权限探测证据。
    pub fn permissions(&self) -> &RuntimePermissions {
        &self.permissions
    }
}

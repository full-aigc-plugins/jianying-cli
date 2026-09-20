use serde::{Deserialize, Serialize};

/// 运行时探测得到的最小文件权限证据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimePermissions {
    executable_readable: bool,
    draft_root_readable: bool,
    draft_root_writable: bool,
}

impl RuntimePermissions {
    pub(crate) fn new(
        executable_readable: bool,
        draft_root_readable: bool,
        draft_root_writable: bool,
    ) -> Self {
        Self {
            executable_readable,
            draft_root_readable,
            draft_root_writable,
        }
    }

    /// 可执行文件是否可读取。
    pub fn executable_readable(&self) -> bool {
        self.executable_readable
    }

    /// 草稿根目录是否可读取。
    pub fn draft_root_readable(&self) -> bool {
        self.draft_root_readable
    }

    /// 草稿根目录是否未被标记为只读。
    pub fn draft_root_writable(&self) -> bool {
        self.draft_root_writable
    }
}

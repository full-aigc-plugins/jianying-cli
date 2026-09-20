use crate::RuntimePlatform;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// 一次本机运行时探测的观察输入。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProbeInput {
    pub(crate) product: String,
    pub(crate) version: String,
    pub(crate) platform: RuntimePlatform,
    pub(crate) executable_path: PathBuf,
    pub(crate) draft_root: PathBuf,
    pub(crate) running_processes: BTreeSet<String>,
    pub(crate) material_paths: Vec<PathBuf>,
    pub(crate) requested_capabilities: BTreeSet<String>,
}

impl RuntimeProbeInput {
    /// 创建探测输入；附加进程、素材和能力由链式方法提供。
    pub fn new(
        product: impl Into<String>,
        version: impl Into<String>,
        platform: RuntimePlatform,
        executable_path: impl Into<PathBuf>,
        draft_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            product: product.into(),
            version: version.into(),
            platform,
            executable_path: executable_path.into(),
            draft_root: draft_root.into(),
            running_processes: BTreeSet::new(),
            material_paths: Vec::new(),
            requested_capabilities: BTreeSet::new(),
        }
    }

    /// 记录当前观察到的进程名。
    pub fn with_running_processes<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.running_processes = values.into_iter().map(Into::into).collect();
        self
    }

    /// 记录任务需要读取的素材路径。
    pub fn with_material_paths<I, P>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.material_paths = values.into_iter().map(Into::into).collect();
        self
    }

    /// 记录任务要求的 capability。
    pub fn with_requested_capabilities<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.requested_capabilities = values.into_iter().map(Into::into).collect();
        self
    }
}

use crate::{RuntimeFileIdentity, RuntimePlatform};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// 只读发现到的本机编辑器安装；发现结果本身不等于受支持 Runtime Profile。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeInstallation {
    product_id: String,
    product: String,
    bundle_id: Option<String>,
    version: Option<String>,
    platform: RuntimePlatform,
    installation_root: PathBuf,
    executable_identity: RuntimeFileIdentity,
    process_name: String,
    draft_roots: Vec<PathBuf>,
    support_status: String,
    automatic_routing: bool,
    next: String,
}

impl RuntimeInstallation {
    /// 建立发现记录，并固定可执行文件内容身份；能力保持未验证且禁止自动路由。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        product_id: impl Into<String>,
        product: impl Into<String>,
        bundle_id: Option<String>,
        version: Option<String>,
        platform: RuntimePlatform,
        installation_root: PathBuf,
        executable_identity: RuntimeFileIdentity,
        process_name: impl Into<String>,
        draft_roots: Vec<PathBuf>,
    ) -> Self {
        Self {
            product_id: product_id.into(),
            product: product.into(),
            bundle_id,
            version,
            platform,
            installation_root,
            executable_identity,
            process_name: process_name.into(),
            draft_roots,
            support_status: "unverified".to_owned(),
            automatic_routing: false,
            next: "create and validate an exact Runtime Profile before native control".to_owned(),
        }
    }

    /// 返回产品标识。
    pub fn product_id(&self) -> &str {
        &self.product_id
    }

    /// 返回安装根目录。
    pub fn installation_root(&self) -> &Path {
        &self.installation_root
    }

    /// 返回可执行文件内容身份。
    pub fn executable_identity(&self) -> &RuntimeFileIdentity {
        &self.executable_identity
    }

    /// 返回可得的产品版本；无法安全读取时为 None。
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }
}

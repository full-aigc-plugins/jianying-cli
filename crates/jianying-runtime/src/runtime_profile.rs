use crate::{
    RuntimeError, RuntimeFileIdentity, RuntimePermissions, RuntimePlatform, RuntimeProbeInput,
    RuntimeProbeReport,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// 经 doctor 探测后供调度器选择的本机运行时档案。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeProfile {
    id: String,
    product: String,
    version: String,
    platform: RuntimePlatform,
    capabilities: BTreeSet<String>,
    file_identity: Option<RuntimeFileIdentity>,
    process_names: BTreeSet<String>,
    draft_roots: Vec<PathBuf>,
}

impl RuntimeProfile {
    /// 创建运行时档案并校验身份和能力键。
    pub fn new<I, S>(
        id: impl Into<String>,
        product: impl Into<String>,
        version: impl Into<String>,
        platform: RuntimePlatform,
        capabilities: I,
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let id = id.into();
        let product = product.into();
        let version = version.into();
        for (name, value) in [("id", &id), ("product", &product), ("version", &version)] {
            if value.trim().is_empty() {
                return Err(RuntimeError::EmptyField(name));
            }
        }
        let capabilities: BTreeSet<String> = capabilities.into_iter().map(Into::into).collect();
        if capabilities.iter().any(|value| value.trim().is_empty()) {
            return Err(RuntimeError::EmptyCapability);
        }
        Ok(Self {
            id,
            product,
            version,
            platform,
            capabilities,
            file_identity: None,
            process_names: BTreeSet::new(),
            draft_roots: Vec::new(),
        })
    }

    /// 绑定经内容哈希确认的可执行文件身份。
    pub fn with_file_identity(mut self, value: RuntimeFileIdentity) -> Self {
        self.file_identity = Some(value);
        self
    }

    /// 声明用于判断编辑器是否运行的精确进程名。
    pub fn with_process_names<I, S>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.process_names = values.into_iter().map(Into::into).collect();
        self
    }

    /// 声明允许访问的草稿根目录。
    pub fn with_draft_roots<I, P>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: Into<PathBuf>,
    {
        self.draft_roots = values.into_iter().map(Into::into).collect();
        self
    }

    /// 判断档案是否声明给定能力。
    pub fn supports(&self, capability: &str) -> bool {
        self.capabilities.contains(capability)
    }

    /// 返回档案标识。
    pub fn id(&self) -> &str {
        &self.id
    }

    /// 返回产品名。
    pub fn product(&self) -> &str {
        &self.product
    }

    /// 返回精确支持版本。
    pub fn version(&self) -> &str {
        &self.version
    }

    /// 返回平台。
    pub fn platform(&self) -> RuntimePlatform {
        self.platform
    }

    /// 返回绑定的文件身份。
    pub fn file_identity(&self) -> Option<&RuntimeFileIdentity> {
        self.file_identity.as_ref()
    }

    /// 返回匹配编辑器的进程名集合。
    pub fn process_names(&self) -> &BTreeSet<String> {
        &self.process_names
    }

    /// 返回批准的草稿根目录。
    pub fn draft_roots(&self) -> &[PathBuf] {
        &self.draft_roots
    }

    /// 对观察输入执行产品、版本、平台、身份、权限、路径、素材和 capability 门禁。
    pub fn probe(&self, input: RuntimeProbeInput) -> Result<RuntimeProbeReport, RuntimeError> {
        if input.product != self.product {
            return Err(RuntimeError::UnsupportedProduct {
                expected: self.product.clone(),
                observed: input.product,
            });
        }
        if input.version != self.version {
            return Err(RuntimeError::UnsupportedVersion {
                expected: self.version.clone(),
                observed: input.version,
            });
        }
        if input.platform != self.platform {
            return Err(RuntimeError::UnsupportedPlatform);
        }
        for capability in &input.requested_capabilities {
            if !self.supports(capability) {
                return Err(RuntimeError::UnsupportedCapability(capability.clone()));
            }
        }
        let expected_identity = self
            .file_identity
            .as_ref()
            .ok_or(RuntimeError::MissingFileIdentity)?;
        let observed_identity = RuntimeFileIdentity::from_path(&input.executable_path)?;
        if observed_identity != *expected_identity {
            return Err(RuntimeError::FileIdentityMismatch(input.executable_path));
        }
        if !self.draft_roots.is_empty()
            && !self
                .draft_roots
                .iter()
                .any(|root| same_path(root, &input.draft_root))
        {
            return Err(RuntimeError::UnapprovedDraftRoot(input.draft_root));
        }
        if !input.draft_root.is_dir() {
            return Err(RuntimeError::NotDirectory(input.draft_root));
        }
        let root_metadata =
            std::fs::metadata(&input.draft_root).map_err(|error| RuntimeError::Io {
                path: input.draft_root.clone(),
                message: error.to_string(),
            })?;
        let root_writable = !root_metadata.permissions().readonly();
        if !root_writable {
            return Err(RuntimeError::DraftRootNotWritable(input.draft_root));
        }
        for material in &input.material_paths {
            if !material.is_file() {
                return Err(RuntimeError::MaterialUnavailable(material.clone()));
            }
        }
        let editor_running = self
            .process_names
            .iter()
            .any(|name| input.running_processes.contains(name));
        Ok(RuntimeProbeReport::new(
            editor_running,
            observed_identity,
            RuntimePermissions::new(true, true, root_writable),
        ))
    }
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

use crate::{CredentialSource, TtsError};
use serde::{Deserialize, Serialize};

/// 指向环境变量或宿主秘密存储的凭据引用，永不持有秘密明文。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRef {
    source: CredentialSource,
    name: String,
}

impl CredentialRef {
    /// 创建凭据引用。
    ///
    /// `name` 是环境变量名或宿主秘密条目名，不得传入真实 token。
    pub fn new(source: CredentialSource, name: impl Into<String>) -> Result<Self, TtsError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(TtsError::InvalidCredentialRef);
        }
        Ok(Self { source, name })
    }

    /// 返回凭据来源。
    pub fn source(&self) -> CredentialSource {
        self.source
    }

    /// 返回可审计的引用名，不读取秘密值。
    pub fn name(&self) -> &str {
        &self.name
    }
}

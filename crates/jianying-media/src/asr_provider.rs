use crate::{AsrArtifact, AsrError, AsrProviderCapability, AsrRequest};
use std::path::Path;

/// 所有本地或远端 ASR adapter 的统一接口。
pub trait AsrProvider: Send + Sync {
    /// 返回含执行器身份和费用属性的能力声明。
    fn capability(&self) -> &AsrProviderCapability;

    /// 转录源音频到指定产物路径。
    fn transcribe(
        &self,
        request: &AsrRequest,
        source: &Path,
        output: &Path,
    ) -> Result<AsrArtifact, AsrError>;
}

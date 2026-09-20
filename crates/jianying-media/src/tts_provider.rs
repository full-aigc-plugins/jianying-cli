use crate::{ProviderCapability, TtsArtifact, TtsError, TtsRequest};
use std::path::Path;

/// 所有本地、系统和云端 TTS adapter 必须实现的统一接口。
pub trait TtsProvider: Send + Sync {
    /// 返回可机器验证的 Provider 能力声明。
    fn capability(&self) -> &ProviderCapability;

    /// 合成语音到指定输出路径，并返回带 Provider 身份的产物。
    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError>;
}

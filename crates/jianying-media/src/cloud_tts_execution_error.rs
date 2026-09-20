use thiserror::Error;

/// 云端 TTS 提交的结果分类，用于决定账本进入 failed 还是 ambiguous。
#[derive(Debug, Error)]
pub enum CloudTtsExecutionError {
    /// 请求尚未发送，或厂商已明确拒绝，可在新审批下显式重试。
    #[error("cloud TTS request failed definitively: {0}")]
    Definite(String),
    /// 请求可能已被厂商接收，必须先对账，禁止直接重提。
    #[error("cloud TTS request outcome is ambiguous: {0}")]
    Ambiguous(String),
}

impl CloudTtsExecutionError {
    /// 返回错误是否属于无法确认远端是否接收的 ambiguous 状态。
    pub fn is_ambiguous(&self) -> bool {
        matches!(self, Self::Ambiguous(_))
    }
}

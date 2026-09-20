use serde::{Deserialize, Serialize};

/// 云端 TTS 的执行用途；真实 canary 使用独立审批命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsExecutionMode {
    Production,
    LiveCanary,
}

impl TtsExecutionMode {
    /// 返回审批系统使用的稳定命令名。
    pub fn approval_command(self) -> &'static str {
        match self {
            Self::Production => "tts.cloud.submit",
            Self::LiveCanary => "tts.cloud.live-canary",
        }
    }

    pub(crate) fn key_name(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::LiveCanary => "live_canary",
        }
    }
}

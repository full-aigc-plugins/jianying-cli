/// 本地 TTS 运行时对外暴露的调用协议。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalTtsTransport {
    /// OpenAI `/v1/audio/speech` 兼容接口，例如 vLLM-Omni。
    OpenAiAudio,
    /// CosyVoice 官方 FastAPI 运行时接口。
    CosyVoiceFastApi,
    /// 通过结构化参数启动本地进程。
    Process,
    /// 使用运行时自有的本地 HTTP 接口。
    GenericHttp,
}

impl LocalTtsTransport {
    /// 返回稳定的机器可读传输标识。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenAiAudio => "openai-audio",
            Self::CosyVoiceFastApi => "cosyvoice-fastapi",
            Self::Process => "process",
            Self::GenericHttp => "generic-http",
        }
    }
}

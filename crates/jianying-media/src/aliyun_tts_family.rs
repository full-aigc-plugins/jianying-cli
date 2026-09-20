/// 阿里百炼不同语音模型所使用的官方协议族。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliyunTtsFamily {
    QwenAudio,
    CosyVoice,
    QwenTts,
}

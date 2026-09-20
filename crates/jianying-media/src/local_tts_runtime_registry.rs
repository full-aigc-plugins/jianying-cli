use crate::{CommercialUsePolicy, LocalTtsRuntimeKind, LocalTtsRuntimeProfile, LocalTtsTransport};

/// CLI 内置的本地开源 TTS 运行时来源与许可证注册表。
#[derive(Clone, Debug)]
pub struct LocalTtsRuntimeRegistry {
    profiles: Vec<LocalTtsRuntimeProfile>,
}

impl LocalTtsRuntimeRegistry {
    /// 构造经过来源分类的内置档案；不会安装依赖或下载模型权重。
    pub fn built_in() -> Self {
        Self {
            profiles: vec![
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::Qwen3Tts,
                    "https://github.com/QwenLM/Qwen3-TTS",
                    "Apache-2.0",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::AllowedAfterArtifactReview,
                    LocalTtsTransport::Process,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::CosyVoice,
                    "https://github.com/FunAudioLLM/CosyVoice",
                    "Apache-2.0",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::AllowedAfterArtifactReview,
                    LocalTtsTransport::CosyVoiceFastApi,
                    Some("http://127.0.0.1:50000/inference_sft"),
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::GptSovits,
                    "https://github.com/RVC-Boss/GPT-SoVITS",
                    "MIT",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::AllowedAfterArtifactReview,
                    LocalTtsTransport::GenericHttp,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::MeloTts,
                    "https://github.com/myshell-ai/MeloTTS",
                    "MIT",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::AllowedAfterArtifactReview,
                    LocalTtsTransport::Process,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::EmotiVoice,
                    "https://github.com/netease-youdao/EmotiVoice",
                    "Apache-2.0",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::AllowedAfterArtifactReview,
                    LocalTtsTransport::OpenAiAudio,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::Mars5Tts,
                    "https://github.com/Camb-ai/MARS5-TTS",
                    "AGPL-3.0-only",
                    "selected-artifact-review-required",
                    CommercialUsePolicy::CopyleftComplianceRequired,
                    LocalTtsTransport::Process,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::ChatTts,
                    "https://github.com/2noise/ChatTTS",
                    "AGPL-3.0-or-later",
                    "CC-BY-NC-4.0",
                    CommercialUsePolicy::NonCommercial,
                    LocalTtsTransport::GenericHttp,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::F5Tts,
                    "https://github.com/SWivid/F5-TTS",
                    "MIT",
                    "CC-BY-NC-4.0-public-base-weights",
                    CommercialUsePolicy::NonCommercial,
                    LocalTtsTransport::Process,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::FishSpeech,
                    "https://github.com/fishaudio/fish-speech",
                    "Fish-Audio-Research-License",
                    "Fish-Audio-Research-License",
                    CommercialUsePolicy::PermissionRequired,
                    LocalTtsTransport::GenericHttp,
                    None,
                ),
                LocalTtsRuntimeProfile::new(
                    LocalTtsRuntimeKind::IndexTts,
                    "https://github.com/index-tts/index-tts",
                    "bilibili-model-use-license-agreement",
                    "bilibili-model-use-license-agreement",
                    CommercialUsePolicy::PermissionRequired,
                    LocalTtsTransport::GenericHttp,
                    None,
                ),
            ],
        }
    }

    /// 按运行时种类查找档案。
    pub fn find(&self, kind: LocalTtsRuntimeKind) -> Option<&LocalTtsRuntimeProfile> {
        self.profiles.iter().find(|profile| profile.kind() == kind)
    }

    /// 返回全部内置档案的只读切片。
    pub fn profiles(&self) -> &[LocalTtsRuntimeProfile] {
        &self.profiles
    }
}

impl Default for LocalTtsRuntimeRegistry {
    fn default() -> Self {
        Self::built_in()
    }
}

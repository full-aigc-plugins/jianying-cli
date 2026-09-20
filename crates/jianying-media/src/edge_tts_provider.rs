use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 通过结构化 argv 调用 EdgeTTS 客户端的联网 TTS Provider。
#[derive(Debug)]
pub struct EdgeTtsProvider {
    executable: PathBuf,
    capability: ProviderCapability,
}

impl EdgeTtsProvider {
    /// 创建 EdgeTTS Provider；可执行文件必须使用绝对路径。
    pub fn new(executable: impl Into<PathBuf>) -> Result<Self, TtsError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(TtsError::InvalidLocalProcess(
                "EdgeTTS executable path must be absolute".to_owned(),
            ));
        }
        let capability = ProviderCapability::new(
            "edge-tts",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([TtsAudioFormat::Mp3]),
            BTreeSet::from([TtsOutputMode::File]),
            10_000,
        )?
        .with_voice_catalog(true)
        .with_speed_control(true)
        .with_volume_control(true);
        Ok(Self {
            executable,
            capability,
        })
    }

    /// 返回是否需要访问外部语音服务；EdgeTTS 固定为是。
    pub fn requires_network(&self) -> bool {
        true
    }

    fn percentage(value: f32) -> String {
        format!("{:+.0}%", (value - 1.0) * 100.0)
    }
}

impl TtsProvider for EdgeTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|error| {
                TtsError::Provider(format!(
                    "could not create EdgeTTS output directory: {error}"
                ))
            })?;
        }
        let partial = output.with_extension(format!(
            "{}.partial-{}",
            output
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("mp3"),
            std::process::id()
        ));
        let mut command = Command::new(&self.executable);
        if let Some(voice) = request.voice() {
            command.arg("--voice").arg(voice);
        }
        if request.speed() != 1.0 {
            command.arg(format!("--rate={}", Self::percentage(request.speed())));
        }
        if request.volume() != 1.0 {
            command.arg(format!("--volume={}", Self::percentage(request.volume())));
        }
        command
            .arg("--text")
            .arg(request.text())
            .arg("--write-media")
            .arg(&partial)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let result = command.output().map_err(|error| {
            TtsError::Provider(format!("EdgeTTS process could not start: {error}"))
        })?;
        if !result.status.success() {
            let _ = std::fs::remove_file(&partial);
            return Err(TtsError::Provider(format!(
                "EdgeTTS process failed ({}): {}",
                result.status,
                String::from_utf8_lossy(&result.stderr).trim()
            )));
        }
        let byte_length = std::fs::metadata(&partial)
            .map(|metadata| metadata.len())
            .unwrap_or(0);
        if byte_length == 0 {
            let _ = std::fs::remove_file(&partial);
            return Err(TtsError::EmptyArtifact);
        }
        std::fs::rename(&partial, output).map_err(|error| {
            let _ = std::fs::remove_file(&partial);
            TtsError::Provider(format!("could not commit EdgeTTS artifact: {error}"))
        })?;
        TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            self.capability.provider_id(),
            byte_length,
        )
    }
}

use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// 使用 macOS 内置 `say` 命令生成 AIFF 语音。
#[derive(Debug)]
pub struct MacOsSayTtsProvider {
    capability: ProviderCapability,
}

impl MacOsSayTtsProvider {
    /// 创建 macOS 系统 TTS Provider 的静态能力描述。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "system-macos",
            false,
            TtsPlatformCondition::MacOs,
            TtsAuthScheme::None,
            BTreeSet::from([TtsAudioFormat::Aiff]),
            BTreeSet::from([TtsOutputMode::File]),
            32_768,
        )?
        .with_voice_catalog(true)
        .with_speed_control(true);
        Ok(Self { capability })
    }

    /// 探测当前主机是否为 macOS 且存在 `say` 命令。
    pub fn available(&self) -> bool {
        cfg!(target_os = "macos")
            && Command::new("say")
                .arg("-v")
                .arg("?")
                .output()
                .is_ok_and(|output| output.status.success())
    }
}

impl TtsProvider for MacOsSayTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        if !self.available() {
            return Err(TtsError::Provider(
                "macOS say is unavailable on this host".to_owned(),
            ));
        }
        let mut command = Command::new("say");
        command.arg("-o").arg(output);
        if let Some(voice) = request.voice() {
            command.arg("-v").arg(voice);
        }
        if request.speed() != 1.0 {
            let words_per_minute = (175.0 * request.speed()).round() as u32;
            command.arg("-r").arg(words_per_minute.to_string());
        }
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let mut child = command
            .spawn()
            .map_err(|error| TtsError::Provider(format!("could not start macOS say: {error}")))?;
        child
            .stdin
            .take()
            .ok_or_else(|| TtsError::Provider("macOS say stdin unavailable".to_owned()))?
            .write_all(request.text().as_bytes())
            .map_err(|error| TtsError::Provider(format!("could not write say stdin: {error}")))?;
        let result = child
            .wait_with_output()
            .map_err(|error| TtsError::Provider(format!("could not wait for say: {error}")))?;
        if !result.status.success() {
            let _ = std::fs::remove_file(output);
            return Err(TtsError::Provider(format!(
                "macOS say failed ({}): {}",
                result.status,
                String::from_utf8_lossy(&result.stderr).trim()
            )));
        }
        let byte_length = std::fs::metadata(output)
            .map(|value| value.len())
            .unwrap_or(0);
        TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            self.capability.provider_id(),
            byte_length,
        )
    }
}

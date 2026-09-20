use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// 使用绝对可执行文件路径和结构化 argv 调用本地 TTS 模型进程。
#[derive(Debug)]
pub struct LocalProcessTtsProvider {
    executable: PathBuf,
    argument_templates: Vec<String>,
    capability: ProviderCapability,
    text_delivery: &'static str,
}

impl LocalProcessTtsProvider {
    /// 创建本地进程 Provider，参数必须包含 `{out}`，文本使用 `{text}` 或 stdin 交付。
    pub fn new(
        executable: impl Into<PathBuf>,
        argument_templates: Vec<String>,
        format: TtsAudioFormat,
    ) -> Result<Self, TtsError> {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(TtsError::InvalidLocalProcess(
                "executable path must be absolute".to_owned(),
            ));
        }
        if !argument_templates
            .iter()
            .any(|argument| argument.contains("{out}"))
        {
            return Err(TtsError::InvalidLocalProcess(
                "arguments must contain an {out} placeholder".to_owned(),
            ));
        }
        let text_delivery = if argument_templates
            .iter()
            .any(|argument| argument.contains("{text}"))
        {
            "argv"
        } else {
            "stdin"
        };
        let has_placeholder = |name: &str| {
            let placeholder = format!("{{{name}}}");
            argument_templates
                .iter()
                .any(|argument| argument.contains(&placeholder))
        };
        let capability = ProviderCapability::new(
            "local-process",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([format]),
            BTreeSet::from([TtsOutputMode::File]),
            1_000_000,
        )?
        .with_model_selection(has_placeholder("model"))
        .with_voice_catalog(has_placeholder("voice"))
        .with_language_selection(has_placeholder("language"))
        .with_speed_control(has_placeholder("speed"))
        .with_volume_control(has_placeholder("volume"))
        .with_pitch_control(has_placeholder("pitch"));
        Ok(Self {
            executable,
            argument_templates,
            capability,
            text_delivery,
        })
    }

    /// 返回文本通过 argv 或 stdin 交付。
    pub fn text_delivery(&self) -> &'static str {
        self.text_delivery
    }
}

impl TtsProvider for LocalProcessTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        let output_text = output.to_string_lossy();
        let arguments: Vec<String> = self
            .argument_templates
            .iter()
            .map(|argument| {
                argument
                    .replace("{out}", &output_text)
                    .replace("{text}", request.text())
                    .replace("{model}", request.model().unwrap_or_default())
                    .replace("{voice}", request.voice().unwrap_or_default())
                    .replace("{language}", request.language().unwrap_or_default())
                    .replace("{speed}", &request.speed().to_string())
                    .replace("{volume}", &request.volume().to_string())
                    .replace("{pitch}", &request.pitch().to_string())
            })
            .collect();
        let mut command = Command::new(&self.executable);
        command
            .args(arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if self.text_delivery == "stdin" {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| {
            TtsError::Provider(format!(
                "local process could not start at {}: {error}",
                self.executable.display()
            ))
        })?;
        if self.text_delivery == "stdin" {
            child
                .stdin
                .take()
                .ok_or_else(|| TtsError::Provider("local process stdin unavailable".to_owned()))?
                .write_all(request.text().as_bytes())
                .map_err(|error| {
                    TtsError::Provider(format!("could not write local process stdin: {error}"))
                })?;
        }
        let result = child.wait_with_output().map_err(|error| {
            TtsError::Provider(format!("could not wait for local process: {error}"))
        })?;
        if !result.status.success() {
            let _ = std::fs::remove_file(output);
            return Err(TtsError::Provider(format!(
                "local process failed ({}): {}",
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

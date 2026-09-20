use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsInputKind,
    TtsOutputMode, TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// 使用 Windows System.Speech 生成 WAV 语音。
#[derive(Debug)]
pub struct WindowsSystemTtsProvider {
    capability: ProviderCapability,
}

impl WindowsSystemTtsProvider {
    /// 创建 Windows 系统 TTS Provider 的静态能力描述。
    pub fn new() -> Result<Self, TtsError> {
        let capability = ProviderCapability::new(
            "system-windows",
            false,
            TtsPlatformCondition::Windows,
            TtsAuthScheme::None,
            BTreeSet::from([TtsAudioFormat::Wav]),
            BTreeSet::from([TtsOutputMode::File]),
            32_768,
        )?
        .with_voice_catalog(true)
        .with_ssml(true)
        .with_speed_control(true)
        .with_volume_control(true);
        Ok(Self { capability })
    }

    /// 探测当前主机是否为 Windows 且系统 PowerShell 可用。
    pub fn available(&self) -> bool {
        cfg!(target_os = "windows")
            && Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "$PSVersionTable.PSVersion.ToString()",
                ])
                .output()
                .is_ok_and(|output| output.status.success())
    }
}

impl TtsProvider for WindowsSystemTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        if !self.available() {
            return Err(TtsError::Provider(
                "Windows system TTS is unavailable on this host".to_owned(),
            ));
        }
        let script = r#"
Add-Type -AssemblyName System.Speech
$synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
try {
  if ($env:JY_TTS_VOICE) { $synth.SelectVoice($env:JY_TTS_VOICE) }
  $synth.Rate = [int]$env:JY_TTS_RATE
  $synth.Volume = [int]$env:JY_TTS_VOLUME
  $synth.SetOutputToWaveFile($env:JY_TTS_OUT)
  $content = [Console]::In.ReadToEnd()
  if ($env:JY_TTS_INPUT_KIND -eq 'ssml') { $synth.SpeakSsml($content) } else { $synth.Speak($content) }
} finally {
  $synth.Dispose()
}
"#;
        let rate = ((request.speed() - 1.0) * 6.0).round().clamp(-10.0, 10.0) as i32;
        let volume = (request.volume() * 100.0).round().clamp(0.0, 100.0) as u32;
        let mut command = Command::new("powershell.exe");
        command
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("JY_TTS_OUT", output)
            .env("JY_TTS_RATE", rate.to_string())
            .env("JY_TTS_VOLUME", volume.to_string())
            .env(
                "JY_TTS_INPUT_KIND",
                if request.input_kind() == TtsInputKind::Ssml {
                    "ssml"
                } else {
                    "text"
                },
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if let Some(voice) = request.voice() {
            command.env("JY_TTS_VOICE", voice);
        }
        let mut child = command.spawn().map_err(|error| {
            TtsError::Provider(format!("could not start Windows system TTS: {error}"))
        })?;
        child
            .stdin
            .take()
            .ok_or_else(|| TtsError::Provider("Windows TTS stdin unavailable".to_owned()))?
            .write_all(request.text().as_bytes())
            .map_err(|error| {
                TtsError::Provider(format!("could not write Windows TTS stdin: {error}"))
            })?;
        let result = child.wait_with_output().map_err(|error| {
            TtsError::Provider(format!("could not wait for Windows TTS: {error}"))
        })?;
        if !result.status.success() {
            let _ = std::fs::remove_file(output);
            return Err(TtsError::Provider(format!(
                "Windows system TTS failed ({}): {}",
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

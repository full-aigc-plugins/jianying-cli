use crate::{
    AsrArtifact, AsrError, AsrOutputFormat, AsrProvider, AsrProviderCapability, AsrRequest,
    AsrTimestampGranularity,
};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// 官方 whisper.cpp `whisper-cli` 的本地、无 shell 进程适配器。
pub struct WhisperCppAsrProvider {
    executable: PathBuf,
    model: PathBuf,
    model_id: String,
    capability: AsrProviderCapability,
}

impl WhisperCppAsrProvider {
    /// 固定可执行文件、模型制品和逻辑模型标识，并把二者哈希纳入 executor identity。
    pub fn new(
        executable: impl Into<PathBuf>,
        model: impl Into<PathBuf>,
        model_id: impl Into<String>,
    ) -> Result<Self, AsrError> {
        let executable = executable.into();
        let model = model.into();
        let model_id = model_id.into();
        if !executable.is_absolute() || !executable.is_file() {
            return Err(AsrError::InvalidLocalProcess(
                "whisper-cli executable must be an absolute existing file".to_owned(),
            ));
        }
        if !model.is_absolute() || !model.is_file() {
            return Err(AsrError::InvalidLocalProcess(
                "whisper.cpp model must be an absolute existing file".to_owned(),
            ));
        }
        if model_id.trim().is_empty() {
            return Err(AsrError::InvalidLocalProcess(
                "whisper.cpp model id must not be empty".to_owned(),
            ));
        }
        let executable_sha256 = sha256_file(&executable)?;
        let model_sha256 = sha256_file(&model)?;
        let capability = AsrProviderCapability::new(
            "whisper-cpp",
            format!("whisper.cpp@sha256:{executable_sha256};model@sha256:{model_sha256}"),
            false,
            [
                AsrOutputFormat::Json,
                AsrOutputFormat::Text,
                AsrOutputFormat::Srt,
                AsrOutputFormat::VerboseJson,
                AsrOutputFormat::Vtt,
            ],
            [AsrTimestampGranularity::Segment],
        )?;
        Ok(Self {
            executable,
            model,
            model_id,
            capability,
        })
    }

    /// 返回固定的本地模型标识。
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    fn generated_path(prefix: &Path, format: AsrOutputFormat) -> PathBuf {
        PathBuf::from(format!("{}.{}", prefix.display(), output_extension(format)))
    }
}

impl AsrProvider for WhisperCppAsrProvider {
    fn capability(&self) -> &AsrProviderCapability {
        &self.capability
    }

    fn transcribe(
        &self,
        request: &AsrRequest,
        source: &Path,
        output: &Path,
    ) -> Result<AsrArtifact, AsrError> {
        self.capability.validate(request)?;
        if !source.is_file() {
            return Err(AsrError::SourceMissing(source.to_path_buf()));
        }
        if sha256_file(source)? != request.source_sha256() {
            return Err(AsrError::SourceDigestMismatch);
        }
        if let Some(requested) = request.model() {
            if requested != self.model_id {
                return Err(AsrError::ModelMismatch {
                    requested: requested.to_owned(),
                    configured: self.model_id.clone(),
                });
            }
        }
        if output.exists() {
            return Err(AsrError::InvalidLocalProcess(format!(
                "output already exists: {}",
                output.display()
            )));
        }
        let parent = output.parent().ok_or_else(|| {
            AsrError::InvalidLocalProcess("output must have a parent directory".to_owned())
        })?;
        std::fs::create_dir_all(parent).map_err(|error| AsrError::Io(error.to_string()))?;
        let prefix = parent.join(format!(
            ".jianying-whisper-{}.partial",
            uuid::Uuid::new_v4().simple()
        ));
        let generated = Self::generated_path(&prefix, request.output_format());
        let mut command = Command::new(&self.executable);
        command
            .arg("--model")
            .arg(&self.model)
            .arg("--file")
            .arg(source)
            .arg("--output-file")
            .arg(&prefix)
            .arg(output_flag(request.output_format()))
            .arg("--no-prints");
        if let Some(language) = request.language() {
            command.arg("--language").arg(language);
        }
        if request.translate_to_english() {
            command.arg("--translate");
        }
        let process = command
            .output()
            .map_err(|error| AsrError::InvalidLocalProcess(error.to_string()))?;
        if !process.status.success() {
            let _ = std::fs::remove_file(&generated);
            return Err(AsrError::ProcessFailed {
                code: process.status.code(),
                stderr: bounded_stderr(&process.stderr),
            });
        }
        let length = std::fs::metadata(&generated)
            .map_err(|_| AsrError::EmptyArtifact(generated.clone()))?
            .len();
        if length == 0 {
            let _ = std::fs::remove_file(&generated);
            return Err(AsrError::EmptyArtifact(generated));
        }
        std::fs::rename(&generated, output).map_err(|error| {
            let _ = std::fs::remove_file(&generated);
            AsrError::Io(error.to_string())
        })?;
        AsrArtifact::from_path(
            output,
            request.output_format(),
            self.capability.provider_id(),
            request.source_sha256(),
        )
    }
}

fn output_flag(format: AsrOutputFormat) -> &'static str {
    match format {
        AsrOutputFormat::Json => "--output-json",
        AsrOutputFormat::Text => "--output-txt",
        AsrOutputFormat::Srt => "--output-srt",
        AsrOutputFormat::VerboseJson => "--output-json-full",
        AsrOutputFormat::Vtt => "--output-vtt",
    }
}

fn output_extension(format: AsrOutputFormat) -> &'static str {
    match format {
        AsrOutputFormat::Json | AsrOutputFormat::VerboseJson => "json",
        AsrOutputFormat::Text => "txt",
        AsrOutputFormat::Srt => "srt",
        AsrOutputFormat::Vtt => "vtt",
    }
}

fn sha256_file(path: &Path) -> Result<String, AsrError> {
    let mut file = File::open(path).map_err(|error| AsrError::Io(error.to_string()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| AsrError::Io(error.to_string()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn bounded_stderr(stderr: &[u8]) -> String {
    const LIMIT: usize = 4_096;
    String::from_utf8_lossy(&stderr[..stderr.len().min(LIMIT)]).into_owned()
}

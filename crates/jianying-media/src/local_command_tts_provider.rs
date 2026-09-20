use crate::{
    ProviderCapability, TtsArtifact, TtsAudioFormat, TtsAuthScheme, TtsError, TtsOutputMode,
    TtsPlatformCondition, TtsProvider, TtsRequest,
};
use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// 通过无 shell 的显式命令模板执行本地 TTS。
#[derive(Debug)]
pub struct LocalCommandTtsProvider {
    command_template: String,
    capability: ProviderCapability,
    text_delivery: &'static str,
}

impl LocalCommandTtsProvider {
    /// 创建本地命令 Provider。
    ///
    /// 模板必须包含 `{out}`；包含 `{text}` 时文本作为单个 argv 传入，否则通过 stdin 传入。
    pub fn new(
        command_template: impl Into<String>,
        format: TtsAudioFormat,
    ) -> Result<Self, TtsError> {
        let command_template = command_template.into();
        let tokens = split_command_template(&command_template)?;
        if tokens.is_empty() {
            return Err(TtsError::InvalidCommandTemplate(
                "template is empty".to_owned(),
            ));
        }
        if !tokens.iter().any(|token| token.contains("{out}")) {
            return Err(TtsError::InvalidCommandTemplate(
                "template must contain an {out} placeholder".to_owned(),
            ));
        }
        let text_delivery = if tokens.iter().any(|token| token.contains("{text}")) {
            "argv"
        } else {
            "stdin"
        };
        let capability = ProviderCapability::new(
            "local-command",
            false,
            TtsPlatformCondition::Any,
            TtsAuthScheme::None,
            BTreeSet::from([format]),
            BTreeSet::from([TtsOutputMode::File]),
            1_000_000,
        )?;
        Ok(Self {
            command_template,
            capability,
            text_delivery,
        })
    }

    /// 返回文本通过 argv 或 stdin 交付，便于审计兼容命令行为。
    pub fn text_delivery(&self) -> &'static str {
        self.text_delivery
    }
}

impl TtsProvider for LocalCommandTtsProvider {
    fn capability(&self) -> &ProviderCapability {
        &self.capability
    }

    fn synthesize(&self, request: &TtsRequest, output: &Path) -> Result<TtsArtifact, TtsError> {
        self.capability.validate_request(request)?;
        let tokens = split_command_template(&self.command_template)?;
        let output_text = output.to_string_lossy();
        let argv: Vec<String> = tokens
            .into_iter()
            .map(|token| {
                token
                    .replace("{out}", &output_text)
                    .replace("{text}", request.text())
            })
            .collect();
        let mut command = Command::new(&argv[0]);
        command
            .args(&argv[1..])
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if self.text_delivery == "stdin" {
            command.stdin(Stdio::piped());
        }
        let mut child = command.spawn().map_err(|error| {
            TtsError::Provider(format!(
                "command not found or could not start: {}: {error}",
                argv[0]
            ))
        })?;
        if self.text_delivery == "stdin" {
            child
                .stdin
                .take()
                .ok_or_else(|| TtsError::Provider("command stdin unavailable".to_owned()))?
                .write_all(request.text().as_bytes())
                .map_err(|error| TtsError::Provider(format!("could not write stdin: {error}")))?;
        }
        let result = child
            .wait_with_output()
            .map_err(|error| TtsError::Provider(format!("could not wait for command: {error}")))?;
        if !result.status.success() {
            let _ = std::fs::remove_file(output);
            return Err(TtsError::Provider(format!(
                "command failed ({}): {}",
                result.status,
                String::from_utf8_lossy(&result.stderr).trim()
            )));
        }
        let byte_length = std::fs::metadata(output)
            .map(|value| value.len())
            .unwrap_or(0);
        if byte_length == 0 {
            let _ = std::fs::remove_file(output);
            return Err(TtsError::Provider(format!(
                "command succeeded but wrote no audio at {}",
                output.display()
            )));
        }
        TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            self.capability.provider_id(),
            byte_length,
        )
    }
}

fn split_command_template(template: &str) -> Result<Vec<String>, TtsError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut in_token = false;
    for character in template.chars() {
        if let Some(expected) = quote {
            if character == expected {
                quote = None;
            } else {
                current.push(character);
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
            in_token = true;
        } else if matches!(character, ' ' | '\t') {
            if in_token {
                tokens.push(std::mem::take(&mut current));
                in_token = false;
            }
        } else {
            current.push(character);
            in_token = true;
        }
    }
    if let Some(quote) = quote {
        return Err(TtsError::InvalidCommandTemplate(format!(
            "unbalanced {quote} quote"
        )));
    }
    if in_token {
        tokens.push(current);
    }
    Ok(tokens)
}

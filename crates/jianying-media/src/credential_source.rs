use serde::{Deserialize, Serialize};

/// TTS 凭据引用的安全来源，不包含凭据值本身。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialSource {
    Environment,
    SecretProvider,
}

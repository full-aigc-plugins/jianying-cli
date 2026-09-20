use serde::{Deserialize, Serialize};

/// Provider 所需鉴权方式的领域声明。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TtsAuthScheme {
    None,
    ApiKey,
    BearerToken,
    OAuthToken,
    SignedRequest,
    SecretProvider,
}

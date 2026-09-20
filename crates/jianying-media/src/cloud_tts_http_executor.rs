use crate::{
    CloudTtsAdapter, CloudTtsExecutionError, CloudTtsOutput, CloudTtsProviderAdapter,
    CredentialSource, TtsArtifact, TtsRequest,
};
use chrono::{TimeZone, Utc};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::net::IpAddr;
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::{Host, Url};

type HmacSha256 = Hmac<Sha256>;

/// 审批完成后执行单次云端 TTS HTTP 提交，并原子保存音频制品。
pub struct CloudTtsHttpExecutor {
    endpoint_override: Option<String>,
    timeout: Duration,
}

impl CloudTtsHttpExecutor {
    /// 创建生产执行器，只使用 adapter 固定的官方 HTTPS endpoint。
    pub fn new(timeout: Duration) -> Self {
        Self {
            endpoint_override: None,
            timeout,
        }
    }

    /// 创建仅面向离线测试的回环 endpoint 执行器。
    ///
    /// 覆盖地址必须是无凭据的 `http://localhost` 或回环 IP，避免把测试开关变成任意 SSRF 通道。
    pub fn with_loopback_endpoint(
        endpoint: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, CloudTtsExecutionError> {
        let endpoint = endpoint.into();
        validate_loopback_endpoint(&endpoint)?;
        Ok(Self {
            endpoint_override: Some(endpoint),
            timeout,
        })
    }

    /// 在审批消费和任何网络 I/O 前验证请求、endpoint 与环境凭据形状。
    pub fn preflight(
        &self,
        adapter: &CloudTtsProviderAdapter,
        request: &TtsRequest,
    ) -> Result<(), CloudTtsExecutionError> {
        adapter
            .validate_request(request)
            .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
        let endpoint = self
            .endpoint_override
            .as_deref()
            .unwrap_or_else(|| adapter.endpoint());
        let parsed = Url::parse(endpoint).map_err(|error| {
            CloudTtsExecutionError::Definite(format!("invalid cloud TTS endpoint: {error}"))
        })?;
        if self.endpoint_override.is_none() && parsed.scheme() != "https" {
            return Err(CloudTtsExecutionError::Definite(
                "production cloud TTS endpoint must use HTTPS".to_owned(),
            ));
        }
        let credential = resolve_environment_credential(request, adapter.provider_id())?;
        if matches!(adapter, CloudTtsProviderAdapter::TencentCloud(_)) {
            parse_tencent_credential(&credential)?;
        }
        Ok(())
    }

    /// 读取凭据引用、执行一次 HTTP 请求并把音频原子落盘。
    pub fn execute(
        &self,
        adapter: &CloudTtsProviderAdapter,
        request: &TtsRequest,
        output: &Path,
    ) -> Result<TtsArtifact, CloudTtsExecutionError> {
        self.preflight(adapter, request)?;
        let credential = resolve_environment_credential(request, adapter.provider_id())?;
        let endpoint = self
            .endpoint_override
            .as_deref()
            .unwrap_or_else(|| adapter.endpoint());
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(self.timeout))
            .max_redirects(0)
            .build()
            .new_agent();

        let output_value = match adapter {
            CloudTtsProviderAdapter::Baidu(value) => {
                let mut form = value
                    .encode_form(request)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                form.insert("tok".to_owned(), credential);
                let mut response = agent
                    .post(endpoint)
                    .header("Accept", "audio/*")
                    .send_form(form)
                    .map_err(classify_send_error)?;
                let content_type = header(&response, "content-type").unwrap_or_default();
                let request_id = header(&response, "x-request-id");
                let bytes = read_response_bytes(&mut response)?;
                value
                    .decode_audio(&bytes, &content_type, request_id.as_deref())
                    .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?
            }
            CloudTtsProviderAdapter::ZhipuGlm(value) => {
                let body = value
                    .encode_request(request)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                let mut response = agent
                    .post(endpoint)
                    .header("Authorization", &format!("Bearer {credential}"))
                    .header("Accept", "audio/*")
                    .send_json(&body)
                    .map_err(classify_send_error)?;
                let content_type = header(&response, "content-type").unwrap_or_default();
                let request_id = header(&response, "x-request-id");
                let bytes = read_response_bytes(&mut response)?;
                value
                    .decode_audio(&bytes, &content_type, request_id.as_deref())
                    .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?
            }
            CloudTtsProviderAdapter::Volcengine(value) => {
                let body = value
                    .encode_request(request)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                let request_id = uuid::Uuid::new_v4().to_string();
                let mut response = agent
                    .post(endpoint)
                    .header("Content-Type", "application/json")
                    .header("Accept", "text/event-stream")
                    .header("X-Api-Key", &credential)
                    .header("X-Api-Resource-Id", value.resource_id())
                    .header("X-Api-Request-Id", &request_id)
                    .send_json(&body)
                    .map_err(classify_send_error)?;
                let response_request_id = header(&response, "x-api-request-id")
                    .or_else(|| header(&response, "x-tt-logid"))
                    .unwrap_or(request_id);
                let bytes = read_response_bytes(&mut response)?;
                value
                    .decode_stream_response(&bytes, Some(&response_request_id))
                    .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?
            }
            CloudTtsProviderAdapter::TencentCloud(value) => {
                let body = value
                    .encode_request(request)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                let body_bytes = serde_json::to_vec(&body)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                let url = Url::parse(endpoint).map_err(|error| {
                    CloudTtsExecutionError::Definite(format!("invalid Tencent endpoint: {error}"))
                })?;
                let host = url.host_str().ok_or_else(|| {
                    CloudTtsExecutionError::Definite(
                        "Tencent endpoint does not contain a host".to_owned(),
                    )
                })?;
                let (secret_id, secret_key) = parse_tencent_credential(&credential)?;
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let authorization =
                    tencent_authorization(&secret_id, &secret_key, host, &body_bytes, timestamp)?;
                let mut response = agent
                    .post(endpoint)
                    .header("Authorization", &authorization)
                    .header("Content-Type", "application/json; charset=utf-8")
                    .header("Host", host)
                    .header("X-TC-Action", "TextToVoice")
                    .header("X-TC-Timestamp", &timestamp.to_string())
                    .header("X-TC-Version", "2019-08-23")
                    .send(&body_bytes)
                    .map_err(classify_send_error)?;
                let bytes = read_response_bytes(&mut response)?;
                let response_json = parse_json_response(&bytes)?;
                value
                    .decode_response(&response_json)
                    .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?
            }
            _ => {
                let body = adapter
                    .encode_json(request)
                    .map_err(|error| CloudTtsExecutionError::Definite(error.to_string()))?;
                let authorization = format!("Bearer {credential}");
                let mut response = agent
                    .post(endpoint)
                    .header("Authorization", &authorization)
                    .header("Content-Type", "application/json")
                    .send_json(&body)
                    .map_err(classify_send_error)?;
                let bytes = read_response_bytes(&mut response)?;
                let response_json = parse_json_response(&bytes)?;
                match adapter {
                    CloudTtsProviderAdapter::XiaomiMimo(value) => {
                        value.decode_response(&response_json)
                    }
                    CloudTtsProviderAdapter::AliyunBailian(value) => {
                        if self.endpoint_override.is_some() {
                            decode_aliyun_loopback_fixture(&response_json)
                        } else {
                            value.decode_response(&response_json)
                        }
                    }
                    CloudTtsProviderAdapter::MiniMax(value) => {
                        value.decode_response(&response_json)
                    }
                    _ => unreachable!("special response modes handled above"),
                }
                .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?
            }
        };

        let (bytes, request_id) = match output_value {
            CloudTtsOutput::Inline { bytes, request_id } => (bytes, request_id),
            CloudTtsOutput::RemoteUrl {
                url, request_id, ..
            } => (self.download_remote_audio(&agent, &url)?, request_id),
        };
        atomic_write(output, &bytes)?;
        let mut artifact = TtsArtifact::new(
            output.to_path_buf(),
            request.format(),
            adapter.provider_id(),
            bytes.len() as u64,
        )
        .map_err(|error| CloudTtsExecutionError::Ambiguous(error.to_string()))?;
        if let Some(request_id) = request_id {
            artifact = artifact.with_request_id(request_id);
        }
        Ok(artifact)
    }

    fn download_remote_audio(
        &self,
        agent: &ureq::Agent,
        audio_url: &str,
    ) -> Result<Vec<u8>, CloudTtsExecutionError> {
        let parsed = Url::parse(audio_url).map_err(|error| {
            CloudTtsExecutionError::Ambiguous(format!("invalid provider audio URL: {error}"))
        })?;
        let permitted = parsed.scheme() == "https"
            || (self.endpoint_override.is_some()
                && parsed.scheme() == "http"
                && is_loopback(parsed.host()));
        if !permitted || !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(CloudTtsExecutionError::Ambiguous(
                "provider audio URL violates the HTTPS/loopback policy".to_owned(),
            ));
        }
        let mut response = agent.get(audio_url).call().map_err(classify_send_error)?;
        read_response_bytes(&mut response)
    }
}

fn resolve_environment_credential(
    request: &TtsRequest,
    provider_id: &str,
) -> Result<String, CloudTtsExecutionError> {
    let credential = request.credential().ok_or_else(|| {
        CloudTtsExecutionError::Definite(format!(
            "provider {provider_id} requires a credential reference"
        ))
    })?;
    if credential.source() != CredentialSource::Environment {
        return Err(CloudTtsExecutionError::Definite(
            "only environment credential references are executable in this runtime".to_owned(),
        ));
    }
    std::env::var(credential.name())
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            CloudTtsExecutionError::Definite(format!(
                "credential environment variable {} is missing or empty",
                credential.name()
            ))
        })
}

fn classify_send_error(error: ureq::Error) -> CloudTtsExecutionError {
    match error {
        ureq::Error::StatusCode(code) if code < 500 => {
            CloudTtsExecutionError::Definite(format!("provider returned HTTP {code}"))
        }
        ureq::Error::Http(error) => CloudTtsExecutionError::Definite(error.to_string()),
        ureq::Error::BadUri(error) => CloudTtsExecutionError::Definite(error),
        other => CloudTtsExecutionError::Ambiguous(format!("HTTP transport failed: {other}")),
    }
}

fn read_response_bytes(
    response: &mut ureq::http::Response<ureq::Body>,
) -> Result<Vec<u8>, CloudTtsExecutionError> {
    const MAX_RESPONSE_BYTES: u64 = 64 * 1024 * 1024;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            CloudTtsExecutionError::Ambiguous(format!("could not read provider response: {error}"))
        })?;
    if bytes.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(CloudTtsExecutionError::Ambiguous(
            "provider response exceeded 64 MiB safety limit".to_owned(),
        ));
    }
    Ok(bytes)
}

fn parse_json_response(bytes: &[u8]) -> Result<serde_json::Value, CloudTtsExecutionError> {
    serde_json::from_slice(bytes).map_err(|error| {
        CloudTtsExecutionError::Ambiguous(format!("provider returned invalid JSON: {error}"))
    })
}

fn decode_aliyun_loopback_fixture(
    response: &serde_json::Value,
) -> Result<CloudTtsOutput, crate::TtsError> {
    let url = response
        .pointer("/output/audio/url")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| crate::TtsError::InvalidProviderResponse {
            provider: "aliyun-bailian".to_owned(),
            reason: "missing output.audio.url".to_owned(),
        })?;
    let parsed = Url::parse(url).map_err(|error| crate::TtsError::InvalidProviderResponse {
        provider: "aliyun-bailian".to_owned(),
        reason: format!("invalid loopback audio URL: {error}"),
    })?;
    if parsed.scheme() != "http" || !is_loopback(parsed.host()) {
        return Err(crate::TtsError::InvalidProviderResponse {
            provider: "aliyun-bailian".to_owned(),
            reason: "offline fixture audio URL must use a loopback HTTP endpoint".to_owned(),
        });
    }
    Ok(CloudTtsOutput::RemoteUrl {
        url: url.to_owned(),
        request_id: response
            .get("request_id")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        expires_at: response
            .pointer("/output/audio/expires_at")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

fn atomic_write(output: &Path, bytes: &[u8]) -> Result<(), CloudTtsExecutionError> {
    if bytes.is_empty() {
        return Err(CloudTtsExecutionError::Ambiguous(
            "provider returned empty audio".to_owned(),
        ));
    }
    let parent = output.parent().ok_or_else(|| {
        CloudTtsExecutionError::Ambiguous("audio output has no parent directory".to_owned())
    })?;
    std::fs::create_dir_all(parent).map_err(|error| {
        CloudTtsExecutionError::Ambiguous(format!("could not create audio directory: {error}"))
    })?;
    let partial = output.with_extension(format!(
        "{}.partial-{}",
        output
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("audio"),
        std::process::id()
    ));
    std::fs::write(&partial, bytes).map_err(|error| {
        CloudTtsExecutionError::Ambiguous(format!("could not stage cloud audio: {error}"))
    })?;
    if let Err(error) = std::fs::rename(&partial, output) {
        let _ = std::fs::remove_file(&partial);
        return Err(CloudTtsExecutionError::Ambiguous(format!(
            "could not commit cloud audio: {error}"
        )));
    }
    Ok(())
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn validate_loopback_endpoint(endpoint: &str) -> Result<(), CloudTtsExecutionError> {
    let url = Url::parse(endpoint).map_err(|error| {
        CloudTtsExecutionError::Definite(format!("invalid mock endpoint: {error}"))
    })?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || !is_loopback(url.host())
    {
        return Err(CloudTtsExecutionError::Definite(
            "mock endpoint must be credential-free http://localhost or a loopback IP".to_owned(),
        ));
    }
    Ok(())
}

fn is_loopback(host: Option<Host<&str>>) -> bool {
    match host {
        Some(Host::Domain(value)) => value.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(value)) => IpAddr::V4(value).is_loopback(),
        Some(Host::Ipv6(value)) => IpAddr::V6(value).is_loopback(),
        None => false,
    }
}

fn parse_tencent_credential(credential: &str) -> Result<(String, String), CloudTtsExecutionError> {
    let value: serde_json::Value = serde_json::from_str(credential).map_err(|_| {
        CloudTtsExecutionError::Definite(
            "Tencent credential env must contain JSON with secret_id and secret_key".to_owned(),
        )
    })?;
    let secret_id = value
        .get("secret_id")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CloudTtsExecutionError::Definite(
                "Tencent credential JSON is missing secret_id".to_owned(),
            )
        })?;
    let secret_key = value
        .get("secret_key")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            CloudTtsExecutionError::Definite(
                "Tencent credential JSON is missing secret_key".to_owned(),
            )
        })?;
    Ok((secret_id.to_owned(), secret_key.to_owned()))
}

fn tencent_authorization(
    secret_id: &str,
    secret_key: &str,
    host: &str,
    body: &[u8],
    timestamp: u64,
) -> Result<String, CloudTtsExecutionError> {
    let date = Utc
        .timestamp_opt(timestamp as i64, 0)
        .single()
        .ok_or_else(|| CloudTtsExecutionError::Definite("invalid signing timestamp".to_owned()))?
        .format("%Y-%m-%d")
        .to_string();
    let payload_hash = sha256_hex(body);
    let canonical_headers = format!("content-type:application/json; charset=utf-8\nhost:{host}\n");
    let canonical_request =
        format!("POST\n/\n\n{canonical_headers}\ncontent-type;host\n{payload_hash}");
    let credential_scope = format!("{date}/tts/tc3_request");
    let string_to_sign = format!(
        "TC3-HMAC-SHA256\n{timestamp}\n{credential_scope}\n{}",
        sha256_hex(canonical_request.as_bytes())
    );
    let secret_date = hmac_sha256(format!("TC3{secret_key}").as_bytes(), date.as_bytes())?;
    let secret_service = hmac_sha256(&secret_date, b"tts")?;
    let secret_signing = hmac_sha256(&secret_service, b"tc3_request")?;
    let signature = hex_lower(&hmac_sha256(&secret_signing, string_to_sign.as_bytes())?);
    Ok(format!(
        "TC3-HMAC-SHA256 Credential={secret_id}/{credential_scope}, SignedHeaders=content-type;host, Signature={signature}"
    ))
}

fn hmac_sha256(key: &[u8], value: &[u8]) -> Result<Vec<u8>, CloudTtsExecutionError> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| {
        CloudTtsExecutionError::Definite("could not initialize Tencent signer".to_owned())
    })?;
    mac.update(value);
    Ok(mac.finalize().into_bytes().to_vec())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex_lower(&digest.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 0x0f) as usize] as char);
    }
    result
}

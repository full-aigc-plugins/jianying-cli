use crate::{AsrError, AsrOutputFormat, AsrTimestampGranularity};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::Path;

/// 与具体执行器无关、只保存源内容哈希的 ASR 请求。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrRequest {
    source_sha256: String,
    output_format: AsrOutputFormat,
    model: Option<String>,
    language: Option<String>,
    translate_to_english: bool,
    timestamps: BTreeSet<AsrTimestampGranularity>,
}

impl AsrRequest {
    /// 创建按源内容寻址的 ASR 请求。
    pub fn new(
        source_sha256: impl Into<String>,
        output_format: AsrOutputFormat,
    ) -> Result<Self, AsrError> {
        let source_sha256 = source_sha256.into();
        if source_sha256.len() != 64 || !source_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(AsrError::InvalidSourceDigest);
        }
        Ok(Self {
            source_sha256: source_sha256.to_ascii_lowercase(),
            output_format,
            model: None,
            language: None,
            translate_to_english: false,
            timestamps: BTreeSet::new(),
        })
    }

    /// 计算内存内容的 SHA-256。
    pub fn hash_bytes(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    /// 读取源文件并计算 SHA-256。
    pub fn hash_source(path: impl AsRef<Path>) -> Result<String, AsrError> {
        let path = path.as_ref();
        if !path.is_file() {
            return Err(AsrError::SourceMissing(path.to_path_buf()));
        }
        let bytes = std::fs::read(path).map_err(|error| AsrError::Io(error.to_string()))?;
        Ok(Self::hash_bytes(&bytes))
    }

    /// 设置执行器模型标识。
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// 设置源语言。
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// 设置是否翻译到英文。
    pub fn with_translate_to_english(mut self, value: bool) -> Self {
        self.translate_to_english = value;
        self
    }

    /// 设置详细 JSON 所需的时间戳粒度。
    pub fn with_timestamps<I>(mut self, values: I) -> Self
    where
        I: IntoIterator<Item = AsrTimestampGranularity>,
    {
        self.timestamps = values.into_iter().collect();
        self
    }

    /// 返回源内容 SHA-256。
    pub fn source_sha256(&self) -> &str {
        &self.source_sha256
    }

    /// 返回产物格式。
    pub fn output_format(&self) -> AsrOutputFormat {
        self.output_format
    }

    /// 返回模型标识。
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// 返回语言标识。
    pub fn language(&self) -> Option<&str> {
        self.language.as_deref()
    }

    /// 是否请求翻译到英文。
    pub fn translate_to_english(&self) -> bool {
        self.translate_to_english
    }

    /// 返回请求的时间戳粒度。
    pub fn timestamps(&self) -> &BTreeSet<AsrTimestampGranularity> {
        &self.timestamps
    }
}

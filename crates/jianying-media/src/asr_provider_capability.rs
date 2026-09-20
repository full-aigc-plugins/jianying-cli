use crate::{AsrError, AsrOutputFormat, AsrRequest, AsrTimestampGranularity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// ASR Provider 的固定执行器身份、费用属性和格式能力。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AsrProviderCapability {
    provider_id: String,
    executor_identity: String,
    paid: bool,
    output_formats: BTreeSet<AsrOutputFormat>,
    timestamp_granularities: BTreeSet<AsrTimestampGranularity>,
}

impl AsrProviderCapability {
    /// 创建 Provider capability；执行器身份应包含版本或制品哈希。
    pub fn new<F, T>(
        provider_id: impl Into<String>,
        executor_identity: impl Into<String>,
        paid: bool,
        output_formats: F,
        timestamp_granularities: T,
    ) -> Result<Self, AsrError>
    where
        F: IntoIterator<Item = AsrOutputFormat>,
        T: IntoIterator<Item = AsrTimestampGranularity>,
    {
        let provider_id = provider_id.into();
        let executor_identity = executor_identity.into();
        let output_formats = output_formats.into_iter().collect::<BTreeSet<_>>();
        if provider_id.trim().is_empty() {
            return Err(AsrError::EmptyProviderId);
        }
        if executor_identity.trim().is_empty() {
            return Err(AsrError::EmptyExecutorIdentity);
        }
        if output_formats.is_empty() {
            return Err(AsrError::EmptyOutputFormats);
        }
        Ok(Self {
            provider_id,
            executor_identity,
            paid,
            output_formats,
            timestamp_granularities: timestamp_granularities.into_iter().collect(),
        })
    }

    /// 对请求格式和时间戳能力执行 fail-closed 校验。
    pub fn validate(&self, request: &AsrRequest) -> Result<(), AsrError> {
        if !self.output_formats.contains(&request.output_format()) {
            return Err(AsrError::UnsupportedOutputFormat(request.output_format()));
        }
        if !request.timestamps().is_empty()
            && request.output_format() != AsrOutputFormat::VerboseJson
        {
            return Err(AsrError::TimestampsRequireVerboseJson);
        }
        for granularity in request.timestamps() {
            if !self.timestamp_granularities.contains(granularity) {
                return Err(AsrError::UnsupportedTimestampGranularity(*granularity));
            }
        }
        Ok(())
    }

    /// 返回 Provider 标识。
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// 返回含版本或哈希的执行器身份。
    pub fn executor_identity(&self) -> &str {
        &self.executor_identity
    }

    /// 返回是否需要费用审批。
    pub fn paid(&self) -> bool {
        self.paid
    }
}

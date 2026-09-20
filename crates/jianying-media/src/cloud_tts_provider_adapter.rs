use crate::{
    AliyunBailianTtsAdapter, BaiduTtsAdapter, CloudTtsAdapter, MiniMaxTtsAdapter,
    TencentCloudTtsAdapter, TtsError, TtsRequest, VolcengineTtsAdapter, XiaomiMimoTtsAdapter,
    ZhipuGlmTtsAdapter,
};
use serde_json::Value;

/// 七个云端厂商的显式协议 adapter；各变体保留独立 wire contract。
#[derive(Debug)]
pub enum CloudTtsProviderAdapter {
    XiaomiMimo(XiaomiMimoTtsAdapter),
    Volcengine(VolcengineTtsAdapter),
    AliyunBailian(AliyunBailianTtsAdapter),
    Baidu(BaiduTtsAdapter),
    TencentCloud(TencentCloudTtsAdapter),
    MiniMax(MiniMaxTtsAdapter),
    ZhipuGlm(ZhipuGlmTtsAdapter),
}

impl CloudTtsProviderAdapter {
    /// 返回稳定 Provider ID。
    pub fn provider_id(&self) -> &str {
        match self {
            Self::XiaomiMimo(value) => value.capability().provider_id(),
            Self::Volcengine(value) => value.capability().provider_id(),
            Self::AliyunBailian(value) => value.capability().provider_id(),
            Self::Baidu(value) => value.capability().provider_id(),
            Self::TencentCloud(value) => value.capability().provider_id(),
            Self::MiniMax(value) => value.capability().provider_id(),
            Self::ZhipuGlm(value) => value.capability().provider_id(),
        }
    }

    /// 返回官方生产 endpoint。
    pub fn endpoint(&self) -> &str {
        match self {
            Self::XiaomiMimo(value) => value.endpoint(),
            Self::Volcengine(value) => value.endpoint(),
            Self::AliyunBailian(value) => value.endpoint(),
            Self::Baidu(value) => value.endpoint(),
            Self::TencentCloud(value) => value.endpoint(),
            Self::MiniMax(value) => value.endpoint(),
            Self::ZhipuGlm(value) => value.endpoint(),
        }
    }

    /// 在读取秘密和发送网络前完成统一请求的厂商能力校验。
    pub fn validate_request(&self, request: &TtsRequest) -> Result<(), TtsError> {
        match self {
            Self::Baidu(value) => value.encode_form(request).map(|_| ()),
            Self::ZhipuGlm(value) => value.encode_request(request).map(|_| ()),
            _ => self.encode_json(request).map(|_| ()),
        }
    }

    pub(crate) fn encode_json(&self, request: &TtsRequest) -> Result<Value, TtsError> {
        match self {
            Self::XiaomiMimo(value) => value.encode_request(request),
            Self::Volcengine(value) => value.encode_request(request),
            Self::AliyunBailian(value) => value.encode_request(request),
            Self::TencentCloud(value) => value.encode_request(request),
            Self::MiniMax(value) => value.encode_request(request),
            Self::Baidu(_) | Self::ZhipuGlm(_) => Err(TtsError::Provider(
                "selected provider does not use the shared JSON response path".to_owned(),
            )),
        }
    }
}

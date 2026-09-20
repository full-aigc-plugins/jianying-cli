use crate::{CloudTtsOutput, ProviderCapability, TtsError, TtsRequest};
use serde_json::Value;

/// 云端 TTS 的纯协议映射接口，不负责解析凭据或发起付费网络请求。
pub trait CloudTtsAdapter: Send + Sync {
    /// 返回该厂商 adapter 的能力声明。
    fn capability(&self) -> &ProviderCapability;

    /// 返回官方 API endpoint。
    fn endpoint(&self) -> &str;

    /// 将统一请求映射为厂商 JSON 请求体。
    fn encode_request(&self, request: &TtsRequest) -> Result<Value, TtsError>;

    /// 离线解码厂商 JSON 响应；实际网络提交由带审批和账本的执行器负责。
    fn decode_response(&self, response: &Value) -> Result<CloudTtsOutput, TtsError>;
}

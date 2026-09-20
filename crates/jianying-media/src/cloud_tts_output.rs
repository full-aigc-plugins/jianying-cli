/// 云端 adapter 从已验证响应中提取的音频引用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloudTtsOutput {
    Inline {
        bytes: Vec<u8>,
        request_id: Option<String>,
    },
    RemoteUrl {
        url: String,
        request_id: Option<String>,
        expires_at: Option<String>,
    },
}

impl CloudTtsOutput {
    /// 返回内联音频；URL 型响应返回 `None`。
    pub fn inline_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Inline { bytes, .. } => Some(bytes),
            Self::RemoteUrl { .. } => None,
        }
    }

    /// 返回远端音频 URL；内联响应返回 `None`。
    pub fn remote_url(&self) -> Option<&str> {
        match self {
            Self::Inline { .. } => None,
            Self::RemoteUrl { url, .. } => Some(url),
        }
    }

    /// 返回厂商请求 ID。
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Inline { request_id, .. } | Self::RemoteUrl { request_id, .. } => {
                request_id.as_deref()
            }
        }
    }
}

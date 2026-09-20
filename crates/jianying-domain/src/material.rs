use crate::MaterialId;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 项目素材；本地媒体和编辑器资源使用可区分的强类型变体。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Material {
    Video { id: MaterialId, path: PathBuf },
    Audio { id: MaterialId, path: PathBuf },
    Image { id: MaterialId, path: PathBuf },
    Font { id: MaterialId, path: PathBuf },
    EditorResource { id: MaterialId, resource_id: String },
}

impl Material {
    /// 创建视频素材。
    pub fn video(id: MaterialId, path: PathBuf) -> Self {
        Self::Video { id, path }
    }

    /// 创建音频素材。
    pub fn audio(id: MaterialId, path: PathBuf) -> Self {
        Self::Audio { id, path }
    }

    /// 创建图片素材。
    pub fn image(id: MaterialId, path: PathBuf) -> Self {
        Self::Image { id, path }
    }

    /// 返回素材标识。
    pub fn id(&self) -> &MaterialId {
        match self {
            Self::Video { id, .. }
            | Self::Audio { id, .. }
            | Self::Image { id, .. }
            | Self::Font { id, .. }
            | Self::EditorResource { id, .. } => id,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), crate::DomainError> {
        if self.id().as_str().trim().is_empty() {
            return Err(crate::DomainError::InvalidField {
                field: "material_id",
                reason: "must not be blank".to_owned(),
            });
        }
        match self {
            Self::Video { path, .. }
            | Self::Audio { path, .. }
            | Self::Image { path, .. }
            | Self::Font { path, .. }
                if path.as_os_str().is_empty() =>
            {
                Err(crate::DomainError::InvalidField {
                    field: "material_path",
                    reason: "must not be empty".to_owned(),
                })
            }
            Self::EditorResource { resource_id, .. } if resource_id.trim().is_empty() => {
                Err(crate::DomainError::InvalidField {
                    field: "resource_id",
                    reason: "must not be blank".to_owned(),
                })
            }
            _ => Ok(()),
        }
    }
}

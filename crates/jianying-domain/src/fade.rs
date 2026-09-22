use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 音视频淡入淡出时长。对应 pyJianYingDraft 的 AudioFade 与 VideoFade。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    in_us: i64,
    out_us: i64,
}

impl Fade {
    /// 创建非负淡入淡出设置。
    pub fn new(in_us: i64, out_us: i64) -> Result<Self, DomainError> {
        let fade = Self { in_us, out_us };
        fade.validate()?;
        Ok(fade)
    }

    /// 返回淡入时长。
    pub fn in_us(&self) -> i64 {
        self.in_us
    }

    /// 返回淡出时长。
    pub fn out_us(&self) -> i64 {
        self.out_us
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.in_us < 0 || self.out_us < 0 {
            return Err(DomainError::InvalidField {
                field: "fade",
                reason: "durations must be non-negative".to_owned(),
            });
        }
        Ok(())
    }
}

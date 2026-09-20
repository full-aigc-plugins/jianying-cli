use crate::DomainError;
use serde::{Deserialize, Serialize};

/// 时间线区间；公共单位固定为整数微秒，结束点不包含在区间内。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeRange {
    start_us: i64,
    duration_us: i64,
}

impl TimeRange {
    /// 创建非负起点、正时长且不溢出的时间区间。
    pub fn new(start_us: i64, duration_us: i64) -> Result<Self, DomainError> {
        if start_us < 0 {
            return Err(DomainError::InvalidField {
                field: "start_us",
                reason: "must be non-negative".to_owned(),
            });
        }
        if duration_us <= 0 {
            return Err(DomainError::InvalidField {
                field: "duration_us",
                reason: "must be positive".to_owned(),
            });
        }
        start_us
            .checked_add(duration_us)
            .ok_or(DomainError::TimeOverflow)?;
        Ok(Self {
            start_us,
            duration_us,
        })
    }

    /// 返回区间起点，单位为微秒。
    pub fn start_us(self) -> i64 {
        self.start_us
    }

    /// 返回区间时长，单位为微秒。
    pub fn duration_us(self) -> i64 {
        self.duration_us
    }

    /// 返回不包含在区间内的结束点。
    pub fn end_us(self) -> i64 {
        self.start_us + self.duration_us
    }
}

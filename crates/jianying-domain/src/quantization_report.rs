use crate::TimeRange;
use serde::{Deserialize, Serialize};

/// 帧网格量化结果，保留原始区间和两端漂移供审计与确认。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationReport {
    original: TimeRange,
    quantized: TimeRange,
    start_delta_us: i64,
    end_delta_us: i64,
}

impl QuantizationReport {
    pub(crate) fn new(
        original: TimeRange,
        quantized: TimeRange,
        start_delta_us: i64,
        end_delta_us: i64,
    ) -> Self {
        Self {
            original,
            quantized,
            start_delta_us,
            end_delta_us,
        }
    }

    /// 返回量化前区间。
    pub fn original(self) -> TimeRange {
        self.original
    }

    /// 返回量化后的帧对齐区间。
    pub fn quantized(self) -> TimeRange {
        self.quantized
    }

    /// 返回量化起点相对原起点的微秒差。
    pub fn start_delta_us(self) -> i64 {
        self.start_delta_us
    }

    /// 返回量化结束点相对原结束点的微秒差。
    pub fn end_delta_us(self) -> i64 {
        self.end_delta_us
    }
}

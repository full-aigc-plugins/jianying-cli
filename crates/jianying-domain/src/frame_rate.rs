use crate::{DomainError, QuantizationReport, TimeRange};
use serde::{Deserialize, Serialize};

/// 有理数帧率；例如 30/1 或 30000/1001。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRate {
    numerator: u32,
    denominator: u32,
}

impl FrameRate {
    /// 创建分子、分母均非零的帧率。
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, DomainError> {
        if numerator == 0 || denominator == 0 {
            return Err(DomainError::InvalidField {
                field: "frame_rate",
                reason: "numerator and denominator must be non-zero".to_owned(),
            });
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }

    /// 返回帧率分子。
    pub fn numerator(self) -> u32 {
        self.numerator
    }

    /// 返回帧率分母。
    pub fn denominator(self) -> u32 {
        self.denominator
    }

    /// 将区间向外量化到帧边界，并显式报告首尾漂移。
    ///
    /// 起点向下、结束点向上，从而不会静默删除原始内容。若任一端漂移
    /// 超过 `maximum_drift_us`，则拒绝量化。
    pub fn quantize_containing(
        self,
        source: TimeRange,
        maximum_drift_us: i64,
    ) -> Result<QuantizationReport, DomainError> {
        if maximum_drift_us < 0 {
            return Err(DomainError::InvalidField {
                field: "maximum_drift_us",
                reason: "must be non-negative".to_owned(),
            });
        }
        let units = 1_000_000_i128 * i128::from(self.denominator);
        let numerator = i128::from(self.numerator);
        let start_frame = i128::from(source.start_us()) * numerator / units;
        let scaled_end = i128::from(source.end_us()) * numerator;
        let end_frame = (scaled_end + units - 1) / units;
        let quantized_start = start_frame * units / numerator;
        let quantized_end = end_frame * units / numerator;
        let quantized_start =
            i64::try_from(quantized_start).map_err(|_| DomainError::TimeOverflow)?;
        let quantized_end = i64::try_from(quantized_end).map_err(|_| DomainError::TimeOverflow)?;
        let quantized = TimeRange::new(quantized_start, quantized_end - quantized_start)?;
        let start_delta_us = quantized.start_us() - source.start_us();
        let end_delta_us = quantized.end_us() - source.end_us();
        let actual_us = start_delta_us.abs().max(end_delta_us.abs());
        if actual_us > maximum_drift_us {
            return Err(DomainError::QuantizationDrift {
                actual_us,
                maximum_us: maximum_drift_us,
            });
        }
        Ok(QuantizationReport::new(
            source,
            quantized,
            start_delta_us,
            end_delta_us,
        ))
    }
}

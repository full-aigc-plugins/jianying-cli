use thiserror::Error;

/// 统一领域模型在任何文件写入前返回的校验错误。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DomainError {
    /// 字段值不满足领域约束。
    #[error("invalid {field}: {reason}")]
    InvalidField { field: &'static str, reason: String },
    /// 片段类型不能放入目标轨道。
    #[error("segment type {segment_type} is incompatible with track type {track_type}")]
    IncompatibleSegment {
        segment_type: &'static str,
        track_type: &'static str,
    },
    /// 同一轨道的两个片段发生重叠。
    #[error("segments overlap on track {track_id}: {previous_end_us} > {next_start_us}")]
    SegmentOverlap {
        track_id: String,
        previous_end_us: i64,
        next_start_us: i64,
    },
    /// 片段引用了项目素材表中不存在的素材。
    #[error("segment references missing material {0}")]
    MissingMaterial(String),
    /// 帧网格量化超出调用方允许的显式漂移范围。
    #[error("frame quantization drift {actual_us}us exceeds {maximum_us}us")]
    QuantizationDrift { actual_us: i64, maximum_us: i64 },
    /// 时间运算溢出。
    #[error("time range arithmetic overflow")]
    TimeOverflow,
}

//! 片段级编辑操作的共享语义。
//!
//! `jianying-cli` 的写入有两条路径：计划编译（`project build`）与命令编辑
//! （`timeline ...`）。历史上两条路径各自拼 JSON，导致同一语义写出不同 wire
//! （例如 `source_timerange.duration` 的舍入口径）。本模块把"要做什么"抽成
//! 与路径无关的操作，由 [`crate::writer`] 唯一落盘。

use anyhow::{bail, Result};

/// 单个片段级编辑操作。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EditOp {
    /// 设置播放速度；同时重算源区间长度并同步速度伴生素材。
    SetSpeed(f64),
    /// 设置片段音量。
    SetVolume(f64),
    /// 设置片段不透明度（`clip.alpha`）。
    SetOpacity(f64),
}

impl EditOp {
    /// 校验取值域；与 plan 侧使用同一套边界。
    pub fn validate(self) -> Result<()> {
        match self {
            EditOp::SetSpeed(speed) => {
                if !speed.is_finite() || speed <= 0.0 {
                    bail!("speed must be a positive finite number");
                }
            }
            EditOp::SetVolume(volume) => {
                if !volume.is_finite() || volume < 0.0 {
                    bail!("volume must be a non-negative finite number");
                }
            }
            EditOp::SetOpacity(alpha) => {
                if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
                    bail!("opacity must be between 0 and 1");
                }
            }
        }
        Ok(())
    }

    /// 操作的稳定名称，用于审计与错误上下文。
    pub fn name(self) -> &'static str {
        match self {
            EditOp::SetSpeed(_) => "speed",
            EditOp::SetVolume(_) => "volume",
            EditOp::SetOpacity(_) => "opacity",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::EditOp;

    #[test]
    fn rejects_out_of_range_values_with_the_plan_wording() {
        assert!(EditOp::SetSpeed(0.0).validate().is_err());
        assert!(EditOp::SetSpeed(f64::NAN).validate().is_err());
        assert!(EditOp::SetVolume(-1.0).validate().is_err());
        assert!(EditOp::SetOpacity(1.5).validate().is_err());
        assert!(EditOp::SetSpeed(0.1).validate().is_ok());
        assert!(EditOp::SetVolume(0.0).validate().is_ok());
        assert!(EditOp::SetOpacity(1.0).validate().is_ok());
    }
}

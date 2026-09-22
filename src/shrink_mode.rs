use anyhow::{bail, Result};

/// 新素材变短时的时间线处理方式。对应 pyJianYingDraft `ShrinkMode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrinkMode {
    CutHead,
    CutTail,
    CutTailAlign,
    Shrink,
}

impl ShrinkMode {
    /// 解析稳定的 snake_case 策略名。
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cut_head" => Ok(Self::CutHead),
            "cut_tail" => Ok(Self::CutTail),
            "cut_tail_align" => Ok(Self::CutTailAlign),
            "shrink" => Ok(Self::Shrink),
            _ => bail!("unsupported shrink mode: {value}"),
        }
    }

    /// 返回稳定的 snake_case 策略名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CutHead => "cut_head",
            Self::CutTail => "cut_tail",
            Self::CutTailAlign => "cut_tail_align",
            Self::Shrink => "shrink",
        }
    }
}

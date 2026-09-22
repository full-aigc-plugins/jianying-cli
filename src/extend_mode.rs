use anyhow::{bail, Result};

/// 新素材变长时的时间线处理方式。对应 pyJianYingDraft `ExtendMode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendMode {
    CutMaterialTail,
    ExtendHead,
    ExtendTail,
    PushTail,
}

impl ExtendMode {
    /// 解析稳定的 snake_case 策略名。
    pub fn parse(value: &str) -> Result<Self> {
        match value {
            "cut_material_tail" => Ok(Self::CutMaterialTail),
            "extend_head" => Ok(Self::ExtendHead),
            "extend_tail" => Ok(Self::ExtendTail),
            "push_tail" => Ok(Self::PushTail),
            _ => bail!("unsupported extend mode: {value}"),
        }
    }

    /// 返回稳定的 snake_case 策略名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CutMaterialTail => "cut_material_tail",
            Self::ExtendHead => "extend_head",
            Self::ExtendTail => "extend_tail",
            Self::PushTail => "push_tail",
        }
    }
}

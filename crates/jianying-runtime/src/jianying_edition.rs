use serde::{Deserialize, Serialize};
use std::fmt;

/// 剪映账号可用版型；安装包名称不参与该值的推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JianyingEdition {
    Unknown,
    Basic,
    Professional,
}

impl JianyingEdition {
    /// 判断当前版型是否满足任务要求。
    pub fn satisfies(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Basic, Self::Basic)
                | (Self::Professional, Self::Basic)
                | (Self::Professional, Self::Professional)
                | (_, Self::Unknown)
        )
    }
}

impl fmt::Display for JianyingEdition {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Unknown => "unknown",
            Self::Basic => "basic",
            Self::Professional => "professional",
        };
        formatter.write_str(value)
    }
}

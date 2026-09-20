use serde::Serialize;

/// 配置校验的机器可读报告。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub profile: String,
    pub issues: Vec<String>,
}

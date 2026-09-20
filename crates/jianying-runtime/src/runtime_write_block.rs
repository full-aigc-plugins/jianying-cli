/// 因编辑器仍在运行而拒绝写入时返回的结构化恢复指令。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWriteBlock {
    recovery_argv: Vec<String>,
}

impl RuntimeWriteBlock {
    pub(crate) fn editor_running(profile_id: &str) -> Self {
        Self {
            recovery_argv: vec![
                "jianying".to_string(),
                "runtime".to_string(),
                "stop".to_string(),
                "--profile".to_string(),
                profile_id.to_string(),
            ],
        }
    }

    /// 返回无需 shell 拼接即可执行的恢复参数向量。
    pub fn recovery_argv(&self) -> &[String] {
        &self.recovery_argv
    }
}

impl std::fmt::Display for RuntimeWriteBlock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "editor is running; stop it before writing")
    }
}

impl std::error::Error for RuntimeWriteBlock {}

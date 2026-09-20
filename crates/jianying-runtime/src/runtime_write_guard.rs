use crate::{RuntimeProbeReport, RuntimeProfile, RuntimeWriteBlock};

/// 草稿写入前的编辑器并发安全门禁。
pub struct RuntimeWriteGuard;

impl RuntimeWriteGuard {
    /// 仅允许已通过探测且编辑器未运行的写入。
    pub fn check(
        profile: &RuntimeProfile,
        report: &RuntimeProbeReport,
    ) -> Result<(), RuntimeWriteBlock> {
        if report.editor_running() {
            return Err(RuntimeWriteBlock::editor_running(profile.id()));
        }
        Ok(())
    }
}

use crate::{RuntimeControlStatus, RuntimeError, RuntimeProfile};

/// 本机编辑器生命周期控制接口。
pub trait RuntimeAdapter {
    /// 返回绑定的运行时档案。
    fn profile(&self) -> &RuntimeProfile;

    /// 启动档案对应的本机编辑器。
    fn start(&self) -> Result<RuntimeControlStatus, RuntimeError>;

    /// 停止由当前 adapter 实例启动并持有的编辑器进程。
    fn stop(&self) -> Result<RuntimeControlStatus, RuntimeError>;

    /// 查询由当前 adapter 实例持有的编辑器进程状态。
    fn status(&self) -> Result<RuntimeControlStatus, RuntimeError>;
}

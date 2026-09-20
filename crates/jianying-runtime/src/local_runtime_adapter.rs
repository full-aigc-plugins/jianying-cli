use crate::{
    RuntimeAdapter, RuntimeControlStatus, RuntimeError, RuntimeFileIdentity, RuntimeProfile,
};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// 使用结构化 argv 启动并仅控制自身子进程的本机运行时 adapter。
pub struct LocalRuntimeAdapter {
    profile: RuntimeProfile,
    executable: PathBuf,
    arguments: Vec<String>,
    child: Mutex<Option<Child>>,
}

impl LocalRuntimeAdapter {
    /// 创建 adapter，并立即校验绝对路径和档案文件身份。
    pub fn new<I, S>(
        profile: RuntimeProfile,
        executable: impl Into<PathBuf>,
        arguments: I,
    ) -> Result<Self, RuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let executable = executable.into();
        if !executable.is_absolute() {
            return Err(RuntimeError::ExecutableNotAbsolute(executable));
        }
        let expected = profile
            .file_identity()
            .ok_or(RuntimeError::MissingFileIdentity)?;
        let observed = RuntimeFileIdentity::from_path(&executable)?;
        if &observed != expected {
            return Err(RuntimeError::FileIdentityMismatch(executable));
        }
        Ok(Self {
            profile,
            executable,
            arguments: arguments.into_iter().map(Into::into).collect(),
            child: Mutex::new(None),
        })
    }
}

impl RuntimeAdapter for LocalRuntimeAdapter {
    fn profile(&self) -> &RuntimeProfile {
        &self.profile
    }

    fn start(&self) -> Result<RuntimeControlStatus, RuntimeError> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| RuntimeError::ProcessLockPoisoned)?;
        if let Some(child) = guard.as_mut() {
            if child
                .try_wait()
                .map_err(|error| RuntimeError::Process(error.to_string()))?
                .is_none()
            {
                return Err(RuntimeError::AlreadyRunning);
            }
        }
        let child = Command::new(&self.executable)
            .args(&self.arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| RuntimeError::Process(error.to_string()))?;
        *guard = Some(child);
        Ok(RuntimeControlStatus::Running)
    }

    fn stop(&self) -> Result<RuntimeControlStatus, RuntimeError> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| RuntimeError::ProcessLockPoisoned)?;
        let mut child = guard.take().ok_or(RuntimeError::NotRunning)?;
        if child
            .try_wait()
            .map_err(|error| RuntimeError::Process(error.to_string()))?
            .is_some()
        {
            return Err(RuntimeError::NotRunning);
        }
        child
            .kill()
            .map_err(|error| RuntimeError::Process(error.to_string()))?;
        child
            .wait()
            .map_err(|error| RuntimeError::Process(error.to_string()))?;
        Ok(RuntimeControlStatus::Stopped)
    }

    fn status(&self) -> Result<RuntimeControlStatus, RuntimeError> {
        let mut guard = self
            .child
            .lock()
            .map_err(|_| RuntimeError::ProcessLockPoisoned)?;
        let Some(child) = guard.as_mut() else {
            return Ok(RuntimeControlStatus::Stopped);
        };
        if child
            .try_wait()
            .map_err(|error| RuntimeError::Process(error.to_string()))?
            .is_some()
        {
            *guard = None;
            Ok(RuntimeControlStatus::Stopped)
        } else {
            Ok(RuntimeControlStatus::Running)
        }
    }
}

impl Drop for LocalRuntimeAdapter {
    fn drop(&mut self) {
        if let Ok(slot) = self.child.get_mut() {
            if let Some(child) = slot.as_mut() {
                if child.try_wait().ok().flatten().is_none() {
                    let _ = child.kill();
                    let _ = child.wait();
                }
            }
        }
    }
}

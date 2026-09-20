use crate::{
    OwnedRuntimeProcess, OwnedRuntimeRecord, RuntimeControlStatus, RuntimeError,
    RuntimeFileIdentity, RuntimeProfile,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, Signal, System};

/// 使用持久所有权记录跨 CLI 调用启动、查询和停止本机编辑器进程。
pub struct PersistentRuntimeController {
    root: PathBuf,
}

impl PersistentRuntimeController {
    /// 创建以指定目录保存所有权记录的控制器。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// 启动档案声明的精确可执行文件，并原子写入 PID、启动时间与文件哈希。
    pub fn start<I, S>(
        &self,
        profile: &RuntimeProfile,
        executable: &Path,
        arguments: I,
    ) -> Result<OwnedRuntimeProcess, RuntimeError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        if !profile.supports("editor.control") {
            return Err(RuntimeError::UnsupportedCapability(
                "editor.control".to_owned(),
            ));
        }
        if !executable.is_absolute() {
            return Err(RuntimeError::ExecutableNotAbsolute(
                executable.to_path_buf(),
            ));
        }
        if let Some(record) = self.load(profile)? {
            if self.verified_process(profile, &record)?.is_some() {
                return Err(RuntimeError::AlreadyRunning);
            }
            self.archive(profile, &record, "exited")?;
        }
        let expected = profile
            .file_identity()
            .ok_or(RuntimeError::MissingFileIdentity)?;
        let observed = RuntimeFileIdentity::from_path(executable)?;
        if &observed != expected {
            return Err(RuntimeError::FileIdentityMismatch(executable.to_path_buf()));
        }
        let arguments: Vec<String> = arguments.into_iter().map(Into::into).collect();
        let mut child = Command::new(executable)
            .args(&arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| RuntimeError::Process(error.to_string()))?;
        let pid = child.id();

        let mut system = System::new();
        let process_pid = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[process_pid]), true);
        let process = system
            .process(process_pid)
            .ok_or_else(|| RuntimeError::Process("started process disappeared".to_owned()))?;
        let record = OwnedRuntimeRecord::new(
            profile.id().to_owned(),
            pid,
            process.start_time(),
            executable.to_path_buf(),
            observed,
            arguments,
            now_epoch_seconds(),
        );
        if let Err(error) = self.save(profile, &record) {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
        drop(child);
        Ok(OwnedRuntimeProcess::new(
            RuntimeControlStatus::Running,
            pid,
            profile.id(),
            true,
        ))
    }

    /// 查询所有权记录对应的精确进程；PID 被复用时返回拒绝而不是误控其他进程。
    pub fn status(&self, profile: &RuntimeProfile) -> Result<OwnedRuntimeProcess, RuntimeError> {
        let Some(record) = self.load(profile)? else {
            return Ok(OwnedRuntimeProcess::new(
                RuntimeControlStatus::Stopped,
                0,
                profile.id(),
                false,
            ));
        };
        match self.verified_process(profile, &record)? {
            Some(_) => Ok(OwnedRuntimeProcess::new(
                RuntimeControlStatus::Running,
                record.pid(),
                profile.id(),
                true,
            )),
            None => Ok(OwnedRuntimeProcess::new(
                RuntimeControlStatus::Stopped,
                record.pid(),
                profile.id(),
                true,
            )),
        }
    }

    /// 仅停止所有权记录、启动时间和可执行身份均匹配的进程。
    pub fn stop(&self, profile: &RuntimeProfile) -> Result<OwnedRuntimeProcess, RuntimeError> {
        let record = self.load(profile)?.ok_or(RuntimeError::NotRunning)?;
        let Some((mut system, pid)) = self.verified_process(profile, &record)? else {
            self.archive(profile, &record, "exited")?;
            return Err(RuntimeError::NotRunning);
        };
        let process = system
            .process(pid)
            .ok_or(RuntimeError::OwnershipMismatch(record.pid()))?;
        let terminated = process.kill_with(Signal::Term).unwrap_or(false);
        if !terminated && !process.kill() {
            return Err(RuntimeError::Process(format!(
                "failed to signal owned pid {}",
                record.pid()
            )));
        }
        for _ in 0..100 {
            std::thread::sleep(Duration::from_millis(10));
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if system.process(pid).is_none_or(|process| {
                matches!(
                    process.status(),
                    ProcessStatus::Dead | ProcessStatus::Zombie
                )
            }) {
                self.archive(profile, &record, "stopped")?;
                return Ok(OwnedRuntimeProcess::new(
                    RuntimeControlStatus::Stopped,
                    record.pid(),
                    profile.id(),
                    true,
                ));
            }
        }
        if system.process(pid).is_some_and(|process| process.kill()) {
            // SIGKILL/TerminateProcess 接受后该进程不能继续执行；同进程测试中仍可能短暂呈现为
            // 等待父进程回收的僵尸，因此归档所有权记录而不按 PID 名称继续轮询或误杀。
            self.archive(profile, &record, "force-stopped")?;
            return Ok(OwnedRuntimeProcess::new(
                RuntimeControlStatus::Stopped,
                record.pid(),
                profile.id(),
                true,
            ));
        }
        Err(RuntimeError::Process(format!(
            "owned pid {} did not stop within the verification window",
            record.pid()
        )))
    }

    fn verified_process(
        &self,
        profile: &RuntimeProfile,
        record: &OwnedRuntimeRecord,
    ) -> Result<Option<(System, Pid)>, RuntimeError> {
        if record.profile_id() != profile.id()
            || profile.file_identity() != Some(record.executable_identity())
            || RuntimeFileIdentity::from_path(record.executable())? != *record.executable_identity()
        {
            return Err(RuntimeError::OwnershipMismatch(record.pid()));
        }
        let pid = Pid::from_u32(record.pid());
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
        let Some(process) = system.process(pid) else {
            return Ok(None);
        };
        if matches!(
            process.status(),
            ProcessStatus::Dead | ProcessStatus::Zombie
        ) {
            return Ok(None);
        }
        if process.start_time() != record.process_start_time() {
            return Err(RuntimeError::OwnershipMismatch(record.pid()));
        }
        let current_executable = process
            .exe()
            .ok_or(RuntimeError::OwnershipMismatch(record.pid()))?;
        if !same_path(current_executable, record.executable()) {
            return Err(RuntimeError::OwnershipMismatch(record.pid()));
        }
        Ok(Some((system, pid)))
    }

    fn load(&self, profile: &RuntimeProfile) -> Result<Option<OwnedRuntimeRecord>, RuntimeError> {
        let path = self.active_path(profile)?;
        if !path.is_file() {
            return Ok(None);
        }
        let bytes = std::fs::read(&path).map_err(|error| RuntimeError::Io {
            path: path.clone(),
            message: error.to_string(),
        })?;
        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|error| RuntimeError::InvalidOwnershipRecord(error.to_string()))
    }

    fn save(
        &self,
        profile: &RuntimeProfile,
        record: &OwnedRuntimeRecord,
    ) -> Result<(), RuntimeError> {
        std::fs::create_dir_all(&self.root).map_err(|error| RuntimeError::Io {
            path: self.root.clone(),
            message: error.to_string(),
        })?;
        let destination = self.active_path(profile)?;
        let temporary = destination.with_extension(format!("json.{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec_pretty(record)
            .map_err(|error| RuntimeError::InvalidOwnershipRecord(error.to_string()))?;
        std::fs::write(&temporary, bytes).map_err(|error| RuntimeError::Io {
            path: temporary.clone(),
            message: error.to_string(),
        })?;
        std::fs::rename(&temporary, &destination).map_err(|error| RuntimeError::Io {
            path: destination,
            message: error.to_string(),
        })
    }

    fn archive(
        &self,
        profile: &RuntimeProfile,
        record: &OwnedRuntimeRecord,
        reason: &str,
    ) -> Result<(), RuntimeError> {
        let source = self.active_path(profile)?;
        if !source.is_file() {
            return Ok(());
        }
        let destination = self.root.join(format!(
            "{}.{}.{}.{}.json",
            profile_key(profile.id()),
            record.created_at(),
            record.pid(),
            reason
        ));
        std::fs::rename(&source, &destination).map_err(|error| RuntimeError::Io {
            path: destination,
            message: error.to_string(),
        })
    }

    fn active_path(&self, profile: &RuntimeProfile) -> Result<PathBuf, RuntimeError> {
        if profile.id().len() > 256 || profile.id().contains('\0') {
            return Err(RuntimeError::InvalidProfileId(profile.id().to_owned()));
        }
        Ok(self
            .root
            .join(format!("{}.active.json", profile_key(profile.id()))))
    }
}

fn profile_key(profile_id: &str) -> String {
    format!("{:x}", Sha256::digest(profile_id.as_bytes()))
}

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

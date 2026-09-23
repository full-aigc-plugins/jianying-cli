use crate::{JobError, JobRecord};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

static INITIALIZED_DATABASES: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();

/// 使用 SQLite WAL 与 revision CAS 的生产级本地作业仓库。
pub struct SqliteJobStore {
    path: PathBuf,
}

impl SqliteJobStore {
    /// 打开指定数据库路径；表结构在首次操作时自动创建。
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// 保存新任务或以 revision CAS 更新已有任务。
    pub fn save(&self, record: &JobRecord) -> Result<(), JobError> {
        let mut connection = self.connect()?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(JobError::sqlite)?;
        let actual = transaction
            .query_row(
                "SELECT revision FROM jobs WHERE task_id = ?1",
                [&record.task_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .map_err(JobError::sqlite)?
            .map(|value| value as u64);
        let expected = record.revision.checked_sub(1);
        if actual.is_none() {
            if record.revision != 0 {
                return Err(JobError::RevisionConflict {
                    task_id: record.task_id.clone(),
                    expected,
                    actual,
                });
            }
            transaction
                .execute(
                    "INSERT INTO jobs(task_id, revision, record_json) VALUES (?1, ?2, ?3)",
                    params![
                        record.task_id,
                        record.revision as i64,
                        serde_json::to_string(record)?
                    ],
                )
                .map_err(JobError::sqlite)?;
        } else if actual != expected {
            return Err(JobError::RevisionConflict {
                task_id: record.task_id.clone(),
                expected,
                actual,
            });
        } else {
            let changed = transaction
                .execute(
                    "UPDATE jobs SET revision = ?2, record_json = ?3 \
                     WHERE task_id = ?1 AND revision = ?4",
                    params![
                        record.task_id,
                        record.revision as i64,
                        serde_json::to_string(record)?,
                        expected.unwrap_or_default() as i64
                    ],
                )
                .map_err(JobError::sqlite)?;
            if changed != 1 {
                return Err(JobError::RevisionConflict {
                    task_id: record.task_id.clone(),
                    expected,
                    actual,
                });
            }
        }
        transaction.commit().map_err(JobError::sqlite)
    }

    /// 按任务 ID 加载记录。
    pub fn load(&self, task_id: &str) -> Result<JobRecord, JobError> {
        let connection = self.connect()?;
        let encoded = connection
            .query_row(
                "SELECT record_json FROM jobs WHERE task_id = ?1",
                [task_id],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .map_err(JobError::sqlite)?
            .ok_or_else(|| JobError::NotFound(task_id.to_owned()))?;
        Ok(serde_json::from_str(&encoded)?)
    }

    /// 按任务 ID 稳定排序列出全部记录。
    pub fn list(&self) -> Result<Vec<JobRecord>, JobError> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT record_json FROM jobs ORDER BY task_id")
            .map_err(JobError::sqlite)?;
        let encoded = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(JobError::sqlite)?;
        let mut records = Vec::new();
        for value in encoded {
            records.push(serde_json::from_str(&value.map_err(JobError::sqlite)?)?);
        }
        Ok(records)
    }

    /// 返回 SQLite 数据库路径。
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn connect(&self) -> Result<Connection, JobError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(JobError::io)?;
        }
        let initialized = INITIALIZED_DATABASES.get_or_init(|| Mutex::new(HashSet::new()));
        let mut initialized = initialized
            .lock()
            .map_err(|_| JobError::Sqlite("database initialization lock poisoned".to_owned()))?;
        let connection = Connection::open(&self.path).map_err(JobError::sqlite)?;
        connection
            // Windows runner 与杀毒扫描下 8 个并发写入者可能排队超过 5 秒；
            // SQLite 自身有界等待，不能把可恢复锁竞争误判为任务失败。
            .busy_timeout(Duration::from_secs(30))
            .map_err(JobError::sqlite)?;
        if !initialized.contains(&self.path) {
            // WAL 切换需要数据库级锁；同一进程内只在首次连接时串行执行，避免并发首写互锁。
            connection
                .pragma_update(None, "journal_mode", "WAL")
                .map_err(JobError::sqlite)?;
            connection
                .execute_batch(
                    "CREATE TABLE IF NOT EXISTS jobs (
                        task_id TEXT PRIMARY KEY NOT NULL,
                        revision INTEGER NOT NULL,
                        record_json TEXT NOT NULL
                    );",
                )
                .map_err(JobError::sqlite)?;
            initialized.insert(self.path.clone());
        }
        Ok(connection)
    }
}

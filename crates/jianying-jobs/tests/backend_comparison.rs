use jianying_jobs::{JobError, JobRecord, JobState, JobStore, SqliteJobStore};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};
use std::time::Instant;

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jyc-backend-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn record(task_id: impl Into<String>) -> JobRecord {
    JobRecord::new(
        task_id.into(),
        PathBuf::from("/workspace/job.json"),
        Some(PathBuf::from("/workspace/out")),
    )
    .unwrap()
}

fn populate_json(root: &Path, count: usize) -> u128 {
    let started = Instant::now();
    let mut workers = Vec::new();
    for worker in 0..8 {
        let root = root.to_path_buf();
        workers.push(std::thread::spawn(move || {
            let store = JobStore::new(root);
            for index in (worker..count).step_by(8) {
                store.save(&record(format!("json-{index:04}"))).unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    started.elapsed().as_millis()
}

fn populate_sqlite(path: &Path, count: usize) -> u128 {
    let started = Instant::now();
    let mut workers = Vec::new();
    for worker in 0..8 {
        let path = path.to_path_buf();
        workers.push(std::thread::spawn(move || {
            let store = SqliteJobStore::new(path);
            for index in (worker..count).step_by(8) {
                store.save(&record(format!("sqlite-{index:04}"))).unwrap();
            }
        }));
    }
    for worker in workers {
        worker.join().unwrap();
    }
    started.elapsed().as_millis()
}

#[test]
fn sqlite_rejects_lost_updates_for_one_task() {
    let root = temp_root("revision-cas");
    let store = SqliteJobStore::new(root.join("jobs.sqlite3"));
    store.save(&record("shared-task")).unwrap();
    let mut left = store.load("shared-task").unwrap();
    let mut right = store.load("shared-task").unwrap();
    left.transition(JobState::Running, "left worker").unwrap();
    right.transition(JobState::Running, "right worker").unwrap();

    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for candidate in [left, right] {
        let path = root.join("jobs.sqlite3");
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            SqliteJobStore::new(path).save(&candidate)
        }));
    }
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Err(JobError::RevisionConflict { .. })))
            .count(),
        1
    );
    assert_eq!(store.load("shared-task").unwrap().revision, 1);
}

#[test]
fn sqlite_rolls_back_uncommitted_write_after_connection_loss() {
    let root = temp_root("crash-recovery");
    let path = root.join("jobs.sqlite3");
    let store = SqliteJobStore::new(&path);
    store.save(&record("committed")).unwrap();

    {
        let mut connection = Connection::open(&path).unwrap();
        let transaction = connection.transaction().unwrap();
        let uncommitted = serde_json::to_string(&record("uncommitted")).unwrap();
        transaction
            .execute(
                "INSERT INTO jobs(task_id, revision, record_json) VALUES (?1, 0, ?2)",
                ("uncommitted", uncommitted),
            )
            .unwrap();
        drop(transaction);
    }

    let reopened = SqliteJobStore::new(&path);
    assert_eq!(reopened.list().unwrap().len(), 1);
    assert!(matches!(
        reopened.load("uncommitted"),
        Err(JobError::NotFound(_))
    ));
    assert_eq!(reopened.load("committed").unwrap().task_id, "committed");
}

#[test]
fn json_and_sqlite_preserve_parallel_volume_without_record_loss() {
    const COUNT: usize = 256;
    let root = temp_root("parallel-volume");
    let json_root = root.join("json");
    let sqlite_path = root.join("jobs.sqlite3");
    let json_ms = populate_json(&json_root, COUNT);
    let sqlite_ms = populate_sqlite(&sqlite_path, COUNT);

    assert_eq!(JobStore::new(&json_root).list().unwrap().len(), COUNT);
    assert_eq!(
        SqliteJobStore::new(&sqlite_path).list().unwrap().len(),
        COUNT
    );
    eprintln!("backend comparison: records={COUNT} json_ms={json_ms} sqlite_ms={sqlite_ms}");

    std::fs::write(json_root.join("interrupted.json.crash.tmp"), b"{").unwrap();
    assert_eq!(JobStore::new(&json_root).list().unwrap().len(), COUNT);
}

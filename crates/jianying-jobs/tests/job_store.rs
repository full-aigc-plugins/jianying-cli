use jianying_jobs::{JobRecord, JobState, JobStore};

#[test]
fn journal_persists_transitions_cancel_and_retry_checkpoint() {
    let root = std::env::temp_dir().join(format!("jianying-job-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = JobStore::new(&root);
    let mut queued = JobRecord::new("task-cancel".to_owned(), "job.json".into(), None).unwrap();
    store.save(&queued).unwrap();
    queued
        .transition(JobState::Cancelled, "user cancelled")
        .unwrap();
    store.save(&queued).unwrap();
    assert_eq!(
        store.load("task-cancel").unwrap().state,
        JobState::Cancelled
    );

    let mut failed = JobRecord::new("task-retry".to_owned(), "job.json".into(), None).unwrap();
    failed.transition(JobState::Running, "started").unwrap();
    failed.transition(JobState::Failed, "interrupted").unwrap();
    failed.transition(JobState::Queued, "retry").unwrap();
    store.save(&failed).unwrap();
    let restored = store.load("task-retry").unwrap();
    assert_eq!(restored.state, JobState::Queued);
    assert_eq!(restored.attempts, 1);
    assert_eq!(restored.history.len(), 4);
}

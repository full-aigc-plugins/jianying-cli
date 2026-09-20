use jianying_jobs::{
    ApprovalRecord, ApprovalStore, AsrLedgerDecision, AsrLedgerError, AsrLedgerState,
    AsrLedgerStore, AsrSubmission,
};
use jianying_media::{AsrOutputFormat, AsrRequest};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Barrier};

fn roots(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("jianying-asr-ledger-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("draft")).unwrap();
    (root.clone(), root.join("approvals"), root.join("ledger"))
}

fn request(content: &[u8]) -> AsrRequest {
    AsrRequest::new(
        AsrRequest::hash_bytes(content),
        AsrOutputFormat::VerboseJson,
    )
    .unwrap()
    .with_model("whisper-large-v3")
    .with_language("zh")
}

fn submission(root: &Path, paid: bool) -> AsrSubmission {
    AsrSubmission::new(
        "fixture-whisper",
        "fixture-whisper@sha256:abc123",
        &request(b"same synthetic audio"),
        root.to_path_buf(),
        root.join("draft"),
        "jy-task-asr-1",
        paid,
        if paid { 50_000 } else { 0 },
    )
    .unwrap()
}

fn grant(approvals: &ApprovalStore, id: &str, submission: &AsrSubmission, now: u64) {
    approvals
        .save(
            &ApprovalRecord::grant(
                id.to_owned(),
                submission.approval_binding().unwrap(),
                300,
                now,
            )
            .unwrap(),
        )
        .unwrap();
}

#[test]
fn completed_result_is_reused_by_content_hash_and_executor_identity() {
    let (root, _, ledger_root) = roots("reuse");
    let ledger = AsrLedgerStore::new(ledger_root);
    let submission = submission(&root, false);
    let first = ledger.prepare_local(&submission, 100).unwrap();
    assert!(matches!(first, AsrLedgerDecision::Submit(_)));
    ledger
        .mark_running(submission.idempotency_key(), 101)
        .unwrap();
    let artifact = root.join("transcript.json");
    std::fs::write(&artifact, b"{\"text\":\"ok\"}").unwrap();
    ledger
        .mark_succeeded(submission.idempotency_key(), &artifact, None, 102)
        .unwrap();

    assert!(matches!(
        ledger.prepare_local(&submission, 103).unwrap(),
        AsrLedgerDecision::Reuse(_)
    ));
    assert_eq!(
        ledger
            .load(submission.idempotency_key())
            .unwrap()
            .attempts(),
        1
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn executor_or_request_change_produces_a_distinct_key_and_artifact_drift_blocks_reuse() {
    let (root, _, ledger_root) = roots("identity-drift");
    let ledger = AsrLedgerStore::new(ledger_root);
    let baseline = submission(&root, false);
    let changed_executor = AsrSubmission::new(
        "fixture-whisper",
        "fixture-whisper@sha256:def456",
        &request(b"same synthetic audio"),
        root.clone(),
        root.join("draft"),
        "jy-task-asr-2",
        false,
        0,
    )
    .unwrap();
    let changed_request = AsrSubmission::new(
        "fixture-whisper",
        "fixture-whisper@sha256:abc123",
        &request(b"same synthetic audio").with_language("en"),
        root.clone(),
        root.join("draft"),
        "jy-task-asr-3",
        false,
        0,
    )
    .unwrap();
    assert_ne!(
        baseline.idempotency_key(),
        changed_executor.idempotency_key()
    );
    assert_ne!(
        baseline.idempotency_key(),
        changed_request.idempotency_key()
    );

    ledger.prepare_local(&baseline, 100).unwrap();
    ledger
        .mark_running(baseline.idempotency_key(), 101)
        .unwrap();
    let artifact = root.join("transcript.json");
    std::fs::write(&artifact, b"{\"text\":\"ok\"}").unwrap();
    ledger
        .mark_succeeded(baseline.idempotency_key(), &artifact, None, 102)
        .unwrap();
    std::fs::write(&artifact, b"tampered").unwrap();
    assert!(matches!(
        ledger.prepare_local(&baseline, 103),
        Err(AsrLedgerError::ArtifactDrift { .. })
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paid_and_ambiguous_requests_never_retry_silently() {
    let (root, approval_root, ledger_root) = roots("paid-ambiguous");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = AsrLedgerStore::new(ledger_root);
    let submission = submission(&root, true);
    grant(&approvals, "asr-approval-1", &submission, 100);
    ledger
        .prepare_paid(&submission, &approvals, "asr-approval-1", 110)
        .unwrap();
    ledger
        .mark_running(submission.idempotency_key(), 111)
        .unwrap();
    ledger
        .mark_ambiguous(submission.idempotency_key(), 112)
        .unwrap();

    assert!(matches!(
        ledger.prepare_paid(&submission, &approvals, "asr-approval-1", 113),
        Err(AsrLedgerError::AmbiguousRequiresReconciliation { .. })
    ));
    assert!(matches!(
        ledger.retry_paid(&submission, &approvals, "missing", 114),
        Err(AsrLedgerError::AmbiguousRequiresReconciliation { .. })
    ));
    ledger
        .reconcile_ambiguous_as_failed(submission.idempotency_key(), 115)
        .unwrap();
    grant(&approvals, "asr-approval-2", &submission, 116);
    let retried = ledger
        .retry_paid(&submission, &approvals, "asr-approval-2", 117)
        .unwrap();
    assert_eq!(retried.state(), AsrLedgerState::Queued);
    assert_eq!(retried.approval_id(), Some("asr-approval-2"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn concurrent_prepare_allows_one_submission_and_stores_no_audio() {
    let (root, _, ledger_root) = roots("concurrent");
    let submission = submission(&root, false);
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _ in 0..2 {
        let ledger_root = ledger_root.clone();
        let submission = submission.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            barrier.wait();
            AsrLedgerStore::new(ledger_root).prepare_local(&submission, 120)
        }));
    }
    barrier.wait();
    let results = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, Ok(AsrLedgerDecision::Submit(_))))
            .count(),
        1
    );
    let persisted = std::fs::read_to_string(
        AsrLedgerStore::new(ledger_root).path(submission.idempotency_key()),
    )
    .unwrap();
    assert!(!persisted.contains("same synthetic audio"));
    std::fs::remove_dir_all(root).unwrap();
}

use jianying_jobs::{
    ApprovalRecord, ApprovalStore, TtsExecutionMode, TtsLedgerDecision, TtsLedgerError,
    TtsLedgerState, TtsLedgerStore, TtsSubmission,
};
use jianying_media::{CredentialRef, CredentialSource, TtsAudioFormat, TtsRequest};
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

fn request(text: &str) -> TtsRequest {
    TtsRequest::new(text, TtsAudioFormat::Mp3)
        .unwrap()
        .with_model("speech-2.8-hd")
        .with_voice("male-qn-qingse")
        .with_credential(
            CredentialRef::new(CredentialSource::Environment, "MINIMAX_API_KEY").unwrap(),
        )
}

fn submission(root: &std::path::Path, text: &str, mode: TtsExecutionMode) -> TtsSubmission {
    TtsSubmission::new(
        "minimax",
        &request(text),
        root.to_path_buf(),
        root.join("draft"),
        "jy-task-tts-1",
        50_000,
        mode,
    )
    .unwrap()
}

fn grant(approvals: &ApprovalStore, approval_id: &str, submission: &TtsSubmission, now: u64) {
    let record = ApprovalRecord::grant(
        approval_id.to_owned(),
        submission.approval_binding().unwrap(),
        300,
        now,
    )
    .unwrap();
    approvals.save(&record).unwrap();
}

fn roots(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    let root =
        std::env::temp_dir().join(format!("jianying-tts-ledger-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let approvals = root.join("approvals");
    let ledger = root.join("ledger");
    std::fs::create_dir_all(root.join("draft")).unwrap();
    (root, approvals, ledger)
}

#[test]
fn first_paid_submission_consumes_exact_approval_and_duplicate_is_blocked() {
    let (root, approval_root, ledger_root) = roots("duplicate");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = TtsLedgerStore::new(ledger_root);
    let submission = submission(&root, "只提交一次", TtsExecutionMode::Production);
    grant(&approvals, "tts-approval-1", &submission, 100);

    let decision = ledger
        .prepare(&submission, &approvals, "tts-approval-1", 120)
        .unwrap();
    let record = match decision {
        TtsLedgerDecision::Submit(record) => record,
        other => panic!("首次请求应允许提交，实际为 {other:?}"),
    };
    assert_eq!(record.state(), TtsLedgerState::Queued);
    assert_eq!(record.attempts(), 0);
    assert_eq!(
        approvals.load("tts-approval-1").unwrap().consumed_at,
        Some(120)
    );

    assert!(matches!(
        ledger.prepare(&submission, &approvals, "tts-approval-1", 121),
        Err(TtsLedgerError::DuplicateInFlight { .. })
    ));
    let persisted = std::fs::read_to_string(ledger.path(submission.idempotency_key())).unwrap();
    assert!(!persisted.contains("只提交一次"));
    assert!(!persisted.contains("MINIMAX_API_KEY"));
}

#[test]
fn ambiguous_request_cannot_retry_until_reconciled_and_reapproved() {
    let (root, approval_root, ledger_root) = roots("ambiguous");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = TtsLedgerStore::new(ledger_root);
    let submission = submission(&root, "超时状态未知", TtsExecutionMode::Production);
    grant(&approvals, "tts-approval-1", &submission, 100);
    ledger
        .prepare(&submission, &approvals, "tts-approval-1", 110)
        .unwrap();
    ledger
        .mark_running(submission.idempotency_key(), 111)
        .unwrap();
    ledger
        .mark_ambiguous(submission.idempotency_key(), 112)
        .unwrap();

    assert!(matches!(
        ledger.prepare(&submission, &approvals, "tts-approval-1", 113),
        Err(TtsLedgerError::AmbiguousRequiresReconciliation { .. })
    ));
    assert!(matches!(
        ledger.retry_failed(&submission, &approvals, "missing", 114),
        Err(TtsLedgerError::AmbiguousRequiresReconciliation { .. })
    ));

    ledger
        .reconcile_ambiguous_as_failed(submission.idempotency_key(), 115)
        .unwrap();
    grant(&approvals, "tts-approval-2", &submission, 116);
    let retried = ledger
        .retry_failed(&submission, &approvals, "tts-approval-2", 117)
        .unwrap();
    assert_eq!(retried.state(), TtsLedgerState::Queued);
    assert_eq!(retried.attempts(), 1);
    assert_eq!(retried.approval_id(), "tts-approval-2");
}

#[test]
fn succeeded_artifact_is_reused_only_when_its_digest_still_matches() {
    let (root, approval_root, ledger_root) = roots("reuse");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = TtsLedgerStore::new(ledger_root);
    let submission = submission(&root, "复用结果", TtsExecutionMode::Production);
    grant(&approvals, "tts-approval-1", &submission, 100);
    ledger
        .prepare(&submission, &approvals, "tts-approval-1", 110)
        .unwrap();
    ledger
        .mark_running(submission.idempotency_key(), 111)
        .unwrap();
    let artifact = root.join("result.mp3");
    std::fs::write(&artifact, b"fixture-audio").unwrap();
    ledger
        .mark_succeeded(
            submission.idempotency_key(),
            &artifact,
            Some("remote-42"),
            112,
        )
        .unwrap();

    assert!(matches!(
        ledger
            .prepare(&submission, &approvals, "unused-after-success", 113)
            .unwrap(),
        TtsLedgerDecision::Reuse(_)
    ));
    std::fs::write(&artifact, b"tampered").unwrap();
    assert!(matches!(
        ledger.prepare(&submission, &approvals, "unused-after-success", 114),
        Err(TtsLedgerError::ArtifactDrift { .. })
    ));
}

#[test]
fn live_canary_requires_a_separately_bound_approval() {
    let (root, approval_root, ledger_root) = roots("canary");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = TtsLedgerStore::new(ledger_root);
    let production = submission(&root, "canary", TtsExecutionMode::Production);
    let canary = submission(&root, "canary", TtsExecutionMode::LiveCanary);
    grant(&approvals, "production-only", &production, 100);

    assert!(matches!(
        ledger.prepare(&canary, &approvals, "production-only", 110),
        Err(TtsLedgerError::Approval(_))
    ));
    grant(&approvals, "live-canary", &canary, 111);
    assert!(matches!(
        ledger
            .prepare(&canary, &approvals, "live-canary", 112)
            .unwrap(),
        TtsLedgerDecision::Submit(_)
    ));
}

#[test]
fn approval_is_bound_to_budget_provider_request_and_target() {
    let (root, approval_root, ledger_root) = roots("binding");
    let approvals = ApprovalStore::new(approval_root);
    let ledger = TtsLedgerStore::new(ledger_root);
    let approved = submission(&root, "绑定内容", TtsExecutionMode::Production);
    grant(&approvals, "exact", &approved, 100);
    let changed = TtsSubmission::new(
        "zhipu-glm",
        &request("绑定内容"),
        root.clone(),
        root.join("another-draft"),
        "jy-task-tts-1",
        50_001,
        TtsExecutionMode::Production,
    )
    .unwrap();
    assert!(matches!(
        ledger.prepare(&changed, &approvals, "exact", 110),
        Err(TtsLedgerError::Approval(_))
    ));
    assert!(!ledger.path(changed.idempotency_key()).exists());
}

#[test]
fn concurrent_prepare_allows_exactly_one_submit() {
    let (root, approval_root, ledger_root) = roots("concurrent");
    let submission = submission(&root, "并发只能提交一次", TtsExecutionMode::Production);
    let approvals = ApprovalStore::new(&approval_root);
    grant(&approvals, "approval-a", &submission, 100);
    grant(&approvals, "approval-b", &submission, 100);
    let barrier = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for approval_id in ["approval-a", "approval-b"] {
        let approval_root = approval_root.clone();
        let ledger_root = ledger_root.clone();
        let submission = submission.clone();
        let barrier = Arc::clone(&barrier);
        handles.push(std::thread::spawn(move || {
            let approvals = ApprovalStore::new(approval_root);
            let ledger = TtsLedgerStore::new(ledger_root);
            barrier.wait();
            ledger.prepare(&submission, &approvals, approval_id, 120)
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
            .filter(|result| matches!(result, Ok(TtsLedgerDecision::Submit(_))))
            .count(),
        1
    );
    assert_eq!(
        ["approval-a", "approval-b"]
            .iter()
            .filter(|id| approvals.load(id).unwrap().consumed_at.is_some())
            .count(),
        1
    );
}

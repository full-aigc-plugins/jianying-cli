use jianying_jobs::{
    ApprovalRecord, ApprovalStore, NativeExportError, NativeExportState, NativeExportStore,
    NativeExportSubmission,
};
use jianying_runtime::{RuntimeFileIdentity, RuntimePlatform, RuntimeProbeInput, RuntimeProfile};
use jianying_schema::{ExportKind, ExportRequest};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

fn roots(name: &str) -> (PathBuf, PathBuf, PathBuf) {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let root = std::env::temp_dir().join(format!(
        "jianying-native-export-{name}-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(root.join("draft")).unwrap();
    fs::write(root.join("draft/draft_content.json"), b"{\"tracks\":[]}").unwrap();
    fs::write(root.join("editor"), b"synthetic editor identity").unwrap();
    (root.clone(), root.join("approvals"), root.join("exports"))
}

fn make_submission(root: &Path, task_id: &str) -> NativeExportSubmission {
    let executable = root.join("editor");
    let draft = root.join("draft");
    let profile = RuntimeProfile::new(
        "synthetic-native-export",
        "Synthetic Editor",
        "1.0",
        RuntimePlatform::MacOs,
        ["render.native"],
    )
    .unwrap()
    .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap())
    .with_draft_roots([draft.clone()]);
    let report = profile
        .probe(
            RuntimeProbeInput::new(
                "Synthetic Editor",
                "1.0",
                RuntimePlatform::MacOs,
                executable,
                draft.clone(),
            )
            .with_requested_capabilities(["render.native"]),
        )
        .unwrap();
    NativeExportSubmission::new(
        ExportRequest::new(ExportKind::Native, root.join("native.mp4"), false).unwrap(),
        &profile,
        &report,
        draft,
        root.to_path_buf(),
        task_id,
    )
    .unwrap()
}

fn grant(approvals: &ApprovalStore, id: &str, submission: &NativeExportSubmission, now: u64) {
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
fn native_export_requires_exact_approval_and_verifies_result() {
    let (root, approval_root, store_root) = roots("success");
    let approvals = ApprovalStore::new(approval_root);
    let store = NativeExportStore::new(store_root);
    let submission = make_submission(&root, "native-task-1");
    let wrong_submission = make_submission(&root, "another-task");
    grant(&approvals, "wrong-approval", &wrong_submission, 99);
    assert!(store
        .prepare(&submission, &approvals, "wrong-approval", 100)
        .is_err());
    assert!(!store.path(submission.task_id()).exists());
    grant(&approvals, "native-approval-1", &submission, 100);

    let queued = store
        .prepare(&submission, &approvals, "native-approval-1", 110)
        .unwrap();
    assert_eq!(queued.state(), NativeExportState::Queued);
    assert_eq!(queued.progress_percent(), 0);
    store.mark_running(submission.task_id(), 111).unwrap();
    store
        .update_progress(submission.task_id(), 40, 112)
        .unwrap();
    assert!(matches!(
        store.update_progress(submission.task_id(), 20, 113),
        Err(NativeExportError::ProgressRegression { .. })
    ));
    store.begin_verification(submission.task_id(), 114).unwrap();
    fs::write(submission.output(), b"synthetic native export bytes").unwrap();
    let succeeded = store.mark_succeeded(submission.task_id(), 115).unwrap();
    assert_eq!(succeeded.state(), NativeExportState::Succeeded);
    assert_eq!(succeeded.progress_percent(), 100);
    let artifact = succeeded.artifact().unwrap();
    assert_eq!(artifact.export_kind(), ExportKind::Native);
    assert!(artifact.byte_length() > 0);
    assert_eq!(artifact.sha256().len(), 64);

    fs::write(submission.output(), b"tampered").unwrap();
    assert!(matches!(
        store.verify_result(submission.task_id()),
        Err(NativeExportError::ArtifactDrift { .. })
    ));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn interrupted_export_preserves_checkpoint_and_requires_new_approval_to_retry() {
    let (root, approval_root, store_root) = roots("interrupted");
    let approvals = ApprovalStore::new(approval_root);
    let store = NativeExportStore::new(store_root);
    let submission = make_submission(&root, "native-task-2");
    grant(&approvals, "native-approval-1", &submission, 100);
    store
        .prepare(&submission, &approvals, "native-approval-1", 110)
        .unwrap();
    store.mark_running(submission.task_id(), 111).unwrap();
    store
        .update_progress(submission.task_id(), 65, 112)
        .unwrap();
    let interrupted = store
        .mark_interrupted(submission.task_id(), "editor closed", 113)
        .unwrap();
    assert_eq!(interrupted.state(), NativeExportState::Interrupted);
    assert_eq!(interrupted.progress_percent(), 65);
    assert_eq!(
        interrupted.recovery_argv(),
        &["jianying", "render", "native-task", "show", "native-task-2"]
    );
    assert!(store
        .retry(&submission, &approvals, "missing", 114)
        .is_err());

    grant(&approvals, "native-approval-2", &submission, 115);
    let retried = store
        .retry(&submission, &approvals, "native-approval-2", 116)
        .unwrap();
    assert_eq!(retried.state(), NativeExportState::Queued);
    assert_eq!(retried.progress_percent(), 0);
    assert_eq!(retried.approval_id(), "native-approval-2");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn proxy_export_cannot_be_labeled_or_authorized_as_native() {
    let (root, _, _) = roots("proxy");
    let executable = root.join("editor");
    let draft = root.join("draft");
    let profile = RuntimeProfile::new(
        "synthetic-native-export",
        "Synthetic Editor",
        "1.0",
        RuntimePlatform::MacOs,
        ["render.native"],
    )
    .unwrap()
    .with_file_identity(RuntimeFileIdentity::from_path(&executable).unwrap())
    .with_draft_roots([draft.clone()]);
    let report = profile
        .probe(RuntimeProbeInput::new(
            "Synthetic Editor",
            "1.0",
            RuntimePlatform::MacOs,
            executable,
            draft.clone(),
        ))
        .unwrap();
    let result = NativeExportSubmission::new(
        ExportRequest::new(ExportKind::Proxy, root.join("proxy.mp4"), false).unwrap(),
        &profile,
        &report,
        draft,
        root.clone(),
        "proxy-task",
    );
    assert!(matches!(result, Err(NativeExportError::NotNativeExport)));
    fs::remove_dir_all(root).unwrap();
}

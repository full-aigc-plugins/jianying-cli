use jianying_jobs::{ApprovalBinding, ApprovalError, ApprovalRecord, ApprovalStore};

#[test]
fn approval_is_exactly_bound_expires_and_is_single_use() {
    let root = std::env::temp_dir().join(format!("jianying-approval-store-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let store = ApprovalStore::new(&root);
    let workspace = root.join("workspace");
    let draft = workspace.join("draft");
    let binding = ApprovalBinding::new(
        "project.publish".to_owned(),
        vec!["--force".to_owned(), "false".to_owned()],
        workspace.clone(),
        draft.clone(),
        "jy-task-1".to_owned(),
    )
    .unwrap();
    let record = ApprovalRecord::grant("approval-1".to_owned(), binding.clone(), 60, 100).unwrap();
    store.save(&record).unwrap();

    let mismatches = [
        ApprovalBinding::new(
            "project.delete".to_owned(),
            vec!["--force".to_owned(), "false".to_owned()],
            workspace.clone(),
            draft.clone(),
            "jy-task-1".to_owned(),
        )
        .unwrap(),
        ApprovalBinding::new(
            "project.publish".to_owned(),
            vec!["--force".to_owned(), "true".to_owned()],
            workspace.clone(),
            draft.clone(),
            "jy-task-1".to_owned(),
        )
        .unwrap(),
        ApprovalBinding::new(
            "project.publish".to_owned(),
            vec!["--force".to_owned(), "false".to_owned()],
            root.join("other-workspace"),
            draft.clone(),
            "jy-task-1".to_owned(),
        )
        .unwrap(),
        ApprovalBinding::new(
            "project.publish".to_owned(),
            vec!["--force".to_owned(), "false".to_owned()],
            workspace.clone(),
            workspace.join("other-draft"),
            "jy-task-1".to_owned(),
        )
        .unwrap(),
        ApprovalBinding::new(
            "project.publish".to_owned(),
            vec!["--force".to_owned(), "false".to_owned()],
            workspace,
            draft,
            "jy-task-2".to_owned(),
        )
        .unwrap(),
    ];
    for mismatch in mismatches {
        assert!(matches!(
            store.consume("approval-1", &mismatch, 120),
            Err(ApprovalError::BindingMismatch)
        ));
    }

    let consumed = store.consume("approval-1", &binding, 120).unwrap();
    assert_eq!(consumed.consumed_at, Some(120));
    assert!(matches!(
        store.consume("approval-1", &binding, 121),
        Err(ApprovalError::AlreadyConsumed(_))
    ));

    let expired = ApprovalRecord::grant("approval-2".to_owned(), binding, 10, 100).unwrap();
    store.save(&expired).unwrap();
    assert!(matches!(
        store.consume("approval-2", &expired.binding, 111),
        Err(ApprovalError::Expired { .. })
    ));
}

use jianying_store::{MutationPhase, MutationPlan, StoreError};

fn fixture(tag: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("jianying-mutation-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let source = root.join("draft");
    let state = root.join("transactions");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("draft_content.json"), "original").unwrap();
    (source, state)
}

#[test]
fn failed_validation_keeps_source_and_writes_recoverable_audit() {
    let (source, state) = fixture("reject");
    let mut plan = MutationPlan::new(source.clone(), state).unwrap();
    plan.stage().unwrap();
    std::fs::write(plan.work_copy().join("draft_content.json"), "mutated").unwrap();
    let error = plan
        .validate(|_| Err(StoreError::Validation("synthetic rejection".to_owned())))
        .unwrap_err();
    assert!(matches!(error, StoreError::Validation(_)));
    assert_eq!(
        std::fs::read_to_string(source.join("draft_content.json")).unwrap(),
        "original"
    );
    assert_eq!(
        std::fs::read_to_string(plan.snapshot().join("draft_content.json")).unwrap(),
        "original"
    );
    assert_eq!(plan.phase(), MutationPhase::Rejected);
    let audit = std::fs::read_to_string(plan.audit_file()).unwrap();
    assert!(audit.contains("synthetic rejection"));
    assert!(audit.contains("restore"));
}

#[test]
fn validated_work_copy_commits_atomically_and_preserves_snapshot() {
    let (source, state) = fixture("commit");
    let mut plan = MutationPlan::new(source.clone(), state).unwrap();
    plan.stage().unwrap();
    std::fs::write(plan.work_copy().join("draft_content.json"), "committed").unwrap();
    plan.validate(|work| {
        let value = std::fs::read_to_string(work.join("draft_content.json"))?;
        if value != "committed" {
            return Err(StoreError::Validation("unexpected work copy".to_owned()));
        }
        Ok(())
    })
    .unwrap();
    plan.commit().unwrap();
    assert_eq!(
        std::fs::read_to_string(source.join("draft_content.json")).unwrap(),
        "committed"
    );
    assert_eq!(
        std::fs::read_to_string(plan.snapshot().join("draft_content.json")).unwrap(),
        "original"
    );
    assert_eq!(plan.phase(), MutationPhase::Committed);
    assert!(std::fs::read_to_string(plan.audit_file())
        .unwrap()
        .contains("atomic-commit"));
}

#[test]
fn snapshot_restore_uses_the_same_validated_atomic_commit_path() {
    let (source, state) = fixture("restore");
    let recovery = source.parent().unwrap().join("recovery");
    std::fs::create_dir_all(&recovery).unwrap();
    std::fs::write(recovery.join("draft_content.json"), "recovered").unwrap();

    let mut plan = MutationPlan::new(source.clone(), state).unwrap();
    plan.stage().unwrap();
    plan.replace_work_copy_from(&recovery).unwrap();
    plan.validate(|work| {
        let value = std::fs::read_to_string(work.join("draft_content.json"))?;
        if value != "recovered" {
            return Err(StoreError::Validation("restore mismatch".to_owned()));
        }
        Ok(())
    })
    .unwrap();
    plan.commit().unwrap();

    assert_eq!(
        std::fs::read_to_string(source.join("draft_content.json")).unwrap(),
        "recovered"
    );
    assert!(std::fs::read_to_string(plan.audit_file())
        .unwrap()
        .contains("restore-work-copy"));
}

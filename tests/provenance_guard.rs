use std::process::Command;

#[test]
fn repository_provenance_gate_accepts_the_checked_in_baseline() {
    let output = Command::new("python3")
        .args(["tools/check_provenance.py", "--root", "."])
        .output()
        .expect("python3 must be available for the provenance gate");
    assert!(
        output.status.success(),
        "provenance gate failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repository_provenance_gate_rejects_unknown_and_restricted_fixture_origins() {
    let output = Command::new("python3")
        .args([
            "tools/check_provenance.py",
            "--root",
            ".",
            "--self-test-negative-cases",
        ])
        .output()
        .expect("python3 must be available for the provenance gate");
    assert!(
        output.status.success(),
        "negative provenance cases were not rejected:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn parity_failures_emit_pointer_fixture_and_reproduction_argv() {
    let output = Command::new("python3")
        .args(["tools/parity_run.py", "--self-test-diagnostics"])
        .output()
        .expect("python3 must be available for the parity diagnostic self-test");
    assert!(
        output.status.success(),
        "parity diagnostic contract failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["path"], "/tracks/0/type");
    assert_eq!(report["fixture"], "tests/parity/scenarios/01-basic.json");
    assert!(report["reproduce"].as_array().unwrap().len() >= 6);
}

#[test]
fn pyjianyingdraft_completion_claim_tracks_partial_rows_and_open_tasks() {
    let parity: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string("provenance/PARITY_MATRIX.json").unwrap())
            .unwrap();
    assert_eq!(parity["source_claims"]["pyjianyingdraft"], "incomplete");
    let rows = parity["functions"]
        .as_array()
        .unwrap()
        .iter()
        .chain(parity["data_structures"].as_array().unwrap());
    let partial = rows
        .filter(|row| row["source"] == "pyjianyingdraft" && row["status"] == "partial")
        .collect::<Vec<_>>();
    assert_eq!(partial.len(), 4);
    for row in partial {
        assert!(row["gap"].as_str().is_some_and(|value| !value.is_empty()));
        assert!(!row["blocking_tasks"].as_array().unwrap().is_empty());
    }
}

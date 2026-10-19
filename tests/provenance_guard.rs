use std::process::{Command, Output};

/// 依次尝试常见解释器，返回第一个真正执行成功的结果。
///
/// CI 上只有 `python3`；Windows 上 `python3` 可能是 Microsoft Store 占位别名
/// （存在但一执行就失败，退出码 9009），而 `python` 往往是 `.cmd` 包装，
/// Rust 的 `Command` 无法直接执行。因此这里优先接受 `PYTHON` 环境变量，
/// 再按真实退出码逐个尝试，全部失败时返回最后一次输出供断言打印诊断。
fn run_python(args: &[&str]) -> Output {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(explicit) = std::env::var("PYTHON") {
        if !explicit.trim().is_empty() {
            candidates.push(explicit);
        }
    }
    for name in ["python3", "python", "py"] {
        candidates.push(name.to_string());
    }

    let mut last: Option<(String, Output)> = None;
    for candidate in candidates {
        match Command::new(&candidate).args(args).output() {
            Ok(output) if output.status.success() => return output,
            Ok(output) => {
                // Microsoft Store 的 `python3.exe` 是个占位别名：它能被启动，但立刻失败并打出
                // "Python was not found ... Microsoft Store"。继续试下一个候选，最后统一提示怎么修，
                // 否则测试只会打印一段无上下文的占位提示。
                last = Some((candidate, output));
            }
            Err(_) => continue,
        }
    }
    match last {
        Some((candidate, output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            assert!(
                !(stderr.contains("Microsoft Store") || output.status.code() == Some(9009)),
                "{candidate} is the Microsoft Store placeholder, not a real interpreter: the \
                 provenance gates shell out to Python. Set PYTHON to a real python.exe \
                 (e.g. `$env:PYTHON = (Get-Command python.cmd).Source`) and re-run.\nstderr:\n{stderr}"
            );
            output
        }
        None => panic!(
            "no python interpreter could be started (tried PYTHON, python3, python, py); the \
             provenance gates need one — set PYTHON to a real python.exe"
        ),
    }
}

#[test]
fn repository_provenance_gate_accepts_the_checked_in_baseline() {
    let output = run_python(&["tools/check_provenance.py", "--root", "."]);
    assert!(
        output.status.success(),
        "provenance gate failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn repository_provenance_gate_rejects_unknown_and_restricted_fixture_origins() {
    let output = run_python(&[
        "tools/check_provenance.py",
        "--root",
        ".",
        "--self-test-negative-cases",
    ]);
    assert!(
        output.status.success(),
        "negative provenance cases were not rejected:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn parity_failures_emit_pointer_fixture_and_reproduction_argv() {
    let output = run_python(&["tools/parity_run.py", "--self-test-diagnostics"]);
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

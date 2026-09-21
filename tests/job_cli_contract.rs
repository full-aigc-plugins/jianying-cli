use jianying_cli::domain_compat::job_from_v1_plan;
use jianying_cli::plan::Plan;
use jianying_schema::JobV2;
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
}

fn run(args: &[&str]) -> Output {
    bin().args(args).output().unwrap()
}

fn stdout_json(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "stdout is not one JSON document: {error}\n{}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn write_json(path: &Path, value: &impl serde::Serialize) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn temp_root(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jyc-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn v1_and_v2_job_create_have_equivalent_observable_draft_semantics() {
    let root = temp_root("job-equivalence");
    let media = root.join("a.mp4");
    let ffmpeg = match Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:d=1",
            "-y",
        ])
        .arg(&media)
        .output()
    {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!("ffmpeg unavailable — skipping");
            return;
        }
        Err(error) => panic!("failed to execute ffmpeg: {error}"),
    };
    if !ffmpeg.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }

    let plan_path = root.join("plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema": "jianying-cli-plan/v1",
            "name": "equivalent",
            "canvas": {"width": 640, "height": 360, "fps": 30},
            "tracks": [{"type": "video", "segments": [{
                "start_us": 0, "duration_us": 1_000_000, "source": "a.mp4"
            }]}]
        }),
    );
    let plan = Plan::load(&plan_path).unwrap();
    let job = job_from_v1_plan(&plan).unwrap();
    let job_path = root.join("job.json");
    write_json(&job_path, &job);

    let legacy_out = root.join("legacy");
    let job_out = root.join("job");
    let legacy = run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        legacy_out.to_str().unwrap(),
    ]);
    assert!(
        legacy.status.success(),
        "{}",
        String::from_utf8_lossy(&legacy.stderr)
    );

    let v2 = run(&[
        "job",
        "run",
        job_path.to_str().unwrap(),
        "--out",
        job_out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        v2.status.success(),
        "{}",
        String::from_utf8_lossy(&v2.stderr)
    );
    assert!(v2.stderr.is_empty());
    let envelope = stdout_json(&v2);
    assert_eq!(envelope["ok"], true);
    assert_eq!(envelope["data"]["operation"], "create");
    let task_id = envelope["data"]["task_id"].as_str().unwrap();
    let state_root = root.join(".jianying-jobs");
    assert!(state_root.join("jobs.sqlite3").is_file());
    let listed = stdout_json(&run(&[
        "job",
        "list",
        "--state-root",
        state_root.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(listed["data"][0]["task_id"], task_id);
    assert_eq!(listed["data"][0]["state"], "succeeded");
    let shown = stdout_json(&run(&[
        "job",
        "show",
        task_id,
        "--state-root",
        state_root.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(shown["data"]["attempts"], 1);
    let audit = stdout_json(&run(&[
        "job",
        "audit",
        task_id,
        "--state-root",
        state_root.to_str().unwrap(),
        "--json",
    ]));
    assert_eq!(audit["data"]["history"].as_array().unwrap().len(), 3);

    let legacy_inspect = stdout_json(&run(&["inspect", legacy_out.to_str().unwrap()]));
    let job_inspect = stdout_json(&run(&["inspect", job_out.to_str().unwrap()]));
    for field in ["name", "duration_us", "tracks", "segments", "canvas"] {
        assert_eq!(legacy_inspect[field], job_inspect[field], "field {field}");
    }
    assert_eq!(
        stdout_json(&run(&["verify", job_out.to_str().unwrap()]))["ok"],
        true
    );

    let source_timeline_path = job_out.join("draft_content.json");
    let source_timeline_before = std::fs::read(&source_timeline_path).unwrap();
    let source_timeline: Value = serde_json::from_slice(&source_timeline_before).unwrap();
    let segment_id = source_timeline["tracks"][0]["segments"][0]["id"]
        .as_str()
        .unwrap();
    let edit_job_path = root.join("media-edit.json");
    write_json(
        &edit_job_path,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"job","output":"job-edited"},
            "operations":[{"operation":"move_segment","segment_id":segment_id,
                "target":{"start_us":0,"duration_us":500_000}}]
        }),
    );
    let media_edit = run(&["job", "run", edit_job_path.to_str().unwrap(), "--json"]);
    assert!(
        media_edit.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&media_edit.stdout),
        String::from_utf8_lossy(&media_edit.stderr)
    );
    assert_eq!(
        std::fs::read(&source_timeline_path).unwrap(),
        source_timeline_before
    );
    let edited_out = root.join("job-edited");
    assert_eq!(
        stdout_json(&run(&["verify", edited_out.to_str().unwrap()]))["ok"],
        true
    );
    let edited_timeline: Value =
        serde_json::from_slice(&std::fs::read(edited_out.join("draft_content.json")).unwrap())
            .unwrap();
    assert_eq!(
        edited_timeline["tracks"][0]["segments"][0]["target_timerange"]["duration"],
        500_000
    );
    assert!(edited_timeline["materials"]["videos"][0]["path"]
        .as_str()
        .unwrap()
        .contains("job-edited"));
}

#[test]
fn json_mode_rejects_unknown_schema_with_a_structured_failure() {
    let root = temp_root("unknown-schema");
    let job_path = root.join("job.json");
    write_json(
        &job_path,
        &json!({"schema":"jianying-job/v99","operation":"inspect","project":{
            "type":"existing","source":"draft"
        }}),
    );
    let output = run(&["job", "run", job_path.to_str().unwrap(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["ok"], false);
    assert_eq!(envelope["error"]["type"], "unsupported_schema");
    assert_eq!(envelope["error"]["details"]["actual"], "jianying-job/v99");
    let task_id = envelope["error"]["details"]["task_id"].as_str().unwrap();
    let retry_state_root = root.join(".jianying-jobs");
    let retry = run(&[
        "job",
        "retry",
        task_id,
        "--state-root",
        retry_state_root.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(retry.status.code(), Some(1));
    let retry_envelope = stdout_json(&retry);
    assert_eq!(retry_envelope["error"]["details"]["task_id"], task_id);
    assert!(envelope["error"]["recovery"].is_array());
}

#[test]
fn json_mode_rejects_edit_without_operations_before_copying() {
    let root = temp_root("empty-edit");
    let job_path = root.join("job.json");
    write_json(
        &job_path,
        &json!({"schema":"jianying-job/v2","operation":"edit","project":{
            "type":"existing","source":"source","output":"copy"
        }}),
    );
    let output = run(&["job", "run", job_path.to_str().unwrap(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["error"]["type"], "invalid_job");
    assert!(!root.join("copy").exists());
}

#[test]
fn job_edit_rejects_opaque_metadata_before_copying_without_leaking_content() {
    let root = temp_root("opaque-metadata");
    let source = root.join("source");
    std::fs::create_dir(&source).unwrap();
    let opaque = b"independent-synthetic-opaque-metadata";
    std::fs::write(source.join("draft_meta_info.json"), opaque).unwrap();
    write_json(&source.join("draft_content.json"), &json!({"tracks":[]}));
    let before = jianying_runtime::DraftTreeSnapshot::capture(&source).unwrap();
    let job = root.join("edit.json");
    write_json(
        &job,
        &json!({
            "schema":"jianying-job/v2", "operation":"edit",
            "project":{"type":"existing","source":"source","output":"copy"},
            "operations":[{"operation":"replace_text","segment_id":"caption","text":"edited"}]
        }),
    );
    let result = run(&["--json", "job", "run", job.to_str().unwrap()]);
    assert!(!result.status.success());
    let envelope = stdout_json(&result);
    assert_eq!(envelope["error"]["type"], "unsupported_draft_encoding");
    assert_eq!(envelope["error"]["details"]["file"], "draft_meta_info.json");
    assert!(envelope["error"]["details"]["task_id"].is_string());
    assert!(!String::from_utf8_lossy(&result.stdout).contains("independent-synthetic"));
    assert!(!root.join("copy").exists());
    assert_eq!(
        before,
        jianying_runtime::DraftTreeSnapshot::capture(&source).unwrap()
    );
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("jianying-edit")));
}

#[test]
fn job_edit_applies_typed_operations_to_an_isolated_copy() {
    let root = temp_root("isolated-edit");
    let plan_path = root.join("plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema":"jianying-cli-plan/v1","name":"editable",
            "canvas":{"width":640,"height":360,"fps":30},
            "tracks":[{"type":"text","name":"字幕","segments":[
                {"start_us":0,"duration_us":1_000_000,"text":"保留源字幕"},
                {"start_us":2_000_000,"duration_us":1_000_000,"text":"待删除"}
            ]}]
        }),
    );
    let source = root.join("source");
    let build = run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        source.to_str().unwrap(),
    ]);
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let source_before = stdout_json(&run(&[
        "captions",
        "list",
        source.to_str().unwrap(),
        "--json",
    ]))["data"]
        .clone();
    let first_id = source_before[0]["segment_id"].as_str().unwrap();
    let second_id = source_before[1]["segment_id"].as_str().unwrap();
    let job_path = root.join("edit.json");
    write_json(
        &job_path,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"source","output":"edited"},
            "operations":[
                {"operation":"replace_text","segment_id":first_id,"text":"副本字幕"},
                {"operation":"move_segment","segment_id":first_id,
                    "target":{"start_us":1_500_000,"duration_us":2_000_000}},
                {"operation":"remove_segment","segment_id":second_id}
            ]
        }),
    );

    let edited = run(&["job", "run", job_path.to_str().unwrap(), "--json"]);
    assert!(
        edited.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&edited.stdout),
        String::from_utf8_lossy(&edited.stderr)
    );
    let result = stdout_json(&edited);
    assert_eq!(result["data"]["operation"], "edit");
    assert_eq!(
        result["data"]["operation_results"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(result["data"]["isolation"]["source_unchanged"], true);
    assert!(!result["data"]["isolation"]["changed_copy_files"]
        .as_array()
        .unwrap()
        .is_empty());

    let source_after = stdout_json(&run(&[
        "captions",
        "list",
        source.to_str().unwrap(),
        "--json",
    ]))["data"]
        .clone();
    assert_eq!(source_after, source_before);
    let output = root.join("edited");
    let output_captions = stdout_json(&run(&[
        "captions",
        "list",
        output.to_str().unwrap(),
        "--json",
    ]))["data"]
        .clone();
    assert_eq!(output_captions.as_array().unwrap().len(), 1);
    assert_eq!(output_captions[0]["text"], "副本字幕");
    assert_eq!(output_captions[0]["start_us"], 1_500_000);
    assert_eq!(output_captions[0]["duration_us"], 2_000_000);
    assert_eq!(
        stdout_json(&run(&["verify", output.to_str().unwrap()]))["ok"],
        true
    );

    let unsupported_path = root.join("unsupported-edit.json");
    write_json(
        &unsupported_path,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"source","output":"unsupported-copy"},
            "operations":[{"operation":"add_material","material":{
                "type":"editor_resource","id":"resource-1","resource_id":"builtin-1"
            }}]
        }),
    );
    let unsupported = run(&["job", "run", unsupported_path.to_str().unwrap(), "--json"]);
    assert_eq!(unsupported.status.code(), Some(1));
    let unsupported_error = stdout_json(&unsupported);
    assert_eq!(
        unsupported_error["error"]["type"],
        "incompatible_capability"
    );
    assert_eq!(
        unsupported_error["error"]["details"]["capability"],
        "job.edit.add_material"
    );
    assert!(!root.join("unsupported-copy").exists());
    assert_eq!(
        stdout_json(&run(&[
            "captions",
            "list",
            source.to_str().unwrap(),
            "--json",
        ]))["data"],
        source_before
    );
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("unsupported-copy.jianying-edit-")
    }));
}

#[test]
fn json_mode_rejects_a_breaking_compatibility_version() {
    let root = temp_root("breaking-compatibility");
    let plan: Plan = serde_json::from_value(json!({
        "schema": "jianying-cli-plan/v1",
        "name": "compatibility-version",
        "canvas": {"width": 640, "height": 360, "fps": 30},
        "tracks": [{"type":"text","segments":[{
            "start_us":0,"duration_us":1_000_000,"text":"hello"
        }]}]
    }))
    .unwrap();
    plan.validate().unwrap();
    let mut value = serde_json::to_value(job_from_v1_plan(&plan).unwrap()).unwrap();
    value["compatibility"]["schema"] = json!("jianying-cli-plan/v2");
    let job_path = root.join("job.json");
    write_json(&job_path, &value);

    let output = run(&["job", "run", job_path.to_str().unwrap(), "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let envelope = stdout_json(&output);
    assert_eq!(envelope["error"]["type"], "unsupported_schema");
    assert_eq!(
        envelope["error"]["details"]["actual"],
        "jianying-cli-plan/v2"
    );
}

#[test]
fn batch_commits_created_drafts_together_and_stdio_serve_reuses_run_handler() {
    let root = temp_root("batch-serve");
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"batch-text",
        "canvas":{"width":640,"height":360,"fps":30},
        "tracks":[{"type":"text","segments":[{"start_us":0,"duration_us":1000000,"text":"hello"}]}]
    }))
    .unwrap();
    let child_job = job_from_v1_plan(&plan).unwrap();
    let batch = JobV2::batch(vec![child_job.clone(), child_job.clone()]).unwrap();
    let batch_path = root.join("batch.json");
    write_json(&batch_path, &batch);
    let batch_out = root.join("batch-out");
    let output = run(&[
        "job",
        "run",
        batch_path.to_str().unwrap(),
        "--out",
        batch_out.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(stdout_json(&output)["data"]["count"], 2);
    assert!(batch_out.join("0000/draft_content.json").is_file());
    assert!(batch_out.join("0001/draft_content.json").is_file());

    let single_path = root.join("single.json");
    write_json(&single_path, &child_job);
    let serve_out = root.join("serve-out");
    let state_root = root.join("serve-state");
    let mut process = bin()
        .args(["job", "serve", "--state-root", state_root.to_str().unwrap()])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    writeln!(
        process.stdin.as_mut().unwrap(),
        "{}",
        json!({"job":single_path,"out":serve_out})
    )
    .unwrap();
    drop(process.stdin.take());
    let served = process.wait_with_output().unwrap();
    assert!(served.status.success());
    let response: Value = serde_json::from_slice(&served.stdout).unwrap();
    assert_eq!(response["ok"], true);
    assert!(response["data"]["task_id"].is_string());
}

#[test]
fn approval_cli_rejects_binding_drift_and_consumes_exact_match_once() {
    let root = temp_root("approval-binding");
    let approval_root = root.join("approvals");
    let cwd = root.join("workspace");
    let target = cwd.join("draft");
    std::fs::create_dir_all(&cwd).unwrap();

    let granted = run(&[
        "approvals",
        "grant",
        "--command",
        "project.publish",
        "--arg=--force",
        "--arg=false",
        "--cwd",
        cwd.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
        "--task-id",
        "jy-task-1",
        "--ttl-seconds",
        "60",
        "--state-root",
        approval_root.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        granted.status.success(),
        "{}",
        String::from_utf8_lossy(&granted.stderr)
    );
    let approval_id = stdout_json(&granted)["data"]["approval_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let drift = run(&[
        "approvals",
        "check",
        &approval_id,
        "--command",
        "project.publish",
        "--arg=--force",
        "--arg=true",
        "--cwd",
        cwd.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
        "--task-id",
        "jy-task-1",
        "--state-root",
        approval_root.to_str().unwrap(),
        "--json",
    ]);
    assert_eq!(drift.status.code(), Some(1));
    assert_eq!(stdout_json(&drift)["error"]["type"], "approval_denied");

    let exact_args = vec![
        "approvals",
        "check",
        approval_id.as_str(),
        "--command",
        "project.publish",
        "--arg=--force",
        "--arg=false",
        "--cwd",
        cwd.to_str().unwrap(),
        "--target",
        target.to_str().unwrap(),
        "--task-id",
        "jy-task-1",
        "--state-root",
        approval_root.to_str().unwrap(),
        "--json",
    ];
    let exact = run(&exact_args);
    assert!(exact.status.success());
    assert!(stdout_json(&exact)["data"]["consumed_at"].is_number());
    let replay = run(&exact_args);
    assert_eq!(replay.status.code(), Some(1));
    assert_eq!(stdout_json(&replay)["error"]["type"], "approval_denied");
}

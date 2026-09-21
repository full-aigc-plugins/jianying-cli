use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 1_000_000,
        width: 320,
        height: 240,
        has_video: true,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "jianying-render-batch-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let media = root.join("clip.mp4");
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:d=1",
            "-an",
            "-c:v",
            "libx264",
            &media.to_string_lossy(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"render-source",
        "canvas":{"width":320,"height":240,"fps":30},
        "tracks":[{"type":"video","segments":[{
            "start_us":0,"duration_us":1_000_000,"source":"clip.mp4"
        }]}]
    }))
    .unwrap();
    let draft_path = root.join("draft");
    draft::build(&plan, &root, &draft_path, None, &probe_stub).unwrap();
    (root, draft_path)
}

#[test]
fn proxy_batch_preflights_and_renders_every_manifest_job() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let (root, draft_path) = fixture();
    let manifest = root.join("batch.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec_pretty(&json!({
            "schema":"jianying-render-batch/v1",
            "jobs":[
                {"draft":draft_path,"output":"one.mp4"},
                {"draft":draft_path,"output":"nested/two.mp4"}
            ]
        }))
        .unwrap(),
    )
    .unwrap();
    let out = root.join("renders");
    let result = run_ok(&[
        "render",
        "batch",
        &manifest.to_string_lossy(),
        "--out-dir",
        &out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(result["succeeded"], 2);
    assert_eq!(result["failed"], 0);
    assert!(out.join("one.mp4").is_file());
    assert!(out.join("nested/two.mp4").is_file());

    let traversal = root.join("traversal.json");
    std::fs::write(
        &traversal,
        serde_json::to_vec(&json!({"schema":"jianying-render-batch/v1",
            "jobs":[{"draft":draft_path,"output":"../escape.mp4"}]}))
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        run(&[
            "render",
            "batch",
            &traversal.to_string_lossy(),
            "--out-dir",
            &out.to_string_lossy(),
            "--json",
        ])
        .status
        .code(),
        Some(1)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn job_batch_alias_executes_the_same_persistent_v2_handler() {
    let root = std::env::temp_dir().join(format!(
        "jianying-job-batch-alias-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":"batch-text",
        "canvas":{"width":320,"height":240,"fps":30},
        "tracks":[{"type":"text","segments":[{
            "start_us":0,"duration_us":1_000_000,"text":"batch"
        }]}]
    }))
    .unwrap();
    let child = jianying_cli::domain_compat::job_from_v1_plan(&plan).unwrap();
    let batch = jianying_cli::schema::JobV2::batch(vec![child.clone(), child]).unwrap();
    let job = root.join("job.json");
    std::fs::write(&job, serde_json::to_vec_pretty(&batch).unwrap()).unwrap();
    let out = root.join("drafts");
    let state = root.join("state");
    let result = run_ok(&[
        "job",
        "batch",
        &job.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--state-root",
        &state.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(result["operation"], "batch");
    assert_eq!(result["count"], 2);
    assert!(result["task_id"].as_str().is_some());
    assert!(out.join("0000/draft_content.json").is_file());
    assert!(out.join("0001/draft_content.json").is_file());
    let _ = std::fs::remove_dir_all(root);
}

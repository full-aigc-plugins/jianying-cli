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
fn job_create_with_relative_output_registers_assets_against_the_final_draft() {
    let root = temp_root("relative-job-output");
    let media = root.join("source.mp4");
    let generated = match Command::new("ffmpeg")
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
    if !generated.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }

    let job_path = root.join("job.json");
    write_json(
        &job_path,
        &json!({
            "schema":"jianying-job/v2","operation":"create",
            "project":{"type":"new","project":{
                "name":"relative-output","width":320,"height":240,
                "frame_rate":{"numerator":30,"denominator":1},
                "materials":[{"type":"video","id":"m1","path":media}],
                "timeline":{"tracks":[{"id":"v1","kind":"video","segments":[{
                    "type":"video","id":"s1","range":{"start_us":0,"duration_us":1_000_000},
                    "material_id":"m1","source_range":{"start_us":0,"duration_us":1_000_000},
                    "speed":1,"volume":1
                }]}]}
            }}
        }),
    );

    let output = bin()
        .current_dir(&root)
        .args([
            "job",
            "run",
            job_path.to_str().unwrap(),
            "--out",
            "draft",
            "--json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let draft = root.join("draft");
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(draft.join("draft_content.json")).unwrap()).unwrap();
    let registered = Path::new(timeline["materials"]["videos"][0]["path"].as_str().unwrap());
    assert!(registered.is_absolute());
    assert!(registered.starts_with(draft.canonicalize().unwrap()));
    assert!(registered.is_file());
}

#[test]
fn v1_compatibility_payload_cannot_override_the_converted_domain_project() {
    let root = temp_root("job-domain-create-authority");
    let plan_path = root.join("plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema": "jianying-cli-plan/v1",
            "name": "domain-authority",
            "canvas": {"width": 640, "height": 360, "fps": 30},
            "tracks": [{"type": "text", "segments": [{
                "start_us": 0,
                "duration_us": 1_000_000,
                "text": "领域模型必须决定生产输出"
            }]}]
        }),
    );
    let plan = Plan::load(&plan_path).unwrap();
    let mut job = serde_json::to_value(job_from_v1_plan(&plan).unwrap()).unwrap();

    // compatibility 只保留输入转换证据；即使其中的合法旧输入与领域项目冲突，
    // 生产 create 也必须以已经校验过的 DraftProject 为唯一执行事实源。
    job["compatibility"]["payload"]["name"] = json!("compatibility-must-not-execute");
    job["compatibility"]["payload"]["tracks"][0]["segments"][0]["text"] = json!("错误的旧执行路径");
    let job_path = root.join("job.json");
    write_json(&job_path, &job);

    let draft = root.join("draft");
    let created = run(&[
        "job",
        "run",
        job_path.to_str().unwrap(),
        "--out",
        draft.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        created.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&created.stdout),
        String::from_utf8_lossy(&created.stderr)
    );

    let inspected = stdout_json(&run(&["inspect", draft.to_str().unwrap()]));
    assert_eq!(inspected["name"], "domain-authority");
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(draft.join("draft_content.json")).unwrap()).unwrap();
    let content: Value = serde_json::from_str(
        timeline["materials"]["texts"][0]["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(content["text"], "领域模型必须决定生产输出");
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
fn job_source_range_controls_proxy_duration() {
    let root = temp_root("job-source-range-proxy");
    let media = root.join("source.mp4");
    let generated = match Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=320x240:d=4",
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
    if !generated.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }
    let job_path = root.join("job.json");
    write_json(
        &job_path,
        &json!({
            "schema":"jianying-job/v2","operation":"create",
            "project":{"type":"new","project":{
                "name":"trimmed-proxy","width":320,"height":240,
                "frame_rate":{"numerator":30,"denominator":1},
                "materials":[{"type":"video","id":"m1","path":media}],
                "timeline":{"tracks":[{"id":"v1","kind":"video","segments":[{
                    "type":"video","id":"s1","range":{"start_us":0,"duration_us":2_000_000},
                    "material_id":"m1","source_range":{"start_us":1_000_000,"duration_us":2_000_000},
                    "speed":1,"volume":1
                }]}]}
            }}
        }),
    );
    let draft = root.join("draft");
    let create = run(&[
        "job",
        "run",
        job_path.to_str().unwrap(),
        "--out",
        draft.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        create.status.success(),
        "{}",
        String::from_utf8_lossy(&create.stderr)
    );
    let preview = root.join("preview.mp4");
    let render = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        preview.to_str().unwrap(),
        "--json",
    ]);
    assert!(
        render.status.success(),
        "{}",
        String::from_utf8_lossy(&render.stderr)
    );
    let probe = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=nw=1:nk=1",
        ])
        .arg(&preview)
        .output()
        .expect("ffprobe must accompany ffmpeg in this integration test");
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let duration: f64 = String::from_utf8(probe.stdout)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(
        (1.9..=2.1).contains(&duration),
        "proxy duration {duration} ignored the Job source range"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn proxy_render_applies_the_registered_video_crop_before_canvas_scaling() {
    let root = temp_root("proxy-crop");
    let media = root.join("split.mp4");
    let generated = match Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=160x240:d=1",
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=160x240:d=1",
            "-filter_complex",
            "hstack=inputs=2",
            "-pix_fmt",
            "yuv420p",
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
    if !generated.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }

    let plan_path = root.join("plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema":"jianying-cli-plan/v1","name":"proxy-crop",
            "canvas":{"width":160,"height":240,"fps":30},
            "tracks":[{"type":"video","segments":[{
                "start_us":0,"duration_us":1_000_000,"source":media,
                "crop":{
                    "upper_left_x":0.5,"upper_left_y":0.0,
                    "upper_right_x":1.0,"upper_right_y":0.0,
                    "lower_left_x":0.5,"lower_left_y":1.0,
                    "lower_right_x":1.0,"lower_right_y":1.0
                }
            }]}]
        }),
    );
    let draft = root.join("draft");
    let built = run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        draft.to_str().unwrap(),
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );

    let preview = root.join("preview.mp4");
    let rendered = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        preview.to_str().unwrap(),
        "--scale",
        "1",
        "--crf",
        "18",
    ]);
    assert!(
        rendered.status.success(),
        "{}",
        String::from_utf8_lossy(&rendered.stderr)
    );

    let pixel = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", "0.2", "-i"])
        .arg(&preview)
        .args([
            "-vf",
            "crop=1:1:10:120,format=rgb24",
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        pixel.status.success(),
        "{}",
        String::from_utf8_lossy(&pixel.stderr)
    );
    assert!(pixel.stdout.len() >= 3, "ffmpeg returned no RGB sample");
    let [red, _green, blue] = [pixel.stdout[0], pixel.stdout[1], pixel.stdout[2]];
    assert!(
        blue > red.saturating_add(100) && blue > 120,
        "left output pixel was not sourced from the blue cropped half: rgb={:?}",
        &pixel.stdout[..3]
    );
}

#[test]
fn proxy_render_uses_distinct_cjk_glyphs_instead_of_missing_character_boxes() {
    let root = temp_root("proxy-cjk-font");
    let media = root.join("black.mp4");
    let generated = match Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=320x240:d=1",
            "-pix_fmt",
            "yuv420p",
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
    if !generated.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }

    let plan_path = root.join("plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema":"jianying-cli-plan/v1","name":"proxy-cjk-font",
            "canvas":{"width":320,"height":240,"fps":30},
            "tracks":[
                {"type":"video","segments":[{
                    "start_us":0,"duration_us":1_000_000,"source":media
                }]},
                {"type":"text","segments":[
                    {"start_us":0,"duration_us":500_000,"text":"园","size":80},
                    {"start_us":500_000,"duration_us":500_000,"text":"区","size":80}
                ]}
            ]
        }),
    );
    let draft = root.join("draft");
    let built = run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        draft.to_str().unwrap(),
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let preview = root.join("preview.mp4");
    let rendered = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        preview.to_str().unwrap(),
        "--scale",
        "1",
        "--crf",
        "18",
        "--burn-captions",
    ]);
    assert!(
        rendered.status.success(),
        "{}",
        String::from_utf8_lossy(&rendered.stderr)
    );

    let frame = |time: &str| {
        let output = Command::new("ffmpeg")
            .args(["-v", "error", "-ss", time, "-i"])
            .arg(&preview)
            .args([
                "-frames:v",
                "1",
                "-pix_fmt",
                "gray",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let first = frame("0.25");
    let second = frame("0.75");
    assert_eq!(first.len(), second.len());
    let different_foreground_pixels = first
        .iter()
        .zip(&second)
        .filter(|(left, right)| (**left >= 96) != (**right >= 96))
        .count();
    assert!(
        different_foreground_pixels > 20,
        "distinct Chinese characters rendered as the same missing-glyph box; differing pixels={different_foreground_pixels}"
    );
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
fn job_edit_adds_a_fully_typed_segment_and_executes_track_operations() {
    let root = temp_root("typed-add-segment");
    let media = root.join("overlay.mp4");
    let generated = match Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=320x240:d=2",
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
    if !generated.status.success() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }

    let plan_path = root.join("source-plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema":"jianying-cli-plan/v1",
            "name":"typed-edit-source",
            "canvas":{"width":320,"height":240,"fps":30},
            "tracks":[{"type":"text","name":"原字幕","segments":[{
                "start_us":0,"duration_us":1_000_000,"text":"源草稿保持不变"
            }]}]
        }),
    );
    let source = root.join("source");
    let built = run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        source.to_str().unwrap(),
    ]);
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let source_before = std::fs::read(source.join("draft_content.json")).unwrap();

    let job_path = root.join("edit.json");
    write_json(
        &job_path,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"source","output":"edited"},
            "operations":[
                {"operation":"add_material","material":{
                    "type":"video","id":"overlay-material","path":"overlay.mp4"
                }},
                {"operation":"add_track","track_id":"overlay-track","name":"叠加画面",
                    "kind":"video","index":0},
                {"operation":"add_segment","track_id":"overlay-track","segment":{
                    "type":"video","id":"overlay-segment",
                    "range":{"start_us":0,"duration_us":1_000_000},
                    "material_id":"overlay-material",
                    "source_range":{"start_us":0,"duration_us":1_000_000},
                    "speed":1.0,"volume":0.8,"change_pitch":true,
                    "scale":1.1,"x":0.1,"y":-0.1,"rotation":5.0,"opacity":0.9,
                    "crop":{"upper_left_x":0.1,"upper_left_y":0.0,
                        "upper_right_x":0.9,"upper_right_y":0.0,
                        "lower_left_x":0.1,"lower_left_y":1.0,
                        "lower_right_x":0.9,"lower_right_y":1.0},
                    "keyframes":{"scale":[{"at_us":0,"value":1.0},{"at_us":500000,"value":1.2}]},
                    "mask":{"name":"圆形","size":0.6,"feather":10.0},
                    "chroma":{"color":"#00FF00","intensity":30.0,"shadow":0.0,
                        "edge_smooth":10.0,"spill":0.0},
                    "background_filling":{"type":"blur","blur":0.375,"color":""},
                    "mix_mode":"正片叠底",
                    "animation_in":{"name":"渐显","duration_us":200000},
                    "transition_out":{"name":"闪黑","duration_us":200000},
                    "fade":{"in_us":100000,"out_us":100000}
                }},
                {"operation":"add_track","track_id":"temporary-track","name":"临时字幕",
                    "kind":"text","index":2},
                {"operation":"reorder_track","track_id":"temporary-track","index":1},
                {"operation":"remove_track","track_id":"temporary-track"}
            ]
        }),
    );

    let edited = bin()
        .current_dir(&root)
        .args(["job", "run", "edit.json", "--json"])
        .output()
        .unwrap();
    assert!(
        edited.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&edited.stdout),
        String::from_utf8_lossy(&edited.stderr)
    );
    let result = stdout_json(&edited);
    assert_eq!(
        result["data"]["operation_results"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    assert_eq!(result["data"]["isolation"]["source_unchanged"], true);
    assert_eq!(
        std::fs::read(source.join("draft_content.json")).unwrap(),
        source_before
    );

    let output = root.join("edited");
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(output.join("draft_content.json")).unwrap()).unwrap();
    let overlay = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["id"] == "overlay-track")
        .expect("typed track must preserve its declared id");
    assert_eq!(overlay["name"], "叠加画面");
    assert_eq!(overlay["segments"].as_array().unwrap().len(), 1);
    let segment = &overlay["segments"][0];
    assert_eq!(segment["id"], "overlay-segment");
    assert_eq!(segment["speed"], 1.0);
    assert_eq!(segment["volume"], 0.8);
    assert_eq!(segment["clip"]["alpha"], 0.9);
    assert_eq!(segment["clip"]["scale"]["x"], 1.1);
    assert_eq!(segment["clip"]["transform"]["x"], 0.1);
    assert!(!segment["common_keyframes"].as_array().unwrap().is_empty());
    let material_id = segment["material_id"].as_str().unwrap();
    let material = timeline["materials"]["videos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|material| material["id"] == material_id)
        .unwrap();
    assert_eq!(material["crop"]["upper_left_x"], 0.1);
    assert!(Path::new(material["path"].as_str().unwrap()).starts_with(
        output
            .canonicalize()
            .expect("edited output must have a canonical identity")
    ));
    assert!(Path::new(material["path"].as_str().unwrap()).is_file());
    for bucket in [
        "masks",
        "chromas",
        "canvases",
        "effects",
        "material_animations",
        "transitions",
        "audio_fades",
    ] {
        assert!(
            !timeline["materials"][bucket].as_array().unwrap().is_empty(),
            "typed segment must compile {bucket}"
        );
    }
    assert!(timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .all(|track| track["id"] != "temporary-track"));
    assert_eq!(
        stdout_json(&run(&["verify", output.to_str().unwrap()]))["ok"],
        true
    );
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| {
        entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("domain-segment")
    }));
}

#[test]
fn job_edit_rejects_unclosed_material_sequences_without_leaking_staging() {
    let root = temp_root("edit-material-sequence");
    let plan_path = root.join("source-plan.json");
    write_json(
        &plan_path,
        &json!({
            "schema":"jianying-cli-plan/v1","name":"sequence-source",
            "canvas":{"width":320,"height":240,"fps":30},
            "tracks":[{"type":"text","segments":[{
                "start_us":0,"duration_us":1_000_000,"text":"源草稿"
            }]}]
        }),
    );
    let source = root.join("source");
    assert!(run(&[
        "build",
        plan_path.to_str().unwrap(),
        "--out",
        source.to_str().unwrap(),
    ])
    .status
    .success());
    let source_before = std::fs::read(source.join("draft_content.json")).unwrap();
    std::fs::write(root.join("unused.bin"), b"unused").unwrap();

    let unused_job = root.join("unused.json");
    write_json(
        &unused_job,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"source","output":"unused-output"},
            "operations":[{"operation":"add_material","material":{
                "type":"video","id":"unused-material","path":"unused.bin"
            }}]
        }),
    );
    let unused = run(&["job", "run", unused_job.to_str().unwrap(), "--json"]);
    assert_eq!(unused.status.code(), Some(1));
    let unused_error = stdout_json(&unused);
    assert_eq!(unused_error["error"]["type"], "invalid_job");
    assert_eq!(
        unused_error["error"]["details"]["reason"],
        "declared materials were not consumed by add_segment: unused-material"
    );
    assert!(!root.join("unused-output").exists());

    let missing_job = root.join("missing.json");
    write_json(
        &missing_job,
        &json!({
            "schema":"jianying-job/v2","operation":"edit",
            "project":{"type":"existing","source":"source","output":"missing-output"},
            "operations":[
                {"operation":"add_track","track_id":"video-track","kind":"video"},
                {"operation":"add_segment","track_id":"video-track","segment":{
                    "type":"video","id":"video-segment",
                    "range":{"start_us":0,"duration_us":1_000_000},
                    "material_id":"missing-material",
                    "source_range":{"start_us":0,"duration_us":1_000_000},
                    "speed":1.0,"volume":1.0
                }}
            ]
        }),
    );
    let missing = run(&["job", "run", missing_job.to_str().unwrap(), "--json"]);
    assert_eq!(missing.status.code(), Some(1));
    let missing_error = stdout_json(&missing);
    assert_eq!(missing_error["error"]["type"], "invalid_job");
    assert_eq!(
        missing_error["error"]["details"]["reason"],
        "add_segment video-segment references undeclared material missing-material"
    );
    assert!(!root.join("missing-output").exists());
    assert_eq!(
        std::fs::read(source.join("draft_content.json")).unwrap(),
        source_before
    );
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| {
        let name = entry.unwrap().file_name();
        let name = name.to_string_lossy();
        name.contains("jianying-edit") || name.contains("domain-segment")
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

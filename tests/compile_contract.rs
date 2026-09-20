use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str], stdin: Option<&str>) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_jianying"));
    child.args(args);
    if let Some(input) = stdin {
        use std::io::Write;
        child
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let mut child = child.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
        return child.wait_with_output().unwrap();
    }
    child.output().unwrap()
}

fn failure(args: &[&str], stdin: Option<&str>) -> Value {
    let output = run(args, stdin);
    assert_eq!(
        output.status.code(),
        Some(1),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn data(args: &[&str]) -> Value {
    let output = run(args, None);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn scratch() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "jianying-compile-contract-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    for name in ["clip1.mp4", "clip2.mp4", "music.mp3"] {
        std::fs::write(root.join(name), b"synthetic").unwrap();
    }
    root
}

fn write_json(path: &Path, value: &Value) {
    std::fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn base_spec() -> Value {
    json!({
        "name":"Compiled",
        "width":720,"height":1280,"fps":30,"ratio":"9:16",
        "tracks":[
            {"type":"video","items":[
                {"ref":"hero","path":"clip1.mp4","start":0,"duration":2,"speed":1.25,"opacity":0.8,"scale":1.1},
                {"path":"clip2.mp4","start":2,"duration":3}
            ]},
            {"type":"audio","items":[{"ref":"music","path":"music.mp3","start":0,"duration":5,"volume":0.4}]},
            {"type":"text","items":[{"ref":"hook","text":"Hook","start":0,"duration":2,"fontSize":18,"color":"#FFD700","y":-0.6}]}
        ]
    })
}

#[test]
fn compile_preflights_builds_refs_operations_batch_and_job_v2() {
    let root = scratch();
    let spec_path = root.join("spec.json");
    let mut spec = base_spec();
    let srt = root.join("captions.srt");
    std::fs::write(&srt, "1\n00:00:00,000 --> 00:00:01,000\nHello\n").unwrap();
    let template = root.join("template.json");
    write_json(
        &template,
        &json!({
            "name":"cta","type":"text",
            "segment":{"id":"old-seg","material_id":"old-mat","target_timerange":{"start":0,"duration":1000000},
                "source_timerange":{"start":0,"duration":1000000},"extra_material_refs":[],"common_keyframes":[],"keyframe_refs":[],
                "speed":1,"volume":1,"visible":true,"reverse":false,"clip":{"alpha":1,"rotation":0,"scale":{"x":1,"y":1},"transform":{"x":0,"y":0}}},
            "material":{"type":"texts","data":{"id":"old-mat","type":"text","content":r#"{"styles":[{"range":[0,3],"size":15}],"text":"CTA"}"#}},
            "extra_materials":[]
        }),
    );
    spec["operations"] = json!([
        {"op":"transition","target":"hero","slug":"dissolve","duration":0.4},
        {"op":"keyframe","target":"hero","property":"uniform_scale","time":0,"value":1,"easing":"ease-out"},
        {"op":"audio-fade","target":"music","fadeIn":0.5,"fadeOut":0.5},
        {"op":"text-style","target":"hook","style":{"borderWidth":0.08,"borderColor":"#000000"}},
        {"op":"text-ranges","target":"hook","ranges":[{"start":0,"end":4,"font_color":"#00FF00"}]},
        {"op":"filter","slug":"vintage","start":0,"duration":2},
        {"op":"effect","slug":"shake","start":0,"duration":1},
        {"op":"template","path":"template.json","start":2,"duration":1,"text":"Follow","ref":"cta"},
        {"op":"captions","path":"captions.srt","trackName":"captions","styleRef":"hook"}
    ]);
    write_json(&spec_path, &spec);

    let checked = data(&[
        "project",
        "compile",
        &spec_path.to_string_lossy(),
        "--check",
        "--json",
    ]);
    assert_eq!(checked["checked"], true);
    assert_eq!(checked["write"], false);
    assert_eq!(checked["tracks"], 3);
    assert_eq!(checked["items"], 4);
    assert_eq!(checked["operations"], 9);
    assert_eq!(checked["refs"], json!(["hero", "music", "hook"]));

    let out = root.join("out");
    let built = data(&[
        "project",
        "compile",
        &spec_path.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(built["ok"], true);
    assert_eq!(built["tracks"], 3);
    assert_eq!(built["segments"], 8);
    assert_eq!(built["duration_us"], 5_000_000);
    assert!(built["refs"]["hero"].is_string());
    assert!(built["refs"]["cta"].is_string());
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(out.join("draft_content.json")).unwrap()).unwrap();
    assert_eq!(timeline["canvas_config"]["width"], 720);
    assert_eq!(timeline["canvas_config"]["ratio"], "9:16");
    assert_eq!(timeline["duration"], 5_000_000);
    assert!(!timeline["materials"]["transitions"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!timeline["materials"]["audio_fades"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["type"] == "filter"));
    assert!(timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["type"] == "effect"));
    assert!(timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["name"] == "captions"));

    let template_spec = root.join("batch-spec.json");
    let mut batch_spec = base_spec();
    batch_spec["name"] = json!("{{name}}");
    batch_spec["tracks"][2]["items"][0]["text"] = json!("{{title}} for {{price}}");
    write_json(&template_spec, &batch_spec);
    let rows = root.join("rows.jsonl");
    std::fs::write(
        &rows,
        "{\"name\":\"A\",\"title\":\"Alpha\",\"price\":9}\n{\"name\":\"B\",\"title\":\"Beta\",\"price\":19}\n",
    )
    .unwrap();
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let batch = data(&[
        "project",
        "compile",
        &template_spec.to_string_lossy(),
        "--data",
        &rows.to_string_lossy(),
        "--drafts",
        &store.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(batch.as_array().unwrap().len(), 2);
    assert!(store.join("A/draft_content.json").is_file());
    assert!(store.join("B/draft_content.json").is_file());

    let job = root.join("compile-job.json");
    write_json(
        &job,
        &json!({
            "schema":"jianying-job/v2","operation":"create",
            "compatibility":{"schema":"capcut-cli-compile/v1","payload":base_spec()}
        }),
    );
    let job_out = root.join("job-out");
    let job_result = data(&[
        "job",
        "run",
        &job.to_string_lossy(),
        "--out",
        &job_out.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(job_result["operation"], "create");
    assert_eq!(job_result["compile"]["ok"], true);
    assert!(job_out.join("draft_content.json").is_file());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn compile_fails_closed_before_writes_and_reports_partial_batch() {
    let root = scratch();
    let store = root.join("store");
    std::fs::create_dir_all(&store).unwrap();
    let spec_path = root.join("spec.json");
    let mut spec = base_spec();
    spec["name"] = json!("{{name}}");
    write_json(&spec_path, &spec);

    let fail_fast = failure(
        &[
            "project",
            "compile",
            &spec_path.to_string_lossy(),
            "--data",
            "-",
            "--drafts",
            &store.to_string_lossy(),
            "--json",
        ],
        Some("{\"name\":\"valid\"}\n{\"wrong\":\"missing name\"}\n"),
    );
    assert_eq!(fail_fast["error"]["type"], "execution_failed");
    assert!(fail_fast["error"]["message"]
        .as_str()
        .unwrap()
        .contains("no drafts written"));
    assert!(!store.join("valid").exists());

    let partial = failure(
        &[
            "project",
            "compile",
            &spec_path.to_string_lossy(),
            "--data",
            "-",
            "--drafts",
            &store.to_string_lossy(),
            "--continue-on-error",
            "--json",
        ],
        Some("{\"name\":\"built\"}\n{\"wrong\":\"missing name\"}\n"),
    );
    assert_eq!(partial["error"]["type"], "compile_batch_partial");
    assert_eq!(partial["error"]["details"]["failed"], 1);
    assert_eq!(
        partial["error"]["details"]["results"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(store.join("built/draft_content.json").is_file());

    let mut invalid = base_spec();
    invalid["name"] = json!("../escape");
    write_json(&spec_path, &invalid);
    let invalid_name = failure(
        &[
            "project",
            "compile",
            &spec_path.to_string_lossy(),
            "--drafts",
            &store.to_string_lossy(),
            "--json",
        ],
        None,
    );
    assert!(invalid_name["error"]["message"]
        .as_str()
        .unwrap()
        .contains("plain folder name"));
    assert!(!root.join("escape").exists());

    let mut invalid_easing = base_spec();
    invalid_easing["operations"] = json!([{
        "op":"keyframe","target":"hero","property":"uniform_scale",
        "time":0,"value":1,"easing":"spring"
    }]);
    write_json(&spec_path, &invalid_easing);
    let easing = failure(
        &[
            "project",
            "compile",
            &spec_path.to_string_lossy(),
            "--check",
            "--json",
        ],
        None,
    );
    assert!(easing["error"]["message"]
        .as_str()
        .unwrap()
        .contains("Unsupported keyframe easing"));

    let _ = std::fs::remove_dir_all(root);
}

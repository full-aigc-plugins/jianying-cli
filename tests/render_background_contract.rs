use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .expect("jianying binary must start")
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn pixel(path: &Path, at: &str, x: usize, y: usize) -> [u8; 3] {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss", at, "-i"])
        .arg(path)
        .args([
            "-vf",
            &format!("crop=1:1:{x}:{y},format=rgb24"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert_success(&output);
    assert_eq!(output.stdout.len(), 3);
    output.stdout.try_into().unwrap()
}

#[test]
fn job_v2_proxy_renders_only_the_referenced_canvas_blur() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "jianying-proxy-background-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let media = root.join("portrait.mp4");
    assert_success(
        &Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=red:s=180x320:d=2:r=30",
                "-vf",
                "drawbox=x=0:y=160:w=180:h=160:color=blue:t=fill",
                "-an",
                "-c:v",
                "libx264",
            ])
            .arg(&media)
            .output()
            .unwrap(),
    );
    let job_path = root.join("job.json");
    std::fs::write(
        &job_path,
        serde_json::to_vec(&json!({
            "schema":"jianying-job/v2", "operation":"create",
            "project":{"type":"new","project":{
                "name":"proxy-background", "width":320, "height":180,
                "frame_rate":{"numerator":30,"denominator":1},
                "timeline":{"tracks":[{"id":"video-main","kind":"video","segments":[
                    {"type":"video","id":"blurred","range":{"start_us":0,"duration_us":1_000_000},
                     "material_id":"portrait","source_range":{"start_us":0,"duration_us":1_000_000},
                     "speed":1,"volume":0,"background_filling":{"type":"blur","blur":0.5}},
                    {"type":"video","id":"plain","range":{"start_us":1_000_000,"duration_us":1_000_000},
                     "material_id":"portrait","source_range":{"start_us":1_000_000,"duration_us":1_000_000},
                     "speed":1,"volume":0}
                ]}]},
                "materials":[{"type":"video","id":"portrait","path":media}]
            }}
        }))
        .unwrap(),
    )
    .unwrap();
    let draft = root.join("draft");
    assert_success(&run(&[
        "job",
        "run",
        job_path.to_str().unwrap(),
        "--out",
        draft.to_str().unwrap(),
        "--state-root",
        root.join("state").to_str().unwrap(),
        "--json",
    ]));
    let draft_content = draft.join("draft_content.json");
    let mut wire: Value = serde_json::from_slice(&std::fs::read(&draft_content).unwrap()).unwrap();
    let blur_id = wire["materials"]["canvases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|canvas| canvas["type"] == "canvas_blur")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let segments = wire["tracks"][0]["segments"].as_array().unwrap();
    assert!(segments[0]["extra_material_refs"]
        .as_array()
        .unwrap()
        .contains(&json!(blur_id)));
    assert!(!segments[1]["extra_material_refs"]
        .as_array()
        .unwrap()
        .contains(&json!(blur_id)));

    let proxy = root.join("proxy.mp4");
    let rendered = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        proxy.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ]);
    assert_success(&rendered);
    let receipt: Value = serde_json::from_slice(&rendered.stdout).unwrap();
    assert_eq!(receipt["data"]["background_blur_segments"], 1);
    let blurred_edge = pixel(&proxy, "0.5", 0, 90);
    let plain_edge = pixel(&proxy, "1.5", 0, 90);
    assert!(
        blurred_edge[0] > 50 && blurred_edge[2] > 50,
        "referenced canvas_blur must mix the source's red/blue boundary at the edge; RGB={blurred_edge:?}"
    );
    assert!(
        plain_edge.iter().all(|component| *component < 30),
        "unreferenced segment must retain the default black edge; RGB={plain_edge:?}"
    );

    let original_wire = wire.clone();
    wire["materials"]["canvases"]
        .as_array_mut()
        .unwrap()
        .retain(|canvas| canvas["id"] != blur_id);
    std::fs::write(&draft_content, serde_json::to_vec(&wire).unwrap()).unwrap();
    let rejected = root.join("rejected.mp4");
    let failure = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        rejected.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ]);
    assert!(
        !failure.status.success(),
        "dangling canvas ref must fail closed"
    );
    assert!(!rejected.exists());

    wire = original_wire.clone();
    let invalid_canvas = wire["materials"]["canvases"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|canvas| canvas["id"] == blur_id)
        .unwrap();
    invalid_canvas["blur"] = json!(2.0);
    std::fs::write(&draft_content, serde_json::to_vec(&wire).unwrap()).unwrap();
    let invalid = root.join("invalid.mp4");
    assert!(!run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        invalid.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ])
    .status
    .success());
    assert!(!invalid.exists());

    wire = original_wire;
    let mut second_blur = wire["materials"]["canvases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|canvas| canvas["id"] == blur_id)
        .unwrap()
        .clone();
    second_blur["id"] = json!("another-blur");
    wire["materials"]["canvases"]
        .as_array_mut()
        .unwrap()
        .push(second_blur);
    wire["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array_mut()
        .unwrap()
        .push(json!("another-blur"));
    std::fs::write(&draft_content, serde_json::to_vec(&wire).unwrap()).unwrap();
    let conflict = root.join("conflict.mp4");
    assert!(!run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        conflict.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ])
    .status
    .success());
    assert!(!conflict.exists());
    let _ = std::fs::remove_dir_all(root);
}

use serde_json::json;
use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .expect("jianying binary must start")
}

fn assert_success(output: &std::process::Output) {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn left_pixel(path: &Path) -> [u8; 3] {
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-ss", "0.5", "-i"])
        .arg(path)
        .args([
            "-vf",
            "crop=1:1:0:90,format=rgb24",
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
fn job_v2_proxy_applies_clip_scale_before_canvas_padding() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "jianying-proxy-scale-{}-{}",
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
                "color=c=red:s=180x320:d=1:r=30",
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
                "name":"proxy-scale", "width":320, "height":180,
                "frame_rate":{"numerator":30,"denominator":1},
                "timeline":{"tracks":[{"id":"video-main","kind":"video","segments":[{
                    "type":"video","id":"portrait","range":{"start_us":0,"duration_us":1_000_000},
                    "material_id":"portrait","source_range":{"start_us":0,"duration_us":1_000_000},
                    "speed":1,"volume":0,"scale":3.17
                }]}]},
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
    let proxy = root.join("proxy.mp4");
    assert_success(&run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        proxy.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ]));
    let pixel = left_pixel(&proxy);
    assert!(
        pixel[0] > 180 && pixel[1] < 90 && pixel[2] < 90,
        "clip scale must fill the left canvas edge; observed RGB={pixel:?}"
    );

    let draft_content = draft.join("draft_content.json");
    let mut wire: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&draft_content).unwrap()).unwrap();
    wire["tracks"][0]["segments"][0]["clip"]["scale"]["x"] = json!(0);
    std::fs::write(&draft_content, serde_json::to_vec(&wire).unwrap()).unwrap();
    let rejected_proxy = root.join("rejected.mp4");
    let rejected = run(&[
        "render",
        "proxy",
        draft.to_str().unwrap(),
        "--out",
        rejected_proxy.to_str().unwrap(),
        "--scale",
        "1",
        "--json",
    ]);
    assert!(!rejected.status.success());
    assert!(!rejected_proxy.exists());
    let _ = std::fs::remove_dir_all(root);
}

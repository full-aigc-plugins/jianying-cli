use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn fixture() -> (PathBuf, Vec<u8>) {
    let root = std::env::temp_dir().join(format!(
        "jianying-material-discovery-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let timeline = json!({
        "tracks":[],
        "materials":{
            "videos":[
                {"id":"ABCDEF001122","name":"fallback","material_name":"clip-a.mp4",
                    "path":"/media/a.mp4","duration":1_000_000,"type":"video","unknown":{"keep":true}},
                {"id":"video-b","material_name":"clip-b.mp4","path":"/media/b.mp4",
                    "duration":2_000_000,"type":"photo"}
            ],
            "audios":[{"id":"audio-a","name":"music.wav","path":"/media/music.wav",
                "duration":3_000_000,"type":"extract_music"}],
            "texts":[]
        }
    });
    let bytes = serde_json::to_vec_pretty(&timeline).unwrap();
    std::fs::write(root.join("draft_content.json"), &bytes).unwrap();
    (root, bytes)
}

#[test]
fn material_discovery_matches_fixed_capcut_summary_and_prefix_semantics() {
    let (draft, before) = fixture();
    let draft_arg = draft.to_string_lossy();

    let counts = run_ok(&["media", "materials", &draft_arg, "--json"]);
    assert_eq!(
        counts,
        json!([
            {"type":"videos","count":2},
            {"type":"audios","count":1},
            {"type":"texts","count":0}
        ])
    );
    let videos = run_ok(&[
        "media",
        "materials",
        &draft_arg,
        "--type",
        "videos",
        "--json",
    ]);
    assert_eq!(videos[0]["name"], "clip-a.mp4");
    assert_eq!(videos[0]["duration_us"], 1_000_000);
    assert_eq!(videos[0]["fields"], 7);
    assert_eq!(videos[1]["type"], "photo");

    let detail = run_ok(&["media", "material", &draft_arg, "abcdef", "--json"]);
    assert_eq!(detail["_type"], "videos");
    assert_eq!(detail["id"], "ABCDEF001122");
    assert_eq!(detail["unknown"]["keep"], true);
    assert_eq!(
        std::fs::read(draft.join("draft_content.json")).unwrap(),
        before
    );

    let missing = run(&["media", "material", &draft_arg, "missing", "--json"]);
    assert_eq!(missing.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&missing.stdout).unwrap();
    assert_eq!(error["ok"], false);
    assert!(error["error"]["message"]
        .as_str()
        .unwrap()
        .contains("material not found"));
}

#[test]
fn unknown_material_type_is_a_structured_read_failure() {
    let (draft, _) = fixture();
    let output = run(&[
        "media",
        "materials",
        &draft.to_string_lossy(),
        "--type",
        "unknown",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unknown material type"));
}

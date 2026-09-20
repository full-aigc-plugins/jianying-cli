use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn fixture() -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "jianying-otio-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let segment = |id: &str, material_id: &str, start: i64, duration: i64| {
        json!({"id":id,"material_id":material_id,
            "target_timerange":{"start":start,"duration":duration},
            "source_timerange":{"start":0,"duration":duration},
            "speed":1.0,"volume":1.0,"visible":true,"clip":null,
            "extra_material_refs":[],"render_index":0})
    };
    let timeline = json!({
        "id":"draft-otio","name":"OTIO test","duration":3_000_000,"fps":30,
        "tracks":[
            {"id":"TV","type":"video","name":"Video 1","segments":[
                segment("seg-a","vid-a",0,1_000_000),
                {"id":"seg-b","material_id":"vid-b",
                    "target_timerange":{"start":2_000_000,"duration":1_000_000},
                    "source_timerange":{"start":500_000,"duration":2_000_000},
                    "speed":2.0,"volume":1.0,"visible":true,"clip":null,
                    "extra_material_refs":[],"render_index":0}
            ]},
            {"id":"TA","type":"audio","name":"Audio 1","segments":[
                segment("seg-c","aud-a",0,3_000_000)
            ]},
            {"id":"TT","type":"text","name":"Captions","segments":[
                segment("seg-t","txt-a",0,1_000_000)
            ]}
        ],
        "materials":{
            "videos":[
                {"id":"vid-a","path":"/media/a.mp4","material_name":"a.mp4",
                    "duration":5_000_000,"type":"video"},
                {"id":"vid-b","path":"","material_name":"missing","duration":0,"type":"video"}
            ],
            "audios":[{"id":"aud-a","path":"/media/song.mp3","name":"song.mp3",
                "duration":10_000_000,"type":"audio"}],
            "texts":[{"id":"txt-a","content":"{\"text\":\"你好\",\"styles\":[]}"}]
        }
    });
    let bytes = serde_json::to_vec_pretty(&timeline).unwrap();
    std::fs::write(root.join("draft_content.json"), &bytes).unwrap();
    std::fs::write(root.join("draft_info.json"), bytes).unwrap();
    root
}

fn data(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

#[test]
fn otio_export_preserves_gaps_ranges_speed_media_and_caption_markers() {
    let draft = fixture();
    let draft_arg = draft.to_string_lossy();
    let plain = run(&["project", "export-timeline", &draft_arg, "--json"]);
    let document = data(&plain);
    assert_eq!(document["OTIO_SCHEMA"], "Timeline.1");
    assert_eq!(
        document["tracks"]["children"][0]["children"][1]["OTIO_SCHEMA"],
        "Gap.1"
    );
    assert_eq!(
        document["tracks"]["children"][0]["children"][2]["effects"][0]["time_scalar"],
        2.0
    );
    assert!(String::from_utf8_lossy(&plain.stderr).contains("skipped track \"Captions\""));

    let output_path = draft.join("cut.otio");
    let summary = data(&run(&[
        "project",
        "export-timeline",
        &draft_arg,
        "--out",
        &output_path.to_string_lossy(),
        "--captions",
        "markers",
        "--quiet",
        "--json",
    ]));
    assert_eq!(summary["tracks"], 2);
    assert_eq!(summary["clips"], 3);
    assert_eq!(summary["gaps"], 1);
    assert_eq!(summary["captions"], 1);
    let written: Value = serde_json::from_slice(&std::fs::read(output_path).unwrap()).unwrap();
    assert_eq!(written["tracks"]["markers"][0]["name"], "你好");
    assert_eq!(
        written["tracks"]["markers"][0]["metadata"]["capcut"]["kind"],
        "caption"
    );
}

#[test]
fn otio_export_rejects_unknown_caption_mode_without_writing() {
    let draft = fixture();
    let output_path = draft.join("bad.otio");
    let output = run(&[
        "project",
        "export-timeline",
        &draft.to_string_lossy(),
        "--out",
        &output_path.to_string_lossy(),
        "--captions",
        "burn",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!Path::new(&output_path).exists());
    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["ok"], false);
    assert!(envelope["error"]["message"]
        .as_str()
        .unwrap()
        .contains("skip|markers"));
}

#[test]
fn otio_import_rebuilds_ranges_placeholders_and_caption_markers() {
    let source = fixture();
    let otio = source.join("roundtrip.otio");
    data(&run(&[
        "project",
        "export-timeline",
        &source.to_string_lossy(),
        "--out",
        &otio.to_string_lossy(),
        "--captions",
        "markers",
        "--quiet",
        "--json",
    ]));
    let imported = source.parent().unwrap().join(format!(
        "jianying-otio-imported-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let result = data(&run(&[
        "project",
        "import-timeline",
        &otio.to_string_lossy(),
        "--out",
        &imported.to_string_lossy(),
        "--quiet",
        "--json",
    ]));
    assert_eq!(result["mode"], "out");
    assert_eq!(result["tracks"], 3);
    assert_eq!(result["clips"], 3);
    assert_eq!(result["gaps"], 1);
    assert_eq!(result["captions"], 1);
    assert_eq!(result["placeholders"].as_array().unwrap().len(), 3);
    let timeline: Value =
        serde_json::from_slice(&std::fs::read(imported.join("draft_content.json")).unwrap())
            .unwrap();
    let video = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "video")
        .unwrap();
    assert_eq!(video["segments"][1]["target_timerange"]["start"], 2_000_000);
    assert_eq!(video["segments"][1]["source_timerange"]["start"], 500_000);
    assert_eq!(video["segments"][1]["speed"], 2.0);
    assert!(timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|track| track["type"] == "text" && track["name"] == "Captions"));

    let before = std::fs::read(imported.join("draft_content.json")).unwrap();
    let dry_run = data(&run(&[
        "project",
        "import-timeline",
        &otio.to_string_lossy(),
        "--into",
        &imported.to_string_lossy(),
        "--dry-run",
        "--json",
    ]));
    assert_eq!(dry_run["dryRun"], true);
    assert_eq!(
        std::fs::read(imported.join("draft_content.json")).unwrap(),
        before
    );
}

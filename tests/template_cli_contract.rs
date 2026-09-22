use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo, template};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 5_000_000,
        width: 1280,
        height: 720,
        has_video: false,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn build(root: &Path, name: &str, size: f64) -> PathBuf {
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1","name":name,
        "canvas":{"width":1280,"height":720,"fps":30},
        "tracks":[{"type":"text","name":"字幕","segments":[{
            "start_us":0,"duration_us":1_000_000,"text":"模板字幕","size":size,
            "color":"#34ABCD","bold":true
        }]}]
    }))
    .unwrap();
    let out = root.join(name);
    draft::build(&plan, root, &out, None, &probe_stub).unwrap();
    out
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

fn count_files(path: &Path) -> usize {
    if !path.is_dir() {
        return usize::from(path.is_file());
    }
    std::fs::read_dir(path)
        .unwrap()
        .map(|entry| count_files(&entry.unwrap().path()))
        .sum()
}

#[test]
fn template_library_save_list_apply_and_duplicate_are_independent() {
    let root = std::env::temp_dir().join(format!(
        "jianying-template-library-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let source = build(&root, "source", 8.0);
    let library = root.join("library");
    let output_root = root.join("outputs");

    let saved = run_ok(&[
        "template",
        "save",
        &source.to_string_lossy(),
        "social-card",
        "--root",
        &library.to_string_lossy(),
        "--description",
        "vertical social caption",
        "--json",
    ]);
    let template = library.join("social-card");
    assert_eq!(saved["manifest"]["schema"], "jianying-template/v1");
    assert!(template.join(".jianying-template.json").is_file());

    let listed = run_ok(&[
        "template",
        "list",
        "--root",
        &library.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(listed["templates"].as_array().unwrap().len(), 1);
    assert_eq!(listed["templates"][0]["name"], "social-card");

    let applied = run_ok(&[
        "template",
        "apply",
        &template.to_string_lossy(),
        "episode-01",
        "--root",
        &output_root.to_string_lossy(),
        "--json",
    ]);
    let output = output_root.join("episode-01");
    assert_eq!(applied["status"], "applied");
    assert!(!output.join(".jianying-template.json").exists());
    assert_eq!(
        run_ok(&["project", "verify", &output.to_string_lossy(), "--json"])["ok"],
        true
    );
    assert_ne!(
        draft::load_timeline(&source).unwrap()["id"],
        draft::load_timeline(&output).unwrap()["id"]
    );

    let duplicate_root = root.join("duplicates");
    let duplicated = run_ok(&[
        "template",
        "duplicate",
        &source.to_string_lossy(),
        "source-copy",
        "--root",
        &duplicate_root.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(duplicated["status"], "duplicated");
    let copied = duplicate_root.join("source-copy");
    assert_eq!(
        run_ok(&["project", "verify", &copied.to_string_lossy(), "--json"])["ok"],
        true
    );

    let import_target = root.join("import-target");
    run_ok(&[
        "project",
        "init",
        "import-target",
        "--out",
        &import_target.to_string_lossy(),
        "--json",
    ]);
    let imported = run_ok(&[
        "template",
        "import-track",
        &import_target.to_string_lossy(),
        &source.to_string_lossy(),
        "字幕",
        "--json",
    ]);
    assert_eq!(imported["status"], "imported");
    assert_eq!(
        run_ok(&[
            "project",
            "verify",
            &import_target.to_string_lossy(),
            "--json"
        ])["ok"],
        true
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn template_text_preset_is_validated_and_applied_transactionally() {
    let root = std::env::temp_dir().join(format!(
        "jianying-template-preset-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let styled = build(&root, "styled", 11.0);
    let target = build(&root, "target", 4.0);
    let presets = root.join("presets");
    let created = run_ok(&[
        "template",
        "make-preset",
        &styled.to_string_lossy(),
        "large-blue",
        "--root",
        &presets.to_string_lossy(),
        "--json",
    ]);
    let preset = PathBuf::from(created["preset"].as_str().unwrap());
    assert_eq!(
        serde_json::from_str::<Value>(&std::fs::read_to_string(&preset).unwrap()).unwrap()
            ["schema"],
        "jianying-template-preset/v1"
    );
    let applied = run_ok(&[
        "template",
        "apply-preset",
        &target.to_string_lossy(),
        &preset.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(applied["text_materials"], 1);
    let captions = run_ok(&["captions", "list", &target.to_string_lossy(), "--json"]);
    assert_eq!(captions[0]["style"]["size"], 11.0);
    assert_eq!(captions[0]["style"]["color"], "#34ABCD");
    assert_eq!(captions[0]["style"]["bold"], true);
    assert_eq!(
        run_ok(&["project", "verify", &target.to_string_lossy(), "--json"])["ok"],
        true
    );

    let malformed = root.join("bad.json");
    std::fs::write(&malformed, r#"{"schema":"unknown"}"#).unwrap();
    assert_eq!(
        run(&[
            "template",
            "apply-preset",
            &target.to_string_lossy(),
            &malformed.to_string_lossy(),
            "--json",
        ])
        .status
        .code(),
        Some(1)
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn template_material_replacement_isolated_segment_timing_and_failure_are_atomic() {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        eprintln!("ffmpeg unavailable — skipping");
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "jianying-template-replace-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let original = root.join("original.mp4");
    let replacement = root.join("replacement.mp4");
    for (path, input) in [
        (&original, "color=c=red:s=320x240:d=2"),
        (&replacement, "color=c=blue:s=640x360:d=1"),
    ] {
        let status = Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(["-f", "lavfi", "-i", input, "-an", "-c:v", "libx264"])
            .arg(path)
            .status()
            .unwrap();
        assert!(status.success());
    }

    let output = root.join("draft");
    run_ok(&[
        "project",
        "init",
        "replace-contract",
        "--out",
        &output.to_string_lossy(),
        "--json",
    ]);
    run_ok(&[
        "media",
        "add-video",
        &output.to_string_lossy(),
        &original.to_string_lossy(),
        "0us",
        "2s",
        "--track-name",
        "主画面",
        "--json",
    ]);
    run_ok(&[
        "media",
        "add-video",
        &output.to_string_lossy(),
        &original.to_string_lossy(),
        "2500ms",
        "2s",
        "--track-name",
        "主画面",
        "--json",
    ]);
    let mut before = draft::load_timeline(&output).unwrap();
    let old_material_id = before["tracks"][0]["segments"][0]["material_id"].clone();
    before["tracks"][0]["segments"][1]["material_id"] = old_material_id.clone();
    template::save_timeline(&output, &before).unwrap();

    let replaced = run_ok(&[
        "template",
        "replace-material",
        &output.to_string_lossy(),
        &replacement.to_string_lossy(),
        "--track",
        "主画面",
        "--index",
        "0",
        "--source-duration",
        "1s",
        "--shrink-mode",
        "cut_tail_align",
        "--json",
    ]);
    assert_eq!(replaced["replacement"]["mode"], "cut_tail_align");
    let timeline = draft::load_timeline(&output).unwrap();
    let segments = timeline["tracks"][0]["segments"].as_array().unwrap();
    assert_ne!(segments[0]["material_id"], old_material_id);
    assert_eq!(segments[1]["material_id"], old_material_id);
    assert_eq!(segments[0]["target_timerange"]["duration"], 1_000_000);
    assert_eq!(segments[1]["target_timerange"]["start"], 1_500_000);
    let replacement_material_id = segments[0]["material_id"].as_str().unwrap();
    let replacement_material = timeline["materials"]["videos"]
        .as_array()
        .unwrap()
        .iter()
        .find(|material| material["id"].as_str() == Some(replacement_material_id))
        .unwrap();
    let stored = Path::new(replacement_material["path"].as_str().unwrap());
    assert!(stored.starts_with(&output));
    assert!(stored.is_file());
    assert_eq!(draft::verify(&output).unwrap()["ok"], true);

    let content_path = output.join("draft_content.json");
    let content_before_failure = std::fs::read(&content_path).unwrap();
    let files_before_failure = count_files(&output);
    let failed = run(&[
        "template",
        "replace-material",
        &output.to_string_lossy(),
        &replacement.to_string_lossy(),
        "--track",
        "主画面",
        "--index",
        "1",
        "--source-start",
        "800ms",
        "--source-duration",
        "500ms",
        "--json",
    ]);
    assert_eq!(failed.status.code(), Some(1));
    assert_eq!(
        std::fs::read(&content_path).unwrap(),
        content_before_failure
    );
    assert_eq!(count_files(&output), files_before_failure);
    assert_eq!(draft::verify(&output).unwrap()["ok"], true);
    let _ = std::fs::remove_dir_all(root);
}

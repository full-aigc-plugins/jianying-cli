use anyhow::Result;
use jianying_cli::{draft, plan::Plan, probe::MediaInfo};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn probe_stub(path: &Path) -> Result<MediaInfo> {
    Ok(MediaInfo {
        path: path.to_string_lossy().into_owned(),
        duration_us: 10_000_000,
        width: 1920,
        height: 1080,
        has_video: false,
        has_audio: false,
        frame_rate: None,
        streams: Vec::new(),
        is_image: false,
    })
}

fn fixture() -> (PathBuf, PathBuf) {
    let root = std::env::temp_dir().join(format!(
        "jianying-captions-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let plan: Plan = serde_json::from_value(json!({
        "schema":"jianying-cli-plan/v1",
        "name":"captions",
        "canvas":{"width":1920,"height":1080,"fps":30},
        "tracks":[{"type":"text","name":"字幕","segments":[
            {"start_us":0,"duration_us":1_000_000,"text":"你好","size":5.0},
            {"start_us":2_000_000,"duration_us":1_000_000,"text":"世界","size":5.0}
        ]}]
    }))
    .unwrap();
    let draft_dir = root.join("draft");
    draft::build(&plan, &root, &draft_dir, None, &probe_stub).unwrap();
    (root, draft_dir)
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

#[test]
fn captions_cover_list_get_add_set_style_translate_and_srt_roundtrip() {
    let (root, draft_dir) = fixture();
    let draft = draft_dir.to_string_lossy();

    let listed = run_ok(&["captions", "list", &draft, "--json"]);
    assert_eq!(listed.as_array().unwrap().len(), 2);
    assert_eq!(listed[0]["text"], "你好");
    let first_id = listed[0]["segment_id"].as_str().unwrap().to_owned();
    assert_eq!(
        run_ok(&["captions", "get", &draft, &first_id, "--json"])["text"],
        "你好"
    );

    run_ok(&["captions", "set", &draft, &first_id, "你好🙂", "--json"]);
    run_ok(&[
        "captions", "style", &draft, &first_id, "--size", "7.5", "--color", "#12AB34", "--bold",
        "true", "--json",
    ]);
    let styled = run_ok(&["captions", "get", &draft, &first_id, "--json"]);
    assert_eq!(styled["text"], "你好🙂");
    assert_eq!(styled["style"]["size"], 7.5);
    assert_eq!(styled["style"]["bold"], true);
    assert_eq!(styled["utf16_length"], 4);

    let translations = root.join("translations.json");
    std::fs::write(
        &translations,
        serde_json::to_vec_pretty(&json!({"by_id":{first_id.clone():"Hello"}})).unwrap(),
    )
    .unwrap();
    let translated = run_ok(&[
        "captions",
        "translate",
        &draft,
        &translations.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(translated["translated"], 1);
    assert_eq!(
        run_ok(&["captions", "get", &draft, &first_id, "--json"])["text"],
        "Hello"
    );

    let added = run_ok(&[
        "captions",
        "add",
        &draft,
        "新增字幕",
        "4s",
        "1.5s",
        "--track",
        "字幕",
        "--json",
    ]);
    assert_eq!(added["start_us"], 4_000_000);

    let exported = root.join("captions.srt");
    run_ok(&[
        "captions",
        "export-srt",
        &draft,
        "--out",
        &exported.to_string_lossy(),
        "--json",
    ]);
    let exported_text = std::fs::read_to_string(&exported).unwrap();
    assert!(exported_text.contains("00:00:04,000 --> 00:00:05,500"));
    assert!(exported_text.contains("新增字幕"));

    let import = root.join("import.srt");
    std::fs::write(&import, "1\n00:00:06,000 --> 00:00:07,000\n导入字幕\n").unwrap();
    let imported = run_ok(&[
        "captions",
        "import-srt",
        &draft,
        &import.to_string_lossy(),
        "--track",
        "导入轨",
        "--offset",
        "500ms",
        "--json",
    ]);
    assert_eq!(imported["imported"], 1);
    assert_eq!(imported["track_name"], "导入轨");
    assert_eq!(run_ok(&["project", "verify", &draft, "--json"])["ok"], true);

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn captions_import_and_export_ass_without_losing_multiline_text() {
    let (root, draft_dir) = fixture();
    let ass = root.join("input.ass");
    std::fs::write(
        &ass,
        "[Script Info]\nScriptType: v4.00+\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:04.00,0:00:05.25,Default,,0,0,0,,第一行\\N第二行\n",
    )
    .unwrap();
    let draft = draft_dir.to_string_lossy();
    assert_eq!(
        run_ok(&[
            "captions",
            "import-ass",
            &draft,
            &ass.to_string_lossy(),
            "--track",
            "ASS",
            "--json",
        ])["imported"],
        1
    );
    let out = root.join("output.ass");
    run_ok(&[
        "captions",
        "export-ass",
        &draft,
        "--out",
        &out.to_string_lossy(),
        "--json",
    ]);
    let text = std::fs::read_to_string(out).unwrap();
    assert!(text.contains("Dialogue:"));
    assert!(text.contains("第一行\\N第二行"));
    assert_eq!(run_ok(&["project", "verify", &draft, "--json"])["ok"], true);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn captions_reject_invalid_style_and_unknown_translation_ids() {
    let (root, draft_dir) = fixture();
    let draft = draft_dir.to_string_lossy();
    let id = run_ok(&["captions", "list", &draft, "--json"])[0]["segment_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let invalid = run(&["captions", "style", &draft, &id, "--color", "red", "--json"]);
    assert_eq!(invalid.status.code(), Some(1));

    let translations = root.join("bad-translations.json");
    std::fs::write(&translations, r#"{"by_id":{"missing":"x"}}"#).unwrap();
    let unknown = run(&[
        "captions",
        "translate",
        &draft,
        &translations.to_string_lossy(),
        "--json",
    ]);
    assert_eq!(unknown.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn caption_style_ranges_use_utf16_offsets_fill_gaps_and_validate_contract() {
    let (root, draft_dir) = fixture();
    let draft_path = draft_dir.to_string_lossy();
    let segment_id = run_ok(&["captions", "list", &draft_path, "--json"])[0]["segment_id"]
        .as_str()
        .unwrap()
        .to_owned();
    run_ok(&[
        "captions",
        "set",
        &draft_path,
        &segment_id,
        "A😀BC",
        "--json",
    ]);

    let styles_file = root.join("styles.json");
    std::fs::write(
        &styles_file,
        "\u{feff}[{\"start\":3,\"end\":4,\"italic\":true},{\"start\":1,\"end\":3,\"font_color\":\"#FF0000\",\"font_size\":20,\"font_alpha\":0.5,\"bold\":true}]",
    )
    .unwrap();
    let styles_arg = format!("@{}", styles_file.display());
    let result = run_ok(&[
        "captions",
        "style-ranges",
        &draft_path,
        &segment_id[..8],
        "--styles",
        &styles_arg,
        "--json",
    ]);
    assert_eq!(result["segmentId"], segment_id);
    assert_eq!(result["styles"], 4);
    assert_eq!(result["text_length"], 5);

    let timeline = draft::load_timeline(&draft_dir).unwrap();
    let content: Value = serde_json::from_str(
        timeline["materials"]["texts"][0]["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let styles = content["styles"].as_array().unwrap();
    assert_eq!(styles.len(), 4);
    assert_eq!(styles[0]["range"], json!([0, 1]));
    assert_eq!(styles[1]["range"], json!([1, 3]));
    assert_eq!(styles[1]["size"], 20);
    assert_eq!(styles[1]["bold"], true);
    assert_eq!(styles[1]["fill"]["content"]["solid"]["alpha"], 0.5);
    assert_eq!(
        styles[1]["fill"]["content"]["solid"]["color"],
        json!([1.0, 0.0, 0.0])
    );
    assert_eq!(styles[2]["range"], json!([3, 4]));
    assert_eq!(styles[2]["italic"], true);
    assert_eq!(styles[3]["range"], json!([4, 5]));

    let overlapping = run(&[
        "captions",
        "style-ranges",
        &draft_path,
        &segment_id,
        "--styles",
        r#"[{"start":0,"end":2},{"start":1,"end":3}]"#,
        "--json",
    ]);
    assert_eq!(overlapping.status.code(), Some(1));

    let out_of_bounds = run(&[
        "captions",
        "style-ranges",
        &draft_path,
        &segment_id,
        "--styles",
        r#"[{"start":0,"end":6}]"#,
        "--json",
    ]);
    assert_eq!(out_of_bounds.status.code(), Some(1));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn caption_bubble_supports_prefix_slug_raw_ids_and_replacement() {
    let (root, draft_dir) = fixture();
    let draft_path = draft_dir.to_string_lossy();
    let segment_id = run_ok(&["captions", "list", &draft_path, "--json"])[0]["segment_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let prefix = &segment_id[..8];
    let first = run_ok(&[
        "captions",
        "bubble",
        &draft_path,
        prefix,
        "--bubble",
        "cloud",
        "--json",
    ]);
    assert_eq!(first["segmentId"], segment_id);
    assert_eq!(first["effect_id"], "7137269184932778510");
    let first_id = first["bubble_id"].as_str().unwrap().to_owned();
    let raw = run_ok(&[
        "captions",
        "bubble",
        &draft_path,
        prefix,
        "--effect-id",
        "1111111111111111111",
        "--resource-id",
        "2222222222222222222",
        "--json",
    ]);
    let timeline = draft::load_timeline(&draft_dir).unwrap();
    let refs = timeline["tracks"][0]["segments"][0]["extra_material_refs"]
        .as_array()
        .unwrap();
    assert!(!refs
        .iter()
        .any(|item| item.as_str() == Some(first_id.as_str())));
    assert!(refs.iter().any(|item| item == &raw["bubble_id"]));
    assert_eq!(
        timeline["materials"]["filters"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        timeline["materials"]["texts"][0]["bubble_effect_id"],
        "1111111111111111111"
    );
    assert_eq!(
        timeline["materials"]["texts"][0]["bubble_resource_id"],
        "2222222222222222222"
    );

    let missing = run(&["captions", "bubble", &draft_path, prefix, "--json"]);
    assert_ne!(missing.status.code(), Some(0));
    let unknown = run(&[
        "captions",
        "bubble",
        &draft_path,
        prefix,
        "--bubble",
        "missing",
        "--json",
    ]);
    assert_ne!(unknown.status.code(), Some(0));
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(unix)]
#[test]
fn captions_translate_uses_a_structured_process_without_a_shell() {
    let (root, draft_dir) = fixture();
    let provider = root.join("translator.sh");
    std::fs::write(
        &provider,
        "#!/bin/sh\ninput=$(cat)\nprintf '%s' \"$input\" | grep -q 'jianying-caption-translation/v1' || exit 7\nprintf '%s' '{\"by_text\":{\"你好\":\"Hello from provider\"}}'\n",
    )
    .unwrap();
    std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();
    let draft = draft_dir.to_string_lossy();
    let result = run_ok(&[
        "captions",
        "translate",
        &draft,
        "--provider",
        &provider.to_string_lossy(),
        "--to",
        "en-US",
        "--json",
    ]);
    assert_eq!(result["translated"], 1);
    assert_eq!(result["provider_kind"], "structured-command");
    assert_eq!(
        run_ok(&["captions", "list", &draft, "--json"])[0]["text"],
        "Hello from provider"
    );
    let _ = std::fs::remove_dir_all(root);
}

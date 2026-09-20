use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Output};

fn write_draft(path: &Path, name: &str, extra_filter: Option<(&str, &str)>) {
    std::fs::create_dir_all(path).unwrap();
    let mut filters = vec![
        json!({"name":"Known","effect_id":"7028463716732079117","resource_id":"7028463716732079117"}),
        json!({"name":"Bubble","type":"text_shape","effect_id":"bubble-e","resource_id":"bubble-r"}),
    ];
    if let Some((effect, resource)) = extra_filter {
        filters.push(json!({"name":"Fresh Filter","effect_id":effect,"resource_id":resource}));
    }
    let timeline = json!({
        "id":name,"name":name,"tracks":[],
        "materials":{
            "video_effects":[{"name":"Snow Fly","effect_id":"custom-e","resource_id":"custom-r"}],
            "transitions":[],"audio_effects":[],"masks":[],"common_mask":[],"common_masks":[],
            "filters":filters,
            "material_animations":[{"animations":[{"id":"anim-e","name":"Animation","resource_id":"anim-r"}]}],
            "texts":[{"font_id":"font-r"}]
        }
    });
    std::fs::write(
        path.join("draft_content.json"),
        serde_json::to_vec_pretty(&timeline).unwrap(),
    )
    .unwrap();
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn data(args: &[&str]) -> Value {
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
fn scan_and_sync_are_plan_first_deduplicated_and_fail_closed() {
    let root = std::env::temp_dir().join(format!(
        "jianying-harvest-scan-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let library = root.join("drafts");
    let first = library.join("first");
    write_draft(&first, "First", None);
    let catalogue = root.join("user-enums.json");
    let draft_text = first.to_string_lossy();
    let catalogue_text = catalogue.to_string_lossy();

    let plan = data(&[
        "media",
        "harvest-enums",
        "scan",
        &draft_text,
        "--catalogue",
        &catalogue_text,
        "--json",
    ]);
    assert_eq!(plan["applied"], false);
    assert_eq!(plan["found"], 5);
    assert_eq!(plan["known"], 1);
    assert_eq!(plan["new"].as_array().unwrap().len(), 4);
    assert_eq!(plan["writable_slugs"], 1);
    assert_eq!(plan["id_only"], 3);
    assert!(!catalogue.exists());

    let applied = data(&[
        "media",
        "harvest-enums",
        "scan",
        &draft_text,
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(applied["added"], 4);
    assert_eq!(applied["total"], 4);
    let repeated = data(&[
        "media",
        "harvest-enums",
        "scan",
        &draft_text,
        "--catalogue",
        &catalogue_text,
        "--json",
    ]);
    assert_eq!(repeated["new"].as_array().unwrap().len(), 0);
    assert_eq!(repeated["known"], 5);

    let second = library.join("second");
    write_draft(&second, "Second", Some(("fresh-e", "fresh-r")));
    let broken = library.join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("draft_content.json"), "not-json").unwrap();
    let sync = data(&[
        "media",
        "harvest-enums",
        "sync",
        "--drafts",
        &library.to_string_lossy(),
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(sync["drafts_scanned"], 2);
    assert_eq!(sync["drafts_skipped"].as_array().unwrap().len(), 1);
    assert_eq!(sync["added"], 1);
    assert_eq!(sync["new_by_kind"]["filters"], 1);

    std::fs::write(&catalogue, "{broken").unwrap();
    let malformed = run(&[
        "media",
        "harvest-enums",
        "scan",
        &draft_text,
        "--catalogue",
        &catalogue_text,
        "--apply",
        "--json",
    ]);
    assert_eq!(malformed.status.code(), Some(1));
    assert_eq!(std::fs::read_to_string(&catalogue).unwrap(), "{broken");
    let _ = std::fs::remove_dir_all(root);
}

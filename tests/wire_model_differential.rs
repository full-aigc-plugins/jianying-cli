use jianying_cli::draft;
use jianying_cli::lossless_draft::{DraftMetadataWire, DraftTimelineWire};
use jianying_cli::plan::Plan;
use serde_json::{json, Value};
use std::path::Path;

#[test]
fn wire_models_round_trip_known_and_unknown_fields_without_loss() {
    let mut content: Value = serde_json::from_str(draft::CONTENT_TEMPLATE).unwrap();
    content["unknown_root"] = json!({"keep": [1, 2, 3]});
    content["canvas_config"]["unknown_canvas"] = json!(true);
    content["tracks"] = json!([{
        "attribute": 0,
        "flag": 0,
        "id": "track-id",
        "is_default_name": false,
        "name": "text",
        "type": "text",
        "unknown_track": "keep",
        "segments": [{
            "id": "segment-id",
            "material_id": "material-id",
            "target_timerange": {"start": 0, "duration": 1_000_000, "unknown_time": 9},
            "speed": 1.0,
            "volume": 1.0,
            "unknown_segment": {"keep": true}
        }]
    }]);
    let timeline = DraftTimelineWire::from_value(content.clone()).unwrap();
    assert_eq!(timeline.to_value().unwrap(), content);

    let mut metadata: Value = serde_json::from_str(draft::META_TEMPLATE).unwrap();
    metadata["unknown_metadata"] = json!({"keep": "yes"});
    let metadata_wire = DraftMetadataWire::from_value(metadata.clone()).unwrap();
    assert_eq!(metadata_wire.to_value().unwrap(), metadata);
}

#[test]
fn rust_text_wire_matches_frozen_python_normalized_reference() {
    let scenario: Value =
        serde_json::from_str(include_str!("parity/scenarios/17-text-styles.json")).unwrap();
    let plan: Plan = serde_json::from_value(scenario["plan"].clone()).unwrap();
    let temp_root = std::env::temp_dir().join(format!(
        "jianying-wire-diff-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&temp_root).unwrap();
    let out = temp_root.join("rust");
    let unexpected_probe = |_path: &Path| panic!("text-only fixture must not probe media");
    draft::build(&plan, &temp_root, &out, None, &unexpected_probe).unwrap();

    let rust_value: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("draft_content.json")).unwrap())
            .unwrap();
    let rust_wire = DraftTimelineWire::from_value(rust_value).unwrap();
    let python_reference: Value = serde_json::from_str(include_str!(
        "fixtures/pyjyd_wire/text_core.normalized.json"
    ))
    .unwrap();

    assert_eq!(rust_wire.normalized_core(), python_reference);
    std::fs::remove_dir_all(temp_root).unwrap();
}

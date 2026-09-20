use jianying_draft::{DraftBundleIntegrity, DraftMetadataWire, DraftTimelineWire};
use serde_json::{json, Value};
use std::path::Path;

fn timeline() -> Value {
    json!({
        "id": "timeline-1",
        "name": "demo",
        "duration": 1_000_000,
        "materials": {
            "videos": [{
                "id": "video-1", "type": "video", "duration": 1_000_000,
                "path": "/drafts/demo/assets/video/a.mp4"
            }],
            "speeds": [{"id": "speed-1", "type": "speed", "speed": 1.0}],
            "transitions": [{
                "id": "transition-1", "type": "transition", "duration": 100_000,
                "effect_id": "fade", "resource_id": "fade-resource"
            }]
        },
        "tracks": [{
            "id": "track-1", "type": "video",
            "segments": [{
                "id": "segment-1", "material_id": "video-1",
                "target_timerange": {"start": 0, "duration": 1_000_000},
                "extra_material_refs": ["speed-1", "transition-1"]
            }]
        }]
    })
}

fn metadata() -> Value {
    json!({
        "draft_id": "draft-1",
        "draft_name": "demo",
        "draft_fold_path": "/drafts/demo",
        "draft_root_path": "/drafts",
        "draft_json_file": "/drafts/demo/draft_content.json",
        "tm_duration": 1_000_000,
        "draft_materials": [{
            "type": 0,
            "value": [{
                "id": "registry-1", "metetype": "video",
                "file_Path": "/drafts/demo/assets/video/a.mp4"
            }]
        }]
    })
}

#[test]
fn validates_material_ids_references_registration_and_mirrors() {
    let timeline = DraftTimelineWire::from_value(timeline()).unwrap();
    timeline.validate_references().unwrap();
    let mirror = DraftTimelineWire::from_value(timeline.to_value().unwrap()).unwrap();
    let metadata = DraftMetadataWire::from_value(metadata()).unwrap();

    DraftBundleIntegrity::validate(
        &timeline,
        &mirror,
        &metadata,
        Some(Path::new("/drafts/demo")),
    )
    .unwrap();
}

#[test]
fn rejects_duplicate_ids_dangling_references_and_broken_mirrors() {
    let mut duplicate = timeline();
    duplicate["materials"]["speeds"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"video-1", "type":"speed"}));
    assert!(DraftTimelineWire::from_value(duplicate)
        .unwrap()
        .validate_references()
        .unwrap_err()
        .to_string()
        .contains("duplicate material id"));

    let mut dangling = timeline();
    dangling["tracks"][0]["segments"][0]["extra_material_refs"] = json!(["missing-resource"]);
    assert!(DraftTimelineWire::from_value(dangling)
        .unwrap()
        .validate_references()
        .unwrap_err()
        .to_string()
        .contains("dangling"));

    let content = DraftTimelineWire::from_value(timeline()).unwrap();
    let mut changed_mirror = timeline();
    changed_mirror["name"] = json!("other");
    let changed_mirror = DraftTimelineWire::from_value(changed_mirror).unwrap();
    let metadata_wire = DraftMetadataWire::from_value(metadata()).unwrap();
    assert!(
        DraftBundleIntegrity::validate(&content, &changed_mirror, &metadata_wire, None)
            .unwrap_err()
            .to_string()
            .contains("mirror")
    );

    let mut missing_registration = metadata();
    missing_registration["draft_materials"] = json!([]);
    let missing_registration = DraftMetadataWire::from_value(missing_registration).unwrap();
    assert!(
        DraftBundleIntegrity::validate(&content, &content, &missing_registration, None)
            .unwrap_err()
            .to_string()
            .contains("not registered")
    );
}

#[test]
fn accepts_sound_effect_as_audio_tracks_primary_material() {
    let mut value = timeline();
    value["materials"]["audio_effects"] = json!([{
        "id":"sfx-material","type":"sound_effect","resource_id":"7021052523762946561"
    }]);
    value["tracks"][0]["type"] = json!("audio");
    value["tracks"][0]["segments"][0]["material_id"] = json!("sfx-material");
    let wire = DraftTimelineWire::from_value(value).unwrap();
    wire.validate_references().unwrap();
}

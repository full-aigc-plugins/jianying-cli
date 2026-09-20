use jianying_cli::lossless_draft::{JsonPointerPatch, LosslessDraftEnvelope};
use serde_json::json;

fn existing_draft() -> serde_json::Value {
    json!({
        "id": "draft-1",
        "name": "existing",
        "unknown_root": {"must_survive": [1, 2, 3]},
        "materials": {
            "texts": [
                {"id": "text-1", "content": {"text": "旧文本", "unknown_style": 77}},
                {"id": "text-2", "content": {"text": "不应变化"}}
            ],
            "videos": [{"id": "video-1", "unknown_media": true}]
        },
        "tracks": [{
            "type": "text",
            "segments": [{"material_id": "text-1", "extra_material_refs": ["text-1"]}],
            "unknown_track": "keep"
        }],
        "mirror_relationships": {"draft_info": "same-project", "unknown": "keep"}
    })
}

#[test]
fn targeted_patch_preserves_unknown_fields_materials_and_mirror_relationships() {
    let original = existing_draft();
    let mut envelope = LosslessDraftEnvelope::from_value(original.clone()).unwrap();
    let view = envelope.typed_view();
    assert_eq!(view.id(), Some("draft-1"));
    assert_eq!(view.track_count(), 1);
    assert_eq!(view.material_count(), 3);

    envelope
        .apply(JsonPointerPatch::replace(
            "/materials/texts/0/content/text",
            json!("新文本"),
        ))
        .unwrap();
    envelope.verify_only_declared_changes().unwrap();

    let working = envelope.value();
    assert_eq!(
        working["materials"]["texts"][0]["content"]["text"],
        "新文本"
    );
    assert_eq!(
        working["materials"]["texts"][0]["content"]["unknown_style"],
        77
    );
    assert_eq!(
        working["materials"]["texts"][1],
        original["materials"]["texts"][1]
    );
    assert_eq!(
        working["materials"]["videos"],
        original["materials"]["videos"]
    );
    assert_eq!(working["unknown_root"], original["unknown_root"]);
    assert_eq!(
        working["mirror_relationships"],
        original["mirror_relationships"]
    );
    assert_eq!(
        envelope.changed_pointers(),
        &["/materials/texts/0/content/text"]
    );
}

#[test]
fn patching_a_missing_or_root_path_is_rejected_without_mutation() {
    let original = existing_draft();
    let mut envelope = LosslessDraftEnvelope::from_value(original.clone()).unwrap();
    assert!(envelope
        .apply(JsonPointerPatch::replace(
            "/materials/texts/9/content",
            json!("x")
        ))
        .is_err());
    assert!(envelope
        .apply(JsonPointerPatch::replace("", json!({"replacement": true})))
        .is_err());
    assert_eq!(envelope.value(), &original);
}

#[test]
fn non_object_drafts_are_rejected() {
    assert!(LosslessDraftEnvelope::from_value(json!([])).is_err());
}

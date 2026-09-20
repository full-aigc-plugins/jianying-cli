use jianying_draft::{DraftResourceKind, DraftTimelineWire};
use serde_json::json;

#[test]
fn inventory_classifies_all_migrated_resource_semantics() {
    let timeline = DraftTimelineWire::from_value(json!({
        "materials": {
            "videos": [{"id":"video-1","type":"video","duration":1000000}],
            "audios": [{"id":"audio-1","type":"extract_music","duration":1000000}],
            "texts": [{"id":"text-1","type":"text","content":"{\"text\":\"测试\"}"}],
            "stickers": [{"id":"sticker-1","type":"sticker","resource_id":"sticker-rid"}],
            "effects": [
                {"id":"filter-1","type":"filter","effect_id":"filter-eid","resource_id":"filter-rid"},
                {"id":"effect-1","type":"mix_mode","effect_id":"effect-eid","resource_id":"effect-rid"}
            ],
            "video_effects": [{"id":"video-effect-1","type":"video_effect","effect_id":"video-effect-eid","resource_id":"video-effect-rid"}],
            "transitions": [{"id":"transition-1","type":"transition","duration":500000,"effect_id":"transition-eid","resource_id":"transition-rid"}],
            "masks": [{"id":"mask-1","type":"mask","resource_id":"mask-rid","config":{}}],
            "material_animations": [{
                "id":"animation-1","type":"sticker_animation",
                "animations":[{"type":"in","resource_id":"animation-rid","start":0,"duration":500000}]
            }]
        },
        "tracks": [{
            "type":"video",
            "segments":[{
                "id":"segment-1",
                "material_id":"video-1",
                "target_timerange":{"start":0,"duration":1000000},
                "common_keyframes":[{
                    "id":"keyframe-list-1",
                    "property_type":"KFTypePositionX",
                    "material_id":"",
                    "keyframe_list":[{
                        "id":"keyframe-point-1",
                        "curveType":"Line",
                        "time_offset":0,
                        "values":[0.25]
                    }]
                }]
            }]
        }]
    }))
    .unwrap();

    let inventory = timeline.resource_inventory().unwrap();
    assert_eq!(inventory.len(), 11);
    for expected in [
        DraftResourceKind::Video,
        DraftResourceKind::Audio,
        DraftResourceKind::Text,
        DraftResourceKind::Sticker,
        DraftResourceKind::Filter,
        DraftResourceKind::Effect,
        DraftResourceKind::Transition,
        DraftResourceKind::Mask,
        DraftResourceKind::Animation,
        DraftResourceKind::Keyframe,
    ] {
        assert!(
            inventory.count(expected) >= 1,
            "missing resource kind {expected:?}"
        );
    }
    let keyframe = inventory
        .resources()
        .iter()
        .find(|resource| resource.kind() == DraftResourceKind::Keyframe)
        .unwrap();
    assert_eq!(keyframe.id(), "keyframe-list-1");
    assert_eq!(keyframe.wire_type(), Some("KFTypePositionX"));
}

#[test]
fn malformed_resource_shapes_fail_before_write() {
    for malformed in [
        json!({"materials":{"videos":[{"type":"video"}]}}),
        json!({"materials":{"transitions":{"id":"not-an-array"}}}),
        json!({"materials":{"stickers":[{"id":"sticker","type":"sticker","resource_id":""}]}}),
        json!({"materials":{"material_animations":[{"id":"animation","type":"sticker_animation","animations":[]}]}}),
        json!({"tracks":[{"type":"video","segments":[{
            "common_keyframes":[{
                "id":"kf","property_type":"","keyframe_list":[]
            }]
        }]}]}),
    ] {
        let timeline = DraftTimelineWire::from_value(malformed).unwrap();
        assert!(timeline.resource_inventory().is_err());
    }
}

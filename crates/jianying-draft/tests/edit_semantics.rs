use jianying_draft::DraftTimelineWire;
use serde_json::{json, Value};

fn edited_timeline() -> Value {
    json!({
        "materials": {
            "videos": [{
                "id":"video-1", "type":"video", "duration":2_000_000,
                "crop": {
                    "upper_left_x":0.0,"upper_left_y":0.0,
                    "upper_right_x":1.0,"upper_right_y":0.0,
                    "lower_left_x":0.0,"lower_left_y":1.0,
                    "lower_right_x":1.0,"lower_right_y":1.0
                }
            }],
            "texts": [{
                "id":"text-1", "type":"subtitle", "alignment":1,
                "content":"{\"text\":\"你A\",\"styles\":[{\"range\":[0,2],\"size\":8.0,\"strokes\":[]}]}"
            }],
            "speeds": [{"id":"speed-1","type":"speed","speed":1.25}],
            "effects": [{
                "id":"mix-1","type":"mix_mode","effect_id":"multiply",
                "resource_id":"multiply-resource","value":1.0
            }]
        },
        "tracks": [
            {"id":"track-video","type":"video","segments":[{
                "id":"segment-video","material_id":"video-1","speed":1.25,"volume":0.8,
                "target_timerange":{"start":0,"duration":1_000_000},
                "source_timerange":{"start":0,"duration":1_250_000},
                "clip":{
                    "alpha":0.9,"rotation":10.0,
                    "scale":{"x":1.1,"y":1.1},"transform":{"x":0.1,"y":-0.2},
                    "flip":{"horizontal":false,"vertical":false}
                },
                "uniform_scale":{"on":true,"value":1.0},
                "extra_material_refs":["speed-1","mix-1"]
            }]},
            {"id":"track-text","type":"text","segments":[{
                "id":"segment-text","material_id":"text-1","speed":1.0,"volume":1.0,
                "target_timerange":{"start":0,"duration":1_000_000},
                "clip":{
                    "alpha":1.0,"rotation":0.0,
                    "scale":{"x":1.0,"y":1.0},"transform":{"x":0.0,"y":-0.78},
                    "flip":{"horizontal":false,"vertical":false}
                }
            }]}
        ]
    })
}

#[test]
fn validates_subtitle_style_speed_volume_crop_transform_and_compositing() {
    DraftTimelineWire::from_value(edited_timeline())
        .unwrap()
        .validate_edit_semantics()
        .unwrap();
}

#[test]
fn rejects_inconsistent_or_out_of_range_edit_semantics() {
    let mut speed = edited_timeline();
    speed["tracks"][0]["segments"][0]["speed"] = json!(2.0);
    assert!(DraftTimelineWire::from_value(speed)
        .unwrap()
        .validate_edit_semantics()
        .unwrap_err()
        .to_string()
        .contains("speed"));

    let mut crop = edited_timeline();
    crop["materials"]["videos"][0]["crop"]["upper_left_x"] = json!(1.5);
    assert!(DraftTimelineWire::from_value(crop)
        .unwrap()
        .validate_edit_semantics()
        .is_err());

    let mut text = edited_timeline();
    text["materials"]["texts"][0]["content"] =
        json!("{\"text\":\"你A\",\"styles\":[{\"range\":[0,3],\"size\":8.0}]}");
    assert!(DraftTimelineWire::from_value(text)
        .unwrap()
        .validate_edit_semantics()
        .unwrap_err()
        .to_string()
        .contains("UTF-16"));

    let mut mix = edited_timeline();
    mix["materials"]["effects"][0]["value"] = json!(1.5);
    assert!(DraftTimelineWire::from_value(mix)
        .unwrap()
        .validate_edit_semantics()
        .is_err());
}

//! Validation matrix — every hard rule in `jianying-cli-plan/v1` gets a
//! positive and a negative case. Field facts mirror pyJianYingDraft's
//! constructors and validators (the parity authority).

use jianying_cli::plan::Plan;
use serde_json::{json, Value};

fn base_plan() -> Value {
    json!({
        "schema": "jianying-cli-plan/v1",
        "name": "v",
        "canvas": {"width": 1920, "height": 1080, "fps": 30},
        "tracks": [{"type": "video", "segments": [
            {"start_us": 0, "duration_us": 1_000_000, "source": "a.mp4"}]}]
    })
}

fn mutate(f: impl Fn(&mut Value)) -> Result<Plan, serde_json::Error> {
    let mut v = base_plan();
    f(&mut v);
    serde_json::from_value(v)
}

fn seg_mut(p: &mut Value, f: impl Fn(&mut Value)) {
    f(&mut p["tracks"][0]["segments"][0]);
}

#[test]
fn accepts_the_base_plan() {
    mutate(|_| {}).unwrap().validate().unwrap();
}

#[test]
fn rejects_bad_schema_name_canvas_fps() {
    // wrong schema: deserializes, validate() rejects
    assert!(mutate(|p| p["schema"] = json!("other/v9"))
        .unwrap()
        .validate()
        .is_err());
    // draft name must be a visible single path component
    for bad in ["", ".hidden", "a/b", "a\\b", ".."] {
        let r = mutate(|p| p["name"] = json!(bad));
        assert!(r.is_err() || r.unwrap().validate().is_err(), "name {bad:?}");
    }
    for (w, h) in [(8u64, 1080u64), (9000, 1080), (1920, 8)] {
        let m = mutate(|p| {
            p["canvas"]["width"] = json!(w);
            p["canvas"]["height"] = json!(h);
        });
        assert!(m.unwrap().validate().is_err(), "canvas {w}x{h}");
    }
    for fps in [23u64, 0, 61, 120] {
        let m = mutate(|p| p["canvas"]["fps"] = json!(fps));
        assert!(m.unwrap().validate().is_err(), "fps {fps}");
    }
}

#[test]
fn main_track_rules() {
    // main video must be first
    let mut p = base_plan();
    p["tracks"].as_array_mut().unwrap().insert(
        0,
        json!({"type": "audio", "segments": [
            {"start_us": 0, "duration_us": 500_000, "source": "n.mp3"}]}),
    );
    let plan: Plan = serde_json::from_value(p).unwrap();
    assert!(plan.validate().is_err());
    // main video must start at 0
    let r = mutate(|p| seg_mut(p, |s| s["start_us"] = json!(100)));
    assert!(r.unwrap().validate().is_err());
    // gaps on the main track are rejected (continuity)
    let mut p = base_plan();
    p["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start_us": 1_500_000, "duration_us": 500_000, "source": "b.mp4"}));
    let plan: Plan = serde_json::from_value(p).unwrap();
    assert!(plan.validate().is_err());
    // same span as an overlay track is fine
    let mut p = base_plan();
    p["tracks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "video", "segments": [
            {"start_us": 500_000, "duration_us": 1_000_000, "source": "b.mp4"}]}));
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
    // video track optional (pyJYD parity): audio-only draft validates
    let mut p = base_plan();
    p["tracks"] = json!([{"type": "audio", "segments": [
        {"start_us": 0, "duration_us": 1_000_000, "source": "n.mp3"}]}]);
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn segment_ordering_and_ranges() {
    // overlap on the same track
    let mut p = base_plan();
    p["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start_us": 500_000, "duration_us": 400_000, "source": "b.mp4"}));
    assert!(serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .is_err());
    // speed / volume bounds
    for (k, lo_bad, hi_bad) in [("speed", 0.05, 8.5), ("volume", -0.1, 4.5)] {
        for v in [lo_bad, hi_bad] {
            let r = mutate(|p| seg_mut(p, |s| s[k] = json!(v)));
            assert!(r.unwrap().validate().is_err(), "{k}={v}");
        }
    }
    // non-positive duration
    for d in [0, -5] {
        let r = mutate(|p| seg_mut(p, |s| s["duration_us"] = json!(d)));
        assert!(r.unwrap().validate().is_err());
    }
    // text fields on a video segment
    let r = mutate(|p| seg_mut(p, |s| s["text"] = json!("x")));
    assert!(r.is_err() || r.unwrap().validate().is_err());
}

#[test]
fn transition_rules() {
    // on the last segment of the main track -> rejected
    let r = mutate(|p| {
        seg_mut(p, |s| {
            s["transition_out"] = json!({"name": "叠化", "duration_us": 300_000});
        })
    });
    assert!(r.unwrap().validate().is_err());
    // with a successor it passes; duration bounds still apply
    let mk = |dur: i64| -> Result<Plan, _> {
        let mut p = base_plan();
        p["tracks"][0]["segments"]
            .as_array_mut()
            .unwrap()
            .push(json!({"start_us": 1_000_000, "duration_us": 500_000, "source": "b.mp4"}));
        p["tracks"][0]["segments"][0]["transition_out"] =
            json!({"name": "叠化", "duration_us": dur});
        serde_json::from_value(p)
    };
    mk(500_000).unwrap().validate().unwrap();
    // pyJYD imposes no ceiling — catalog defaults of 1.5s/2s are legal too
    mk(1_500_000).unwrap().validate().unwrap();
    for bad in [0, -300_000] {
        assert!(mk(bad).unwrap().validate().is_err(), "dur {bad}");
    }
    // omitted duration falls back to the catalog default (叠化 = 500ms)
    let mut p = base_plan();
    p["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start_us": 1_000_000, "duration_us": 500_000, "source": "b.mp4"}));
    p["tracks"][0]["segments"][0]["transition_out"] = json!({"name": "叠化"});
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
    // unknown transition name
    let mut p = base_plan();
    p["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start_us": 1_000_000, "duration_us": 500_000, "source": "b.mp4"}));
    p["tracks"][0]["segments"][0]["transition_out"] =
        json!({"name": "不存在的转场", "duration_us": 300_000});
    assert!(serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .is_err());
}

#[test]
fn vip_authorization_boundary() {
    let mut p = base_plan();
    p["tracks"][0]["segments"]
        .as_array_mut()
        .unwrap()
        .push(json!({"start_us": 1_000_000, "duration_us": 500_000, "source": "b.mp4"}));
    p["tracks"][0]["segments"][0]["transition_out"] =
        json!({"name": "叠化扭曲", "duration_us": 300_000}); // VIP per catalog
    let no = serde_json::from_value::<Plan>(p.clone()).unwrap();
    assert!(no.validate().is_err(), "VIP without allow_vip");
    p["allow_vip"] = json!(true);
    let yes: Plan = serde_json::from_value(p).unwrap();
    yes.validate().unwrap();
}

#[test]
fn mask_rules() {
    let circle = |extra: Value| -> Result<Plan, _> {
        let mut p = base_plan();
        p["tracks"][0]["segments"][0]["mask"] = extra;
        serde_json::from_value(p)
    };
    circle(json!({"name": "圆形", "size": 0.6}))
        .unwrap()
        .validate()
        .unwrap();
    for bad in [
        json!({"name": "圆形", "size": 1.5}),
        json!({"name": "圆形", "feather": 150.0}),
        json!({"name": "未知形", "size": 0.5}),
    ] {
        assert!(circle(bad).unwrap().validate().is_err());
    }
    // rect-only knobs on non-rect masks
    assert!(circle(json!({"name": "圆形", "rect_width": 0.5}))
        .unwrap()
        .validate()
        .is_err());
    assert!(
        circle(json!({"name": "矩形", "size": 0.5, "round_corner": 30.0}))
            .unwrap()
            .validate()
            .is_ok()
    );
}

#[test]
fn keyframe_rules() {
    let kf = |points: Value| -> Result<Plan, _> {
        let mut p = base_plan();
        p["tracks"][0]["segments"][0]["keyframes"] = json!({"scale": points});
        serde_json::from_value(p)
    };
    kf(json!([{"at_us": 0, "value": 1.0}, {"at_us": 500_000, "value": 1.1}]))
        .unwrap()
        .validate()
        .unwrap();
    // first point must be at 0
    assert!(
        kf(json!([{"at_us": 100, "value": 1.0}, {"at_us": 500_000, "value": 1.1}]))
            .unwrap()
            .validate()
            .is_err()
    );
    // single point
    assert!(kf(json!([{"at_us": 0, "value": 1.0}]))
        .unwrap()
        .validate()
        .is_err());
    // non-ascending
    assert!(kf(
        json!([{"at_us": 0, "value": 1.0}, {"at_us": 500_000, "value": 1.1},
                      {"at_us": 400_000, "value": 1.2}])
    )
    .unwrap()
    .validate()
    .is_err());
    // out of range value / beyond segment
    assert!(
        kf(json!([{"at_us": 0, "value": 12.0}, {"at_us": 500_000, "value": 1.1}]))
            .unwrap()
            .validate()
            .is_err()
    );
    assert!(
        kf(json!([{"at_us": 0, "value": 1.0}, {"at_us": 9_999_999, "value": 1.1}]))
            .unwrap()
            .validate()
            .is_err()
    );
    // pyJYD parity: keyframes take precedence over static values
    let mut p = base_plan();
    p["tracks"][0]["segments"][0]["scale"] = json!(1.05);
    p["tracks"][0]["segments"][0]["keyframes"] =
        json!({"scale": [{"at_us": 0, "value": 1.0}, {"at_us": 500_000, "value": 1.1}]});
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
    // pyJYD parity: keyframes allowed on trimmed/speed-adjusted sources
    let mut p = base_plan();
    p["tracks"][0]["segments"][0]["source_start_us"] = json!(200_000);
    p["tracks"][0]["segments"][0]["keyframes"] =
        json!({"scale": [{"at_us": 0, "value": 1.0}, {"at_us": 500_000, "value": 1.1}]});
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn text_style_rules_are_utf16() {
    let mut p = base_plan();
    p["tracks"].as_array_mut().unwrap().push(json!({
        "type": "text", "segments": [{
            "start_us": 0, "duration_us": 800_000, "text": "强调部分",
            "styles": [{"range": [0, 2], "color": "#FF0000"}]}]}));
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
    // range beyond the UTF-16 length (4 chars) is rejected
    let mut p = base_plan();
    p["tracks"].as_array_mut().unwrap().push(json!({
        "type": "text", "segments": [{
            "start_us": 0, "duration_us": 800_000, "text": "强调部分",
            "styles": [{"range": [0, 9], "color": "#FF0000"}]}]}));
    assert!(serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .is_err());
    // emoji count as UTF-16 units (surrogate pair)
    let mut p = base_plan();
    p["tracks"].as_array_mut().unwrap().push(json!({
        "type": "text", "segments": [{
            "start_us": 0, "duration_us": 800_000, "text": "😀字",
            "styles": [{"range": [0, 2], "color": "#00FF00"}]}]}));
    serde_json::from_value::<Plan>(p)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn tim_strings_accepted_in_plan_loading() {
    // Plan::load preprocesses *_us strings — exercised through the tim module
    let us = |s: &str| jianying_cli::tim::parse(s).unwrap();
    assert_eq!(us("1h"), 3_600_000_000);
    assert_eq!(us("1h52m3s"), 6_723_000_000); // 3600s+3120s+3s
    assert_eq!(us("0.5s"), 500_000);
    assert_eq!(us("500ms"), 500_000);
    assert_eq!(us("1500us"), 1_500);
    assert_eq!(us("00:00:02,500"), 2_500_000);
    assert_eq!(us("0s"), 0);
    assert!(jianying_cli::tim::parse("nonsense").is_err());
    // preprocess converts string values under *_us keys
    let mut v = json!({"start_us": "200ms", "nested": [{"duration_us": "1.5s"}]});
    jianying_cli::tim::preprocess(&mut v);
    assert_eq!(v["start_us"], json!(200_000));
    assert_eq!(v["nested"][0]["duration_us"], json!(1_500_000));
}

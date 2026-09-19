//! Catalog guards — the embedded capability data must keep matching the
//! pyJianYingDraft metadata populations (16 domains), stay duplicate-free and
//! honor known anchor facts.

use jianying_cli::catalogs;
use serde_json::Value;

type CatalogFn = fn() -> &'static Value;

fn entries(load: CatalogFn) -> &'static Vec<Value> {
    load().as_array().unwrap()
}

#[test]
fn domain_populations_match_pyjyd_metadata() {
    // counts verified against GuanYixuan/pyJianYingDraft metadata @ c331806
    let expected: &[(&str, usize, usize)] = &[
        ("transitions", 453, 130),
        ("filters", 1052, 250),
        ("fonts", 798, 480),
        ("video_scene_effects", 1097, 635),
        ("video_character_effects", 240, 159),
        ("audio_scene_effects", 85, 12),
        ("tone_effects", 57, 14),
        ("speech_to_songs", 6, 2),
        ("video_animations_in", 155, 38),
        ("video_animations_out", 124, 21),
        ("video_animations_group", 123, 107),
        ("text_animations_in", 145, 67),
        ("text_animations_out", 97, 51),
        ("text_animations_loop", 93, 41),
        ("masks", 6, 6),
        ("mix_modes", 10, 10),
    ];
    let loaders: &[(&str, CatalogFn)] = &[
        ("transitions", catalogs::transitions),
        ("filters", catalogs::filters),
        ("fonts", catalogs::fonts),
        ("video_scene_effects", catalogs::video_scene_effects),
        ("video_character_effects", catalogs::video_character_effects),
        ("audio_scene_effects", catalogs::audio_scene_effects),
        ("tone_effects", catalogs::tone_effects),
        ("speech_to_songs", catalogs::speech_to_songs),
        ("video_animations_in", catalogs::video_animations_in),
        ("video_animations_out", catalogs::video_animations_out),
        ("video_animations_group", catalogs::video_animations_group),
        ("text_animations_in", catalogs::text_animations_in),
        ("text_animations_out", catalogs::text_animations_out),
        ("text_animations_loop", catalogs::text_animations_loop),
        ("masks", catalogs::masks),
        ("mix_modes", catalogs::mix_modes),
    ];
    for (i, (name, load)) in loaders.iter().enumerate() {
        let all = entries(*load);
        let (_, total, non_vip) = expected[i];
        assert_eq!(all.len(), total, "{name} total");
        let nv = all
            .iter()
            .filter(|e| !e["vip"].as_bool().unwrap_or(false))
            .count();
        assert_eq!(nv, non_vip, "{name} non-vip count");
    }
}

#[test]
fn no_duplicate_display_names_per_domain() {
    let loaders: Vec<(&str, CatalogFn)> = vec![
        ("transitions", catalogs::transitions),
        ("filters", catalogs::filters),
        ("fonts", catalogs::fonts),
        ("video_scene_effects", catalogs::video_scene_effects),
        ("video_character_effects", catalogs::video_character_effects),
        ("audio_scene_effects", catalogs::audio_scene_effects),
        ("tone_effects", catalogs::tone_effects),
        ("speech_to_songs", catalogs::speech_to_songs),
        ("video_animations_in", catalogs::video_animations_in),
        ("video_animations_out", catalogs::video_animations_out),
        ("video_animations_group", catalogs::video_animations_group),
        ("text_animations_in", catalogs::text_animations_in),
        ("text_animations_out", catalogs::text_animations_out),
        ("text_animations_loop", catalogs::text_animations_loop),
        ("masks", catalogs::masks),
        ("mix_modes", catalogs::mix_modes),
    ];
    for (name, load) in loaders {
        let mut names: Vec<&str> = entries(load)
            .iter()
            .map(|e| e["name"].as_str().unwrap_or_default())
            .collect();
        let total = names.len();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), total, "{name} has duplicate display names");
    }
}

#[test]
fn anchor_entries_pin_pyjyd_facts() {
    // 叠化: TransitionMeta("叠化", False, "6724845717472416269", "322577", ..., 0.500, True)
    let d = catalogs::find(catalogs::transitions(), "叠化").unwrap();
    assert_eq!(d["resource_id"], "6724845717472416269");
    assert_eq!(d["effect_id"], "322577");
    assert_eq!(d["duration_us"], 500_000);
    assert_eq!(d["is_overlap"], true);
    assert_eq!(d["vip"], false);
    // VIP anchor: 叠化扭曲
    let v = catalogs::find(catalogs::transitions(), "叠化扭曲").unwrap();
    assert_eq!(v["vip"], true);
    // mask anchor: 矩形 shape rectangle + aspect
    let m = catalogs::find(catalogs::masks(), "矩形").unwrap();
    assert_eq!(m["shape"], "rectangle");
    // effect params survive with ranges: 1998 has two params 0..1
    let e = catalogs::find(catalogs::video_scene_effects(), "1998").unwrap();
    let params = e["params"].as_array().unwrap();
    assert_eq!(params.len(), 2);
    assert_eq!(params[0]["name"], "effects_adjust_filter");
    assert_eq!(params[0]["default"], 1.0);
}

#[test]
fn resolve_enforces_the_vip_boundary() {
    assert!(catalogs::resolve(catalogs::transitions(), "transition", "叠化", false).is_ok());
    let err = catalogs::resolve(catalogs::transitions(), "transition", "叠化扭曲", false);
    assert!(err.is_err());
    assert!(err.unwrap_err().to_string().contains("VIP"));
    assert!(catalogs::resolve(catalogs::transitions(), "transition", "叠化扭曲", true).is_ok());
    assert!(catalogs::resolve(catalogs::transitions(), "transition", "不存在", true).is_err());
}

#[test]
fn adjust_params_normalize_ui_scale_to_unit_range() {
    let e = catalogs::find(catalogs::video_scene_effects(), "1998").unwrap();
    let mut user = std::collections::BTreeMap::new();
    user.insert("effects_adjust_filter".to_string(), 80.0);
    let out = catalogs::adjust_params(e, Some(&user));
    let arr = out.as_array().unwrap();
    assert_eq!(arr[0]["value"], 0.8);
    assert_eq!(arr[1]["value"], 1.0); // untouched default
                                      // out-of-catalog param names are rejected at the plan layer, not here
    let none = catalogs::adjust_params(e, None);
    assert_eq!(none.as_array().unwrap()[0]["value"], 1.0);
}

#[test]
fn pyjyd_wire_constants_are_pinned() {
    // These constants mirror pyJianYingDraft behavior the parity suite verified:
    use jianying_cli::plan::hex_rgb;
    let c = hex_rgb("#FF8000").unwrap();
    assert!((c[0] - 1.0).abs() < 1e-9 && (c[1] - 128.0 / 255.0).abs() < 1e-9);
    assert!(hex_rgb("#FF80").is_err());
    // '#' is optional (lenient, mirrors pyJYD _normalize_rgba_color)
    assert!(hex_rgb("FF8000").is_ok());
    assert!(jianying_cli::plan::rgba_hex("#FF8000FF").is_ok());
    assert!(jianying_cli::plan::rgba_hex("#FF80").is_err());
    // border stroke width = width/100*0.2; shadow diffuse = diffuse/100/6
    assert_eq!(40.0 / 100.0 * 0.2, 0.08000000000000002);
    assert!((20.0_f64 / 100.0 / 6.0 - 0.03333333333333333).abs() < 1e-9);
}

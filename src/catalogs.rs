//! Capability catalogs generated from GuanYixuan/pyJianYingDraft metadata
//! (Apache-2.0 DATA: name/is_vip/resource_id/effect_id/md5/params tables).
//! Regenerate with `tools/gen_catalogs.py <pyJianYingDraft-checkout>`.

use anyhow::{bail, Result};
use serde_json::Value;
use std::sync::OnceLock;

macro_rules! catalog {
    ($file:literal, $load:ident) => {
        pub fn $load() -> &'static Value {
            static CELL: OnceLock<Value> = OnceLock::new();
            CELL.get_or_init(|| {
                serde_json::from_str(include_str!(concat!("../catalogs/", $file)))
                    .expect("embedded catalog must parse")
            })
        }
    };
}

catalog!("transitions.json", transitions);
catalog!("filters.json", filters);
catalog!("fonts.json", fonts);
catalog!("video_scene_effects.json", video_scene_effects);
catalog!("video_character_effects.json", video_character_effects);
catalog!("audio_scene_effects.json", audio_scene_effects);
catalog!("tone_effects.json", tone_effects);
catalog!("speech_to_songs.json", speech_to_songs);
catalog!("video_animations_in.json", video_animations_in);
catalog!("video_animations_out.json", video_animations_out);
catalog!("video_animations_group.json", video_animations_group);
catalog!("text_animations_in.json", text_animations_in);
catalog!("text_animations_out.json", text_animations_out);
catalog!("text_animations_loop.json", text_animations_loop);
catalog!("masks.json", masks);
catalog!("mix_modes.json", mix_modes);
catalog!("capcut/audio_effects.json", capcut_audio_effects);
catalog!("capcut/masks.json", capcut_masks);
catalog!("capcut/filters.json", capcut_filters);
catalog!("capcut/bubbles.json", capcut_bubbles);
catalog!("capcut/effects.json", capcut_effects);
catalog!("capcut/transitions.json", capcut_transitions);
catalog!("capcut/text_animations.json", capcut_text_animations);
catalog!("capcut/image_animations.json", capcut_image_animations);
catalog!("capcut/enums.json", capcut_enums);
catalog!(
    "capcut/jianying_text_animations.json",
    capcut_jianying_text_animations
);

/// Look a display name up in a catalog array.
pub fn find<'a>(catalog: &'a Value, name: &str) -> Option<&'a Value> {
    catalog
        .as_array()?
        .iter()
        .find(|e| e["name"].as_str() == Some(name))
}

fn entry_str(entry: &Value, field: &str) -> String {
    entry[field].as_str().unwrap_or_default().to_string()
}

/// Resolve a named entry in a catalog, enforcing the VIP authorization
/// boundary (`allow_vip` comes from the plan, default false).
pub fn resolve(
    catalog: &'static Value,
    domain: &str,
    name: &str,
    allow_vip: bool,
) -> Result<&'static Value> {
    let entry = find(catalog, name).ok_or_else(|| {
        anyhow::anyhow!("{domain} {name:?} is not in the bundled catalog (name is case-sensitive)")
    })?;
    if entry["vip"].as_bool().unwrap_or(false) && !allow_vip {
        bail!(
            "{domain} {name:?} is a VIP resource; the plan sets allow_vip=false \
             (membership entitlement is the user's own, jianying-cli never asserts it)"
        );
    }
    Ok(entry)
}

pub fn resource_id(entry: &Value) -> String {
    entry_str(entry, "resource_id")
}

pub fn effect_id(entry: &Value) -> String {
    entry_str(entry, "effect_id")
}

pub fn is_vip(entry: &Value) -> bool {
    entry["vip"].as_bool().unwrap_or(false)
}

/// Effect params (catalog values are normalized 0..1; plan values are 0..100
/// like the 剪映 UI and get divided by 100).
pub fn adjust_params(
    entry: &Value,
    user: Option<&std::collections::BTreeMap<String, f64>>,
) -> Value {
    let mut out = Vec::new();
    if let Some(params) = entry["params"].as_array() {
        for (i, p) in params.iter().enumerate() {
            let pname = p["name"].as_str().unwrap_or_default();
            let mut value = p["default"].as_f64().unwrap_or(1.0);
            if let Some(u) = user.and_then(|m| m.get(pname)).copied() {
                value = (u / 100.0).clamp(0.0, 1.0);
            }
            out.push(serde_json::json!({
                "default_value": p["default"],
                "max_value": p["max"],
                "min_value": p["min"],
                "name": pname,
                "parameterIndex": i,
                "portIndex": 0,
                "value": value,
            }));
        }
    }
    Value::Array(out)
}

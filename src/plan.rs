//! `jianying-cli-plan/v1` — the full-capability input contract.
//!
//! Times are microseconds on the timeline; `start_us` positions a segment on
//! its track; sources are local files; the first video track is the continuous
//! main track. Capability names resolve against the bundled catalogs
//! (generated from pyJianYingDraft's Apache-2.0 metadata).

use anyhow::{bail, Result};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "jianying-cli-plan/v1";
pub const FPS_VALUES: [u64; 5] = [24, 25, 30, 50, 60];

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub name: String,
    pub canvas: Canvas,
    pub tracks: Vec<Track>,
    /// Assert the user's own membership entitlement for VIP catalog entries.
    #[serde(default)]
    pub allow_vip: bool,
    /// Directory of the plan file, set by `Plan::load`; relative sources
    /// resolve against it.
    #[serde(skip)]
    pub parent: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub width: u64,
    pub height: u64,
    pub fps: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub name: Option<String>,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub start_us: i64,
    pub duration_us: i64,
    #[serde(default)]
    pub source: Option<PathBuf>,
    #[serde(default)]
    pub source_start_us: i64,
    #[serde(default)]
    pub source_duration_us: Option<i64>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub photo: bool,
    // visuals (video/sticker/text)
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    #[serde(default)]
    pub rotation: Option<f64>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub keyframes: Option<BTreeMap<String, Vec<KeyPoint>>>,
    // video-only capabilities
    #[serde(default)]
    pub mask: Option<Mask>,
    #[serde(default)]
    pub filters: Vec<NamedIntensity>,
    #[serde(default)]
    pub effects: Vec<NamedParams>,
    #[serde(default)]
    pub mix_mode: Option<String>,
    #[serde(default)]
    pub animation_in: Option<Animation>,
    #[serde(default)]
    pub animation_out: Option<Animation>,
    #[serde(default)]
    pub animation_group: Option<Animation>,
    #[serde(default)]
    pub transition_out: Option<TransitionOut>,
    // audio-only capabilities
    #[serde(default)]
    pub fade: Option<Fade>,
    #[serde(default)]
    pub audio_effects: Vec<NamedParams>,
    // text-only capabilities
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub border_color: Option<String>,
    #[serde(default)]
    pub border_width: Option<f64>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    #[serde(default)]
    pub underline: Option<bool>,
    #[serde(default)]
    pub alignment: Option<u8>,
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub background: Option<TextBackground>,
    #[serde(default)]
    pub shadow: Option<TextShadow>,
    /// Additional styled ranges (UTF-16 code-unit offsets into `text`).
    #[serde(default)]
    pub styles: Vec<StyleRange>,
    /// Raw 花字 passthrough (pyJianYingDraft ships no 花字 catalog).
    #[serde(default)]
    pub text_effect: Option<RawIds>,
    // sticker-only
    #[serde(default)]
    pub sticker_id: Option<String>,
    #[serde(default)]
    pub resource_id: Option<String>,
    // filter/effect track (global scope)
    #[serde(default)]
    pub intensity: Option<f64>,
    #[serde(default)]
    pub params: Option<BTreeMap<String, f64>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct KeyPoint {
    pub at_us: i64,
    pub value: f64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TransitionOut {
    pub name: String,
    #[serde(default)]
    pub duration_us: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Mask {
    pub name: String,
    #[serde(default)]
    pub center_x: f64,
    #[serde(default)]
    pub center_y: f64,
    #[serde(default)]
    pub size: f64,
    #[serde(default)]
    pub rotation: f64,
    #[serde(default)]
    pub feather: f64,
    #[serde(default)]
    pub invert: bool,
    #[serde(default)]
    pub rect_width: Option<f64>,
    #[serde(default)]
    pub round_corner: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NamedIntensity {
    pub name: String,
    #[serde(default)]
    pub intensity: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct NamedParams {
    pub name: String,
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Animation {
    pub name: String,
    #[serde(default)]
    pub duration_us: Option<i64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Fade {
    pub in_us: i64,
    pub out_us: i64,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TextBackground {
    pub color: String,
    #[serde(default)]
    pub style: Option<u8>,
    #[serde(default)]
    pub alpha: Option<f64>,
    #[serde(default)]
    pub round_radius: Option<f64>,
    #[serde(default)]
    pub height: Option<f64>,
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub horizontal_offset: Option<f64>,
    #[serde(default)]
    pub vertical_offset: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct TextShadow {
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub alpha: Option<f64>,
    #[serde(default)]
    pub angle: Option<f64>,
    #[serde(default)]
    pub distance: Option<f64>,
    #[serde(default)]
    pub diffuse: Option<f64>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct StyleRange {
    /// [start, end] in UTF-16 code units
    pub range: [usize; 2],
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub bold: Option<bool>,
    #[serde(default)]
    pub italic: Option<bool>,
    #[serde(default)]
    pub underline: Option<bool>,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct RawIds {
    pub effect_id: String,
    pub resource_id: String,
}

fn check_range(v: f64, lo: f64, hi: f64, what: &str) -> Result<()> {
    if !(lo..=hi).contains(&v) {
        bail!("{what} {v} outside {lo}..{hi}");
    }
    Ok(())
}

const KEY_CHANNELS_VIDEO: &[(&str, f64, f64)] = &[
    ("scale", 0.01, 10.0),
    ("x", -2.0, 2.0),
    ("y", -2.0, 2.0),
    ("rotation", -360.0, 360.0),
    ("opacity", 0.0, 1.0),
];
const KEY_CHANNELS_TEXT: &[(&str, f64, f64)] = &[
    ("scale", 0.01, 10.0),
    ("x", -2.0, 2.0),
    ("y", -2.0, 2.0),
    ("rotation", -360.0, 360.0),
];
const KEY_CHANNELS_AUDIO: &[(&str, f64, f64)] = &[("volume", 0.0, 4.0)];

impl Plan {
    pub fn load(path: &Path) -> Result<Plan> {
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read plan {}: {e}", path.display()))?;
        let mut plan: Plan = serde_json::from_str(&raw)
            .map_err(|e| anyhow::anyhow!("plan is not a valid {SCHEMA}: {e}"))?;
        plan.parent = Some(path.parent().map(Path::to_path_buf).unwrap_or_default());
        plan.validate()?;
        Ok(plan)
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema != SCHEMA {
            bail!("unsupported plan schema {} (expected {SCHEMA})", self.schema);
        }
        let name = &self.name;
        if name.is_empty()
            || name.len() >= 200
            || name.starts_with('.')
            || name.chars().any(|c| "/\\\n\r\0".contains(c))
        {
            bail!("draft name must be a visible single directory component");
        }
        if !(16..=8192).contains(&self.canvas.width) || !(16..=8192).contains(&self.canvas.height)
        {
            bail!("canvas dimensions must be 16..8192");
        }
        if !FPS_VALUES.contains(&self.canvas.fps) {
            bail!("fps must be one of {FPS_VALUES:?}");
        }
        if self.tracks.is_empty() {
            bail!("plan needs at least one track");
        }
        let mut seen_main = false;
        for (ti, track) in self.tracks.iter().enumerate() {
            if track.kind == "video" && !seen_main {
                if ti != 0 {
                    bail!("the main video track must be the first track");
                }
                seen_main = true;
            }
            if !matches!(
                track.kind.as_str(),
                "video" | "audio" | "text" | "sticker" | "filter" | "effect"
            ) {
                bail!("unsupported track kind {}", track.kind);
            }
            if matches!(track.kind.as_str(), "filter" | "effect") {
                let same_kind = self.tracks.iter().filter(|t| t.kind == track.kind).count();
                if same_kind > 1 {
                    bail!("multiple {} tracks need separate native acceptance", track.kind);
                }
            }
            if track.segments.is_empty() {
                bail!("track {} needs at least one segment", ti);
            }
            let mut prior_end: i64 = 0;
            for (si, seg) in track.segments.iter().enumerate() {
                let last = ti == 0 && si + 1 == track.segments.len();
                self.validate_segment(track, seg, prior_end, last)?;
                prior_end = seg.start_us + seg.duration_us;
            }
            if track.kind == "video" && ti == 0 && track.segments[0].start_us != 0 {
                bail!("main video must start at 0; place gaps on overlay tracks only");
            }
        }
        if !seen_main {
            bail!("plan needs a video track (the main track)");
        }
        Ok(())
    }

    fn resolve_effect(&self, name: &str) -> Result<&'static Value> {
        crate::catalogs::resolve(crate::catalogs::video_scene_effects(), "effect", name, self.allow_vip)
            .or_else(|_| {
                crate::catalogs::resolve(
                    crate::catalogs::video_character_effects(),
                    "effect",
                    name,
                    self.allow_vip,
                )
            })
    }

    fn validate_segment(
        &self,
        track: &Track,
        seg: &Segment,
        prior_end: i64,
        last_on_main: bool,
    ) -> Result<()> {
        if seg.start_us < prior_end {
            bail!(
                "segments on the {} track must be ordered and non-overlapping ({} < {})",
                track.kind,
                seg.start_us,
                prior_end
            );
        }
        if seg.duration_us <= 0 {
            bail!("duration_us must be positive");
        }
        let speed = seg.speed.unwrap_or(1.0);
        check_range(speed, 0.1, 8.0, "speed")?;
        check_range(seg.volume.unwrap_or(1.0), 0.0, 4.0, "volume")?;

        match track.kind.as_str() {
            "video" | "audio" => {
                let src = seg
                    .source
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("segment at {} needs a source", seg.start_us))?;
                if src.as_os_str().is_empty() {
                    bail!("segment at {} has an empty source path", seg.start_us);
                }
                if seg.text.is_some() || seg.size.is_some() || seg.color.is_some() {
                    bail!("{}-track segments cannot carry text fields", track.kind);
                }
            }
            "text" => {
                if seg.text.as_deref().map(str::trim).unwrap_or("").is_empty() {
                    bail!("text segment at {} has empty text", seg.start_us);
                }
                if seg.source.is_some() {
                    bail!("text segments cannot carry a source");
                }
            }
            "sticker" => {
                if seg.sticker_id.is_none() || seg.resource_id.is_none() {
                    bail!("sticker segment at {} needs sticker_id and resource_id", seg.start_us);
                }
            }
            "filter" => {
                let name = seg.filters.first().map(|f| f.name.clone()).ok_or_else(|| {
                    anyhow::anyhow!("filter-track segment at {} needs filters[0]", seg.start_us)
                })?;
                crate::catalogs::resolve(crate::catalogs::filters(), "filter", &name, self.allow_vip)?;
                if let Some(i) = seg.intensity.or_else(|| seg.filters[0].intensity) {
                    check_range(i, 0.0, 100.0, "filter intensity")?;
                }
            }
            "effect" => {
                let name = seg.effects.first().map(|e| e.name.clone()).ok_or_else(|| {
                    anyhow::anyhow!("effect-track segment at {} needs effects[0]", seg.start_us)
                })?;
                let entry = self.resolve_effect(&name)?;
                Self::check_params_static(entry, seg.params.as_ref())?;
            }
            _ => unreachable!(),
        }

        if track.kind == "audio" {
            for e in &seg.audio_effects {
                crate::catalogs::resolve(
                    crate::catalogs::audio_scene_effects(),
                    "audio effect",
                    &e.name,
                    self.allow_vip,
                )
                .or_else(|_| {
                    crate::catalogs::resolve(
                        crate::catalogs::tone_effects(),
                        "audio effect",
                        &e.name,
                        self.allow_vip,
                    )
                })
                .or_else(|_| {
                    crate::catalogs::resolve(
                        crate::catalogs::speech_to_songs(),
                        "audio effect",
                        &e.name,
                        self.allow_vip,
                    )
                })?;
                for (k, v) in &e.params {
                    check_range(*v, 0.0, 100.0, &format!("audio effect param {k}"))?;
                }
            }
            if let Some(f) = &seg.fade {
                if f.in_us < 0 || f.out_us < 0 {
                    bail!("fade durations must be non-negative");
                }
            }
        }

        if let Some(t) = &seg.transition_out {
            if track.kind != "video" {
                bail!("transitions are only supported on video tracks");
            }
            if last_on_main {
                bail!("a transition needs a following segment (cannot be on the last one)");
            }
            let dur = t.duration_us.unwrap_or(0);
            if dur <= 0 || dur > 1_000_000 {
                bail!("transition duration_us must be within 1..1000000");
            }
            crate::catalogs::resolve(crate::catalogs::transitions(), "transition", &t.name, self.allow_vip)?;
        }

        if let Some(m) = &seg.mask {
            if track.kind != "video" {
                bail!("masks require a video segment");
            }
            crate::catalogs::resolve(crate::catalogs::masks(), "mask", &m.name, self.allow_vip)?;
            if m.size != 0.0 {
                check_range(m.size, 0.01, 1.0, "mask size")?;
            }
            check_range(m.feather, 0.0, 100.0, "mask feather")?;
            check_range(m.rotation, -360.0, 360.0, "mask rotation")?;
            if (m.rect_width.is_some() || m.round_corner.is_some()) && m.name != "矩形" {
                bail!("rect_width/round_corner are only valid on the 矩形 mask");
            }
            if let Some(w) = m.rect_width {
                check_range(w, 0.01, 1.0, "rect_width")?;
            }
            if let Some(rc) = m.round_corner {
                check_range(rc, 0.0, 100.0, "round_corner")?;
            }
        }
        if track.kind == "video" {
            for f in &seg.filters {
                crate::catalogs::resolve(crate::catalogs::filters(), "filter", &f.name, self.allow_vip)?;
                if let Some(i) = f.intensity {
                    check_range(i, 0.0, 100.0, "filter intensity")?;
                }
            }
            for e in &seg.effects {
                let entry = self.resolve_effect(&e.name)?;
                Self::check_params_static(entry, Some(&e.params))?;
            }
            if let Some(mm) = &seg.mix_mode {
                crate::catalogs::resolve(crate::catalogs::mix_modes(), "mix mode", mm, self.allow_vip)?;
            }
        }
        for (kind, a) in [
            ("in", &seg.animation_in),
            ("out", &seg.animation_out),
            ("group", &seg.animation_group),
        ] {
            if let Some(a) = a {
                let catalog = match (track.kind.as_str(), kind) {
                    ("video", "in") => crate::catalogs::video_animations_in(),
                    ("video", "out") => crate::catalogs::video_animations_out(),
                    ("video", "group") => crate::catalogs::video_animations_group(),
                    ("text", "in") => crate::catalogs::text_animations_in(),
                    ("text", "out") => crate::catalogs::text_animations_out(),
                    ("text", "group") => crate::catalogs::text_animations_loop(),
                    _ => continue,
                };
                crate::catalogs::resolve(catalog, "animation", &a.name, self.allow_vip)?;
            }
        }
        if track.kind == "text" {
            if let Some(f) = &seg.font {
                crate::catalogs::resolve(crate::catalogs::fonts(), "font", f, self.allow_vip)?;
            }
            if let Some(b) = &seg.background {
                hex_rgb(&b.color)?;
                if let Some(a) = b.alpha {
                    check_range(a, 0.0, 1.0, "background alpha")?;
                }
                if let Some(s) = b.style {
                    if !matches!(s, 1 | 2) {
                        bail!("background style must be 1 or 2");
                    }
                }
            }
            if let Some(s) = &seg.shadow {
                if let Some(c) = &s.color {
                    hex_rgb(c)?;
                }
                if let Some(v) = s.alpha {
                    check_range(v, 0.0, 1.0, "shadow alpha")?;
                }
                if let Some(v) = s.angle {
                    check_range(v, -180.0, 180.0, "shadow angle")?;
                }
                if let Some(v) = s.distance {
                    check_range(v, 0.0, 100.0, "shadow distance")?;
                }
                if let Some(v) = s.diffuse {
                    check_range(v, 0.0, 100.0, "shadow diffuse")?;
                }
            }
            if let Some(w) = seg.border_width {
                check_range(w, 0.0, 100.0, "border_width")?;
            }
            let text = seg.text.as_deref().unwrap_or_default();
            let utf16_len = text.encode_utf16().count();
            let mut prev = 0usize;
            for r in &seg.styles {
                let [a, b] = r.range;
                if a >= b || b > utf16_len || a < prev {
                    bail!(
                        "text style range {:?} invalid for text of {utf16_len} UTF-16 units",
                        r.range
                    );
                }
                prev = b;
                if let Some(c) = &r.color {
                    hex_rgb(c)?;
                }
                if let Some(sz) = r.size {
                    check_range(sz, 1.0, 100.0, "style size")?;
                }
            }
        }

        if let Some(kfs) = &seg.keyframes {
            let allowed: &[(&str, f64, f64)] = match track.kind.as_str() {
                "video" => KEY_CHANNELS_VIDEO,
                "text" => KEY_CHANNELS_TEXT,
                _ => KEY_CHANNELS_AUDIO,
            };
            if speed != 1.0 || seg.source_start_us != 0 {
                bail!("keyframes require speed == 1 and source_start_us == 0");
            }
            for (channel, points) in kfs {
                let (_, lo, hi) = allowed
                    .iter()
                    .find(|(c, _, _)| c == channel)
                    .ok_or_else(|| {
                        anyhow::anyhow!("unsupported keyframe channel {channel} on {} track", track.kind)
                    })?;
                if points.len() < 2 {
                    bail!("keyframe channel {channel} needs at least two points");
                }
                let mut prev: i64 = -1;
                for p in points {
                    if p.at_us <= prev || p.at_us > seg.duration_us {
                        bail!("keyframe channel {channel} times must ascend within the segment");
                    }
                    prev = p.at_us;
                    check_range(p.value, *lo, *hi, &format!("keyframe {channel}"))?;
                }
                if points[0].at_us != 0 {
                    bail!("keyframe channel {channel} must start at at_us 0");
                }
                let static_conflict = match channel.as_str() {
                    "scale" => seg.scale,
                    "x" => seg.x,
                    "y" => seg.y,
                    "rotation" => seg.rotation,
                    "opacity" => seg.opacity,
                    "volume" => seg.volume,
                    _ => None,
                };
                if let Some(v) = static_conflict {
                    if (v - points[0].value).abs() > 1e-7 {
                        bail!("static {channel} conflicts with its first keyframe value");
                    }
                }
            }
        }

        if track.kind != "text" {
            if let Some(v) = seg.scale {
                check_range(v, 0.01, 10.0, "scale")?;
            }
            if let Some(v) = seg.x {
                check_range(v, -2.0, 2.0, "x")?;
            }
            if let Some(v) = seg.y {
                check_range(v, -2.0, 2.0, "y")?;
            }
            if let Some(v) = seg.rotation {
                check_range(v, -360.0, 360.0, "rotation")?;
            }
            if let Some(v) = seg.opacity {
                check_range(v, 0.0, 1.0, "opacity")?;
            }
        }
        Ok(())
    }

    fn check_params_static(entry: &Value, user: Option<&BTreeMap<String, f64>>) -> Result<()> {
        let Some(user) = user else { return Ok(()) };
        for (k, v) in user {
            check_range(*v, 0.0, 100.0, &format!("effect param {k}"))?;
            let known = entry["params"]
                .as_array()
                .map(|ps| ps.iter().any(|p| p["name"].as_str() == Some(k.as_str())))
                .unwrap_or(false);
            if !known {
                bail!("effect param {k:?} is not declared by the catalog entry");
            }
        }
        Ok(())
    }

    pub fn total_duration(&self) -> i64 {
        self.tracks
            .iter()
            .flat_map(|t| t.segments.iter())
            .map(|s| s.start_us + s.duration_us)
            .max()
            .unwrap_or(0)
    }
}

impl Default for Segment {
    fn default() -> Self {
        Segment {
            start_us: 0,
            duration_us: 0,
            source: None,
            source_start_us: 0,
            source_duration_us: None,
            speed: None,
            volume: None,
            photo: false,
            scale: None,
            x: None,
            y: None,
            rotation: None,
            opacity: None,
            keyframes: None,
            mask: None,
            filters: Vec::new(),
            effects: Vec::new(),
            mix_mode: None,
            animation_in: None,
            animation_out: None,
            animation_group: None,
            transition_out: None,
            fade: None,
            audio_effects: Vec::new(),
            text: None,
            size: None,
            color: None,
            border_color: None,
            border_width: None,
            bold: None,
            italic: None,
            underline: None,
            alignment: None,
            font: None,
            background: None,
            shadow: None,
            styles: Vec::new(),
            text_effect: None,
            sticker_id: None,
            resource_id: None,
            intensity: None,
            params: None,
        }
    }
}

/// Parse `#RRGGBB` into 0..1 floats.
pub fn hex_rgb(hex: &str) -> Result<[f64; 3]> {
    let h = hex.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        bail!("color must be #RRGGBB, got {hex:?}");
    }
    let ch = |i: usize| -> Result<f64> {
        u8::from_str_radix(&h[i * 2..i * 2 + 2], 16)
            .map(|v| v as f64 / 255.0)
            .map_err(|e| anyhow::anyhow!("bad color {hex:?}: {e}"))
    };
    Ok([ch(0)?, ch(1)?, ch(2)?])
}

//! Minimal SRT parser → subtitle intervals (for the captions pipeline).

use anyhow::{bail, Result};

#[derive(Debug, Clone, serde::Serialize)]
pub struct Cue {
    pub start_us: i64,
    pub end_us: i64,
    pub text: String,
}

fn parse_ts(s: &str) -> Result<i64> {
    let s = s.trim();
    let (hms, ms) = s
        .split_once(',')
        .ok_or_else(|| anyhow::anyhow!("bad SRT timestamp {s:?}"))?;
    let parts: Vec<&str> = hms.split(':').collect();
    if parts.len() != 3 {
        bail!("bad SRT timestamp {s:?}");
    }
    let h: i64 = parts[0].trim().parse()?;
    let m: i64 = parts[1].trim().parse()?;
    let sec: i64 = parts[2].trim().parse()?;
    let millis: i64 = ms.trim().parse()?;
    Ok(((h * 60 + m) * 60 + sec) * 1_000_000 + millis * 1_000)
}

/// Parse SRT subtitle text into cues. Blank lines separate blocks; a block is
/// `index`, `start --> end`, then one or more text lines.
pub fn parse(srt: &str) -> Result<Vec<Cue>> {
    let mut cues = Vec::new();
    for block in srt.replace("\r\n", "\n").split("\n\n") {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        if lines.len() < 2 {
            continue;
        }
        let arrow = lines
            .iter()
            .find(|l| l.contains("-->"))
            .ok_or_else(|| anyhow::anyhow!("SRT block without --> timing"))?;
        let (a, b) = arrow
            .split_once("-->")
            .ok_or_else(|| anyhow::anyhow!("bad SRT timing"))?;
        let start = parse_ts(a)?;
        let end = parse_ts(b)?;
        if end <= start {
            bail!("SRT cue ends before it starts: {arrow}");
        }
        let text = lines
            .iter()
            .filter(|l| {
                !l.contains("-->") && !l.trim().parse::<u64>().map(|_| true).unwrap_or(false)
            })
            .map(|l| l.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            continue;
        }
        cues.push(Cue {
            start_us: start,
            end_us: end,
            text,
        });
    }
    Ok(cues)
}

/// Styled subtitle options (pyJYD import_srt parity: time_offset + text_style
/// + clip_settings reduced to the fields an agent actually sets).
#[derive(Debug, Clone, serde::Deserialize, Default)]
pub struct SrtOptions {
    /// shift all cues by this offset (accepts tim() strings at the CLI layer)
    pub offset_us: i64,
    pub size: Option<f64>,
    /// 0 left / 1 center / 2 right
    pub align: Option<u8>,
    pub color: Option<String>,
    pub border_width: Option<f64>,
    /// default -0.8 (pyJYD)
    pub y: Option<f64>,
    pub bold: Option<bool>,
}

/// Convert cues into a styled text track for the plan.
pub fn cues_to_text_track(cues: Vec<Cue>, opts: &SrtOptions) -> crate::plan::Track {
    crate::plan::Track {
        kind: "text".into(),
        name: Some("字幕".into()),
        segments: cues
            .into_iter()
            .map(|c| {
                let mut s = crate::plan::Segment {
                    start_us: (c.start_us + opts.offset_us).max(0),
                    duration_us: c.end_us - c.start_us,
                    text: Some(c.text),
                    // pyJYD import_srt defaults: size 5, centered, y=-0.8
                    size: Some(opts.size.unwrap_or(5.0)),
                    alignment: Some(opts.align.unwrap_or(1)),
                    color: opts.color.clone(),
                    border_width: opts.border_width,
                    bold: opts.bold,
                    y: Some(opts.y.unwrap_or(-0.8)),
                    from_srt: true,
                    ..Default::default()
                };
                if opts.color.is_none() {
                    s.color = None;
                }
                s
            })
            .collect(),
    }
}

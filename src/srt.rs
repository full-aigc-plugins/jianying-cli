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
            .filter(|l| !l.contains("-->") && !l.trim().parse::<u64>().map(|_| true).unwrap_or(false))
            .map(|l| l.trim().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            continue;
        }
        cues.push(Cue { start_us: start, end_us: end, text });
    }
    Ok(cues)
}

/// Convert cues into a text track for the plan.
pub fn cues_to_text_track(cues: Vec<Cue>) -> crate::plan::Track {
    crate::plan::Track {
        kind: "text".into(),
        name: Some("字幕".into()),
        segments: cues
            .into_iter()
            .map(|c| crate::plan::Segment {
                start_us: c.start_us,
                duration_us: c.end_us - c.start_us,
                text: Some(c.text),
                ..Default::default()
            })
            .collect(),
    }
}

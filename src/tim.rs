//! pyJianYingDraft-compatible flexible time parsing (`tim()`).
//!
//! Plan fields ending in `_us` may be given as integers (microseconds) or as
//! strings using the same grammar pyJianYingDraft accepts: "1h52m3s",
//! "0.5s", "500ms", "1500us", compound forms, and SRT timestamps
//! ("00:01:02,500"). Preprocessing converts strings to integer microseconds
//! before deserialization.

use anyhow::{bail, Result};
use serde_json::Value;

pub fn parse(s: &str) -> Result<i64> {
    let s = s.trim();
    if s.is_empty() {
        bail!("empty time string");
    }
    // SRT style: HH:MM:SS,mmm or HH:MM:SS.mmm
    let srt = s.replace(',', ".");
    if let Ok(us) = parse_srt(&srt) {
        return Ok(us);
    }
    let re_like = |unit: &str| s.to_lowercase().contains(unit);
    if !(re_like("h") || re_like("m") || re_like("s")) {
        bail!("unrecognized time string {s:?}");
    }
    let mut total: i64 = 0;
    let mut num = String::new();
    let mut chars = s.chars().peekable();
    let mut any = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() || c == '.' {
            num.push(c);
            chars.next();
            continue;
        }
        // consume the alphabetic unit run WITHOUT swallowing the terminator
        // (Iterator::take_while would eat the first non-matching char)
        let mut unit = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_alphabetic() {
                unit.push(c);
                chars.next();
            } else {
                break;
            }
        }
        let v: f64 = num
            .parse()
            .map_err(|_| anyhow::anyhow!("bad number before {unit:?} in {s:?}"))?;
        num.clear();
        let mult: i64 = match unit.to_lowercase().as_str() {
            "h" => 3_600_000_000,
            "m" => 60_000_000,
            "s" => 1_000_000,
            "ms" => 1_000,
            "us" | "µs" => 1,
            _ => bail!("unknown time unit {unit:?} in {s:?}"),
        };
        total += (v * mult as f64).round() as i64;
        any = true;
    }
    if !any || !num.is_empty() {
        bail!("unrecognized time string {s:?}");
    }
    Ok(total)
}

fn parse_srt(s: &str) -> Result<i64> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 3 {
        bail!("not srt");
    }
    let h: f64 = parts[0].trim().parse()?;
    let m: f64 = parts[1].trim().parse()?;
    let sec: f64 = parts[2].trim().parse()?;
    Ok(((h * 3600.0 + m * 60.0 + sec) * 1_000_000.0).round() as i64)
}

/// Recursively convert string values of `*_us` keys to integer microseconds.
pub fn preprocess(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let conversions: Vec<String> = map
                .iter()
                .filter(|(k, val)| k.ends_with("_us") && matches!(val, Value::String(_)))
                .map(|(k, _)| k.clone())
                .collect();
            for k in conversions {
                if let Some(Value::String(s)) = map.get(&k) {
                    let s = s.clone();
                    if let Ok(us) = parse(&s) {
                        map.insert(k, Value::Number(us.into()));
                    }
                }
            }
            for (_k, val) in map.iter_mut() {
                preprocess(val);
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                preprocess(item);
            }
        }
        _ => {}
    }
}

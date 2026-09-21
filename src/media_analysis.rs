//! 基于 ffmpeg 与字幕的确定性场景、静音和重拍分析。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;

const US: f64 = 1_000_000.0;
const EVIDENCE_ALGORITHM: &str = "jianying-media-evidence/1.0.0";

#[derive(Clone, Copy, Debug)]
struct Span {
    start: f64,
    end: Option<f64>,
}

/// 运行 ffmpeg scene filter 并返回可直接用于剪辑的切点与分段。
pub fn scenes(
    media: &Path,
    threshold: f64,
    min_gap: f64,
    limit: Option<usize>,
    ffmpeg: Option<&Path>,
) -> Result<Value> {
    if !media.is_file() {
        bail!("detect-scenes: video not found: {}", media.display());
    }
    if !(0.0..=1.0).contains(&threshold) || min_gap < 0.0 {
        bail!("scene threshold must be 0..1 and min-gap must be >= 0");
    }
    let command = ffmpeg
        .map(Path::to_path_buf)
        .or_else(|| crate::probe::ffmpeg_path().map(Into::into))
        .context("detect-scenes: ffmpeg not found; install ffmpeg or pass --ffmpeg-cmd")?;
    let output = Command::new(&command)
        .args(["-hide_banner", "-i"])
        .arg(media)
        .args([
            "-vf",
            &format!("select='gt(scene,{threshold})',metadata=print"),
            "-an",
            "-f",
            "null",
            "-",
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "detect-scenes: ffmpeg failed on {}: {}",
            media.display(),
            stderr
        );
    }
    let probed_duration = crate::probe::probe(media)
        .ok()
        .map(|info| info.duration_us as f64 / US);
    let duration = probed_duration.or_else(|| parse_duration(&stderr));
    let mut cuts = parse_scene_cuts(&stderr);
    if let Some(bound) = duration {
        cuts.retain(|(time, _)| *time < bound);
    }
    cuts = merge_scene_cuts(cuts, min_gap);
    if let Some(limit) = limit {
        cuts.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal))
        });
        cuts.truncate(limit);
        cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    }
    let points: Vec<Value> = cuts
        .iter()
        .map(|(time, score)| {
            json!({
                "time":round6(*time),"time_us":(*time * US).round() as i64,
                "timecode":timecode(*time),"score":*score
            })
        })
        .collect();
    Ok(json!({
        "video":media,"threshold":threshold,"min_gap":min_gap,"limit":limit,
        "duration":duration.map(round6),"duration_us":duration.map(|d|(d*US).round() as i64),
        "duration_source":duration.map(|_|"container"),"cuts":points,
        "segments":scene_segments(&cuts,duration)
    }))
}

/// 运行 ffmpeg silencedetect 并返回静音段与互补保留段。
pub fn silence(
    media: &Path,
    threshold_db: f64,
    min_silence: f64,
    pad: f64,
    limit: Option<usize>,
    ffmpeg: Option<&Path>,
) -> Result<Value> {
    if !media.is_file() {
        bail!("detect-silence: media not found: {}", media.display());
    }
    if min_silence <= 0.0 || pad < 0.0 {
        bail!("min-silence must be > 0 and pad must be >= 0");
    }
    let command = ffmpeg
        .map(Path::to_path_buf)
        .or_else(|| crate::probe::ffmpeg_path().map(Into::into))
        .context("detect-silence: ffmpeg not found; install ffmpeg or pass --ffmpeg-cmd")?;
    let output = Command::new(&command)
        .args(["-hide_banner", "-i"])
        .arg(media)
        .args([
            "-af",
            &format!("silencedetect=noise={threshold_db}dB:d={min_silence}"),
            "-vn",
            "-f",
            "null",
            "-",
        ])
        .output()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "detect-silence: ffmpeg failed on {}: {}",
            media.display(),
            stderr
        );
    }
    let probed_duration = crate::probe::probe(media)
        .ok()
        .map(|info| info.duration_us as f64 / US);
    let duration = probed_duration.or_else(|| parse_duration(&stderr));
    let mut spans = close_spans(parse_silences(&stderr), duration);
    spans = spans
        .into_iter()
        .filter_map(|span| {
            let padded = Span {
                start: span.start + pad,
                end: span.end.map(|end| end - pad),
            };
            (padded.end.is_none() || padded.end.is_some_and(|end| end > padded.start))
                .then_some(padded)
        })
        .collect();
    if let Some(limit) = limit {
        spans.sort_by(|a, b| {
            span_length(*b)
                .partial_cmp(&span_length(*a))
                .unwrap_or(Ordering::Equal)
        });
        spans.truncate(limit);
        spans.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(Ordering::Equal));
    }
    let keeps = complement(&spans, duration);
    Ok(json!({
        "media":media,"threshold_db":threshold_db,"min_silence":min_silence,"pad":pad,
        "limit":limit,"duration":duration.map(round6),
        "duration_us":duration.map(|d|(d*US).round() as i64),
        "duration_source":duration.map(|_|if probed_duration.is_some(){"container"}else{"ffmpeg-header"}),
        "silences":spans.iter().map(|s|span_json(*s)).collect::<Vec<_>>(),
        "keeps":keeps.iter().map(|s|span_json(*s)).collect::<Vec<_>>()
    }))
}

/// 生成与输入内容摘要绑定的视频关键帧、静音、峰值和响度证据。
pub fn evidence(
    media: &Path,
    silence_threshold_db: f64,
    min_silence: f64,
    keyframe_limit: usize,
    ffmpeg: Option<&Path>,
    ffprobe: Option<&Path>,
) -> Result<Value> {
    if !media.is_file() {
        bail!("media-evidence: media not found: {}", media.display());
    }
    if min_silence <= 0.0 || keyframe_limit == 0 {
        bail!("min-silence and keyframe-limit must be greater than zero");
    }
    let ffprobe = ffprobe
        .map(Path::to_path_buf)
        .or_else(|| crate::probe::ffprobe_path().map(Into::into))
        .context("media-evidence: ffprobe not found; install ffmpeg or pass --ffprobe-cmd")?;
    let inventory = Command::new(&ffprobe)
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(media)
        .output()
        .with_context(|| format!("running {}", ffprobe.display()))?;
    if !inventory.status.success() {
        bail!(
            "media-evidence: ffprobe inventory failed: {}",
            String::from_utf8_lossy(&inventory.stderr).trim()
        );
    }
    let inventory: Value = serde_json::from_slice(&inventory.stdout)
        .context("media-evidence: ffprobe inventory returned invalid JSON")?;
    let streams = inventory["streams"].as_array().cloned().unwrap_or_default();
    let has_video = streams.iter().any(|stream| stream["codec_type"] == "video");
    let has_audio = streams.iter().any(|stream| stream["codec_type"] == "audio");
    let duration = inventory["format"]["duration"]
        .as_str()
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| {
            streams
                .iter()
                .find_map(|stream| stream["duration"].as_str()?.parse().ok())
        });

    let keyframes = if has_video {
        let output = Command::new(&ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-skip_frame",
                "nokey",
                "-show_frames",
                "-show_entries",
                "frame=best_effort_timestamp_time,pkt_pts_time,pict_type",
                "-of",
                "json",
            ])
            .arg(media)
            .output()?;
        if !output.status.success() {
            bail!(
                "media-evidence: keyframe probe failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        let frames: Value = serde_json::from_slice(&output.stdout)
            .context("media-evidence: keyframe probe returned invalid JSON")?;
        frames["frames"]
            .as_array()
            .into_iter()
            .flatten()
            .take(keyframe_limit)
            .filter_map(|frame| {
                let seconds = frame["best_effort_timestamp_time"]
                    .as_str()
                    .or_else(|| frame["pkt_pts_time"].as_str())?
                    .parse::<f64>()
                    .ok()?;
                Some(json!({
                    "time": round6(seconds),
                    "time_us": (seconds * US).round() as i64,
                    "timecode": timecode(seconds),
                    "picture_type": frame["pict_type"].as_str().unwrap_or("I")
                }))
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let audio = if has_audio {
        let ffmpeg = ffmpeg
            .map(Path::to_path_buf)
            .or_else(|| crate::probe::ffmpeg_path().map(Into::into))
            .context("media-evidence: ffmpeg not found; install ffmpeg or pass --ffmpeg-cmd")?;
        let filter = format!(
            "silencedetect=noise={silence_threshold_db}dB:d={min_silence},ebur128=peak=true,volumedetect"
        );
        let output = Command::new(&ffmpeg)
            .args(["-hide_banner", "-i"])
            .arg(media)
            .args(["-vn", "-af", &filter, "-f", "null", "-"])
            .output()?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !output.status.success() {
            bail!("media-evidence: ffmpeg audio analysis failed: {stderr}");
        }
        let spans = close_spans(parse_silences(&stderr), duration);
        json!({
            "present": true,
            "silences": spans.iter().map(|span| span_json(*span)).collect::<Vec<_>>(),
            "integrated_lufs": last_metric(&stderr, "I:"),
            "true_peak_dbfs": last_metric(&stderr, "Peak:"),
            "mean_volume_db": last_metric(&stderr, "mean_volume:"),
            "max_volume_db": last_metric(&stderr, "max_volume:")
        })
    } else {
        json!({
            "present": false,
            "silences": [],
            "integrated_lufs": null,
            "true_peak_dbfs": null,
            "mean_volume_db": null,
            "max_volume_db": null
        })
    };

    let metadata = std::fs::metadata(media)?;
    Ok(json!({
        "schema": "jianying-media-evidence/v1",
        "algorithm": EVIDENCE_ALGORITHM,
        "source": {
            "path": media,
            "sha256": sha256_file(media)?,
            "bytes": metadata.len()
        },
        "duration": duration.map(round6),
        "duration_us": duration.map(|value| (value * US).round() as i64),
        "video": {"present": has_video, "keyframes": keyframes, "truncated": has_video && keyframes.len() == keyframe_limit},
        "audio": audio,
        "parameters": {
            "silence_threshold_db": silence_threshold_db,
            "min_silence": min_silence,
            "keyframe_limit": keyframe_limit
        }
    }))
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn last_metric(text: &str, label: &str) -> Option<f64> {
    text.lines()
        .filter_map(|line| {
            let (_, rest) = line.rsplit_once(label)?;
            rest.split_whitespace().next()?.parse::<f64>().ok()
        })
        .next_back()
}

/// 从 SRT 中识别窗口内重复的口播尝试，默认保留后一次。
pub fn retakes(srt: &Path, window: f64, similarity: f64, min_words: usize) -> Result<Value> {
    let cues = crate::srt::parse(&std::fs::read_to_string(srt)?)?;
    retake_report(
        &cues,
        srt.to_string_lossy().into_owned(),
        None,
        window,
        similarity,
        min_words,
    )
}

/// 从已有草稿的文本轨道读取字幕并识别重拍片段。
pub fn retakes_from_draft(
    draft: &Path,
    track_name: Option<&str>,
    window: f64,
    similarity: f64,
    min_words: usize,
) -> Result<Value> {
    let timeline = crate::draft::load_timeline(draft)?;
    let text_materials = timeline["materials"]["texts"]
        .as_array()
        .context("materials.texts must be an array")?;
    let tracks: Vec<&Value> = timeline["tracks"]
        .as_array()
        .context("tracks must be an array")?
        .iter()
        .filter(|track| {
            track["type"].as_str() == Some("text")
                && track_name.is_none_or(|name| track["name"].as_str() == Some(name))
        })
        .collect();
    if tracks.is_empty() {
        bail!(
            "{}",
            track_name.map_or_else(
                || "the draft has no text track; pass --srt or add captions first".to_owned(),
                |name| format!("no text track named {name:?}")
            )
        );
    }
    let mut cues = Vec::new();
    for track in &tracks {
        for segment in track["segments"].as_array().into_iter().flatten() {
            let Some(material_id) = segment["material_id"].as_str() else {
                continue;
            };
            let Some(material) = text_materials
                .iter()
                .find(|material| material["id"].as_str() == Some(material_id))
            else {
                continue;
            };
            let Some(content) = material["content"].as_str() else {
                continue;
            };
            let text = serde_json::from_str::<Value>(content)
                .ok()
                .and_then(|value| value["text"].as_str().map(str::to_owned))
                .unwrap_or_else(|| content.to_owned());
            if text.trim().is_empty() {
                continue;
            }
            let start_us = segment["target_timerange"]["start"].as_i64().unwrap_or(0);
            let duration_us = segment["target_timerange"]["duration"]
                .as_i64()
                .unwrap_or(0);
            cues.push(crate::srt::Cue {
                start_us,
                end_us: start_us + duration_us,
                text,
            });
        }
    }
    cues.sort_by_key(|cue| cue.start_us);
    let source = tracks
        .iter()
        .filter_map(|track| track["name"].as_str())
        .collect::<Vec<_>>()
        .join(", ");
    retake_report(
        &cues,
        source,
        timeline["duration"]
            .as_i64()
            .filter(|duration| *duration > 0),
        window,
        similarity,
        min_words,
    )
}

fn retake_report(
    cues: &[crate::srt::Cue],
    source: String,
    declared_duration_us: Option<i64>,
    window: f64,
    similarity: f64,
    min_words: usize,
) -> Result<Value> {
    if window <= 0.0 || !(0.0 < similarity && similarity <= 1.0) || min_words == 0 {
        bail!("invalid retake detection parameters");
    }
    let mut pairs = Vec::new();
    let words: Vec<Vec<String>> = cues.iter().map(|cue| normalize_words(&cue.text)).collect();
    for i in 0..cues.len() {
        if words[i].len() < min_words {
            continue;
        }
        for j in i + 1..cues.len() {
            if (cues[j].start_us - cues[i].end_us) as f64 > window * US {
                break;
            }
            if words[j].len() < min_words {
                continue;
            }
            let score = sequence_similarity(&words[i], &words[j]);
            if score >= similarity {
                pairs.push(
                    json!({"earlier":cue_side(&cues[i]),"later":cue_side(&cues[j]),
                    "similarity":score,"words":words[i].len()}),
                );
                break;
            }
        }
    }
    let cuts = merge_spans(
        pairs
            .iter()
            .map(|pair| Span {
                start: pair["earlier"]["start"].as_f64().unwrap_or(0.0),
                end: pair["earlier"]["end"].as_f64(),
            })
            .collect(),
    );
    let duration_us = declared_duration_us
        .unwrap_or_else(|| cues.iter().map(|cue| cue.end_us).max().unwrap_or(0));
    let duration = (duration_us > 0).then_some(duration_us as f64 / US);
    Ok(
        json!({"source":source,"window":window,"similarity":similarity,"min_words":min_words,
        "cues":cues.len(),"duration":duration.map(round6),"duration_us":duration.map(|_|duration_us),
        "retakes":pairs,"cuts":cuts.iter().map(|s|span_json(*s)).collect::<Vec<_>>(),
        "keeps":complement(&cuts,duration).iter().map(|s|span_json(*s)).collect::<Vec<_>>() }),
    )
}

fn parse_scene_cuts(stderr: &str) -> Vec<(f64, f64)> {
    let mut pending = None;
    let mut cuts = Vec::new();
    for line in stderr.lines() {
        if let Some(value) = extract_number(line, "pts_time:") {
            pending = Some(value);
        }
        if let Some(score) = extract_number(line, "lavfi.scene_score=") {
            if let Some(time) = pending.take() {
                if time > 0.0 {
                    cuts.push((time, score));
                }
            }
        }
    }
    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    cuts
}

fn merge_scene_cuts(cuts: Vec<(f64, f64)>, gap: f64) -> Vec<(f64, f64)> {
    if cuts.len() < 2 || gap <= 0.0 {
        return cuts;
    }
    let mut out = Vec::new();
    let mut best = cuts[0];
    let mut prior = cuts[0].0;
    for cut in cuts.into_iter().skip(1) {
        if cut.0 - prior < gap {
            if cut.1 > best.1 {
                best = cut;
            }
        } else {
            out.push(best);
            best = cut;
        }
        prior = cut.0;
    }
    out.push(best);
    out
}

fn parse_silences(stderr: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut pending = None;
    for line in stderr.lines() {
        if let Some(v) = extract_number(line, "silence_start:") {
            pending = Some(v.max(0.0));
        }
        if let Some(end) = extract_number(line, "silence_end:") {
            if let Some(start) = pending.take() {
                if end > start {
                    out.push(Span {
                        start,
                        end: Some(end),
                    })
                }
            }
        }
    }
    if let Some(start) = pending {
        out.push(Span { start, end: None })
    }
    out
}

fn parse_duration(stderr: &str) -> Option<f64> {
    let marker = stderr.find("Duration:")?;
    let value = stderr[marker + 9..].trim_start().split(',').next()?;
    let mut parts = value.split(':');
    Some(
        parts.next()?.parse::<f64>().ok()? * 3600.0
            + parts.next()?.parse::<f64>().ok()? * 60.0
            + parts.next()?.parse::<f64>().ok()?,
    )
}
fn extract_number(line: &str, marker: &str) -> Option<f64> {
    let start = line.find(marker)? + marker.len();
    let token = line[start..]
        .trim_start()
        .split(|c: char| c.is_whitespace() || c == '|')
        .next()?;
    token.parse().ok()
}
fn close_spans(spans: Vec<Span>, duration: Option<f64>) -> Vec<Span> {
    match duration {
        None => spans,
        Some(bound) => spans
            .into_iter()
            .filter_map(|s| {
                (s.start < bound).then_some(Span {
                    start: s.start,
                    end: Some(s.end.unwrap_or(bound).min(bound)),
                })
            })
            .filter(|s| s.end.unwrap_or(0.0) > s.start)
            .collect(),
    }
}
fn complement(spans: &[Span], duration: Option<f64>) -> Vec<Span> {
    let mut out = Vec::new();
    let mut cursor = 0.0;
    for s in spans {
        if s.start > cursor {
            out.push(Span {
                start: cursor,
                end: Some(s.start),
            })
        }
        match s.end {
            Some(end) => cursor = cursor.max(end),
            None => return out,
        }
    }
    match duration {
        Some(end) if cursor < end => out.push(Span {
            start: cursor,
            end: Some(end),
        }),
        None => out.push(Span {
            start: cursor,
            end: None,
        }),
        _ => {}
    }
    out
}
fn merge_spans(mut spans: Vec<Span>) -> Vec<Span> {
    spans.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(Ordering::Equal));
    let mut out: Vec<Span> = Vec::new();
    for span in spans {
        if let Some(last) = out.last_mut() {
            if last.end.is_some_and(|end| span.start <= end) {
                last.end = match (last.end, span.end) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    _ => None,
                };
                continue;
            }
        }
        out.push(span)
    }
    out
}
fn span_length(span: Span) -> f64 {
    span.end
        .map(|end| end - span.start)
        .unwrap_or(f64::INFINITY)
}
fn span_json(span: Span) -> Value {
    json!({"start":round6(span.start),"end":span.end.map(round6),"duration":span.end.map(|e|round6(e-span.start)),"start_us":(span.start*US).round() as i64,"end_us":span.end.map(|e|(e*US).round() as i64),"duration_us":span.end.map(|e|((e-span.start)*US).round() as i64)})
}
fn scene_segments(cuts: &[(f64, f64)], duration: Option<f64>) -> Vec<Value> {
    let mut starts = vec![0.0];
    starts.extend(cuts.iter().map(|c| c.0));
    starts
        .iter()
        .enumerate()
        .map(|(i, start)| {
            span_json(Span {
                start: *start,
                end: starts.get(i + 1).copied().or(duration),
            })
        })
        .collect()
}
fn timecode(seconds: f64) -> String {
    let ms = (seconds * 1000.0).round() as i64;
    format!(
        "{:02}:{:02}:{:06.3}",
        ms / 3_600_000,
        (ms % 3_600_000) / 60_000,
        (ms % 60_000) as f64 / 1000.0
    )
}
fn round6(value: f64) -> f64 {
    (value * US).round() / US
}
fn normalize_words(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric())
                .flat_map(char::to_lowercase)
                .collect::<String>()
        })
        .filter(|word| !word.is_empty())
        .collect()
}
fn sequence_similarity(a: &[String], b: &[String]) -> f64 {
    let mut prev = vec![0usize; b.len() + 1];
    let mut cur = prev.clone();
    for left in a {
        for (j, right) in b.iter().enumerate() {
            cur[j + 1] = if left == right {
                prev[j] + 1
            } else {
                prev[j + 1].max(cur[j])
            }
        }
        std::mem::swap(&mut prev, &mut cur);
        cur.fill(0)
    }
    round6((2 * prev[b.len()]) as f64 / (a.len() + b.len()) as f64 * 1000.0) / 1000.0
}
fn cue_side(cue: &crate::srt::Cue) -> Value {
    json!({"text":cue.text,"start":round6(cue.start_us as f64/US),"end":round6(cue.end_us as f64/US),"start_us":cue.start_us,"end_us":cue.end_us})
}

//! FFmpeg proxy render of a built draft (capcut-cli discipline): flattens the
//! main video track, mixes audio tracks, optionally burns text segments as
//! captions. This is a PREVIEW — the authoritative render is 剪映 itself.

use crate::draft::load_timeline;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn us_to_secs(us: i64) -> String {
    format!("{:.6}", us as f64 / 1_000_000.0)
}

fn drawtext_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "\\'")
        .replace('%', "\\%")
        .replace('\n', " ")
}

/// Build and run the ffmpeg graph. Returns the output path.
pub fn render(
    draft_dir: &Path,
    out: &PathBuf,
    scale: f64,
    burn_captions: bool,
    crf: i32,
) -> Result<Value> {
    let tl = load_timeline(draft_dir)?;
    let ff =
        crate::probe::ffmpeg_path().ok_or_else(|| anyhow::anyhow!("ffmpeg not found on PATH"))?;
    let width = tl["canvas_config"]["width"].as_u64().unwrap_or(1920);
    let height = tl["canvas_config"]["height"].as_u64().unwrap_or(1080);
    let out_w = ((width as f64 * scale).round() as i64) & !1;
    let out_h = ((height as f64 * scale).round() as i64) & !1;

    let mut inputs: Vec<String> = Vec::new();
    let mut vparts: Vec<String> = Vec::new();
    let mut apart: Vec<String> = Vec::new();
    let mut input_idx = 0usize;

    let tracks = tl["tracks"].as_array().unwrap();
    let main = tracks.iter().find(|t| t["type"] == "video");
    if let Some(t) = main {
        for s in t["segments"].as_array().unwrap() {
            let path = &tl["materials"]["videos"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == s["material_id"])
                .map(|m| m["path"].as_str().unwrap_or_default().to_string())
                .unwrap_or_default();
            if path.is_empty() {
                bail!("main-track material has no path");
            }
            let src_start = s["source_timerange"]["start"].as_i64().unwrap_or(0);
            let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
            let speed = s["speed"].as_f64().unwrap_or(1.0);
            inputs.push("-i".into());
            inputs.push(path.clone());
            let is_photo = tl["materials"]["videos"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == s["material_id"])
                .map(|m| m["type"] == "photo")
                .unwrap_or(false);
            let mut chain = if is_photo {
                "loop=loop=-1:size=1:start=0".to_string()
            } else {
                format!(
                    "trim=start={}:duration={},setpts=(PTS-STARTPTS)/{}",
                    us_to_secs(src_start),
                    us_to_secs((dur as f64 * speed).round() as i64),
                    speed
                )
            };
            chain.push_str(&format!(
                ",scale={out_w}:{out_h}:force_original_aspect_ratio=decrease,pad={out_w}:{out_h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=30,format=yuv420p"
            ));
            vparts.push(format!("[v{input_idx}]{chain}"));
            input_idx += 1;
        }
    }
    if vparts.is_empty() {
        bail!("no main video track to render");
    }
    let concat_in: String = (0..input_idx).map(|i| format!("[v{i}]")).collect();
    vparts.push(format!("{concat_in}concat=n={input_idx}:v=1:a=0[vout]"));

    let mut aidx = input_idx;
    for t in tracks.iter().filter(|t| t["type"] == "audio") {
        for s in t["segments"].as_array().unwrap() {
            let mat = tl["materials"]["audios"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == s["material_id"]);
            let Some(path) = mat.and_then(|m| m["path"].as_str()) else {
                continue;
            };
            inputs.push("-i".into());
            inputs.push(path.to_string());
            let src_start = s["source_timerange"]["start"].as_i64().unwrap_or(0);
            let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
            let speed = s["speed"].as_f64().unwrap_or(1.0).clamp(0.5, 2.0);
            let volume = s["volume"].as_f64().unwrap_or(1.0);
            let delay_ms = s["target_timerange"]["start"].as_i64().unwrap_or(0) / 1_000;
            let fade = tl["materials"]["audio_fades"]
                .as_array()
                .unwrap()
                .iter()
                .find(|f| {
                    s["extra_material_refs"]
                        .as_array()
                        .map(|r| r.contains(&f["id"]))
                        .unwrap_or(false)
                });
            let (fi, fo) = fade
                .map(|f| {
                    (
                        f["fade_in_duration"].as_i64().unwrap_or(0),
                        f["fade_out_duration"].as_i64().unwrap_or(0),
                    )
                })
                .unwrap_or((0, 0));
            let mut chain = format!(
                "[{aidx}:a]atrim=start={}:duration={},asetpts=PTS-STARTPTS,atempo={}",
                us_to_secs(src_start),
                us_to_secs(dur),
                speed
            );
            if volume != 1.0 {
                chain.push_str(&format!(",volume={volume}"));
            }
            if fi > 0 {
                chain.push_str(&format!(",afade=t=in:st=0:d={}", us_to_secs(fi)));
            }
            if fo > 0 {
                chain.push_str(&format!(
                    ",afade=t=out:st={}:d={}",
                    us_to_secs((dur - fo).max(0)),
                    us_to_secs(fo)
                ));
            }
            chain.push_str(&format!(",adelay={delay_ms}|{delay_ms}[a{aidx}]"));
            apart.push(chain);
            aidx += 1;
        }
    }
    let has_audio = !apart.is_empty();
    if has_audio {
        let amix_in: String = (input_idx..aidx).map(|i| format!("[a{i}]")).collect();
        apart.push(format!(
            "{amix_in}amix=inputs={}:normalize=0[aout]",
            aidx - input_idx
        ));
    }

    let mut filter = vparts.join(";");
    if !apart.is_empty() {
        filter.push(';');
        filter.push_str(&apart.join(";"));
    }

    // optional caption burning from text segments
    let mut drawtext: Vec<String> = Vec::new();
    if burn_captions {
        for t in tracks.iter().filter(|t| t["type"] == "text") {
            for s in t["segments"].as_array().unwrap() {
                let mat = tl["materials"]["texts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|m| m["id"] == s["material_id"]);
                let Some(mat) = mat else { continue };
                let content: Value = serde_json::from_str(mat["content"].as_str().unwrap_or("{}"))?;
                let text = content["text"].as_str().unwrap_or_default();
                if text.is_empty() {
                    continue;
                }
                let start = s["target_timerange"]["start"].as_i64().unwrap_or(0);
                let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
                let font_size = (out_h as f64 / 640.0
                    * mat["font_size"].as_f64().unwrap_or(8.0).max(8.0))
                .round()
                .max(12.0);
                let y = s["clip"]["transform"]["y"].as_f64().unwrap_or(-0.6);
                let y_px = ((1.0 - (y + 1.0) / 2.0) * out_h as f64).round().max(0.0);
                drawtext.push(format!(
                    "drawtext=text='{}':fontsize={font_size}:fontcolor=white:borderw=2:bordercolor=black:x=(w-text_w)/2:y={y_px}:enable='between(t,{},{})'",
                    drawtext_escape(text),
                    us_to_secs(start),
                    us_to_secs(start + dur)
                ));
            }
        }
        if !drawtext.is_empty() {
            filter.push_str(&format!(";[vout]{}[vfinal]", drawtext.join(",")));
        }
    }
    let vmap = if burn_captions && !drawtext.is_empty() {
        "[vfinal]"
    } else {
        "[vout]"
    };

    let mut cmd = Command::new(&ff);
    cmd.arg("-y").arg("-v").arg("error");
    let mut it = inputs.iter();
    while let (Some(a), Some(b)) = (it.next(), it.next()) {
        cmd.arg(a).arg(b);
    }
    cmd.args(["-filter_complex", &filter, "-map", vmap]);
    if has_audio {
        cmd.args(["-map", "[aout]"]);
    }
    cmd.args([
        "-c:v",
        "libx264",
        "-preset",
        "veryfast",
        "-crf",
        &crf.to_string(),
        "-pix_fmt",
        "yuv420p",
    ]);
    if has_audio {
        cmd.args(["-c:a", "aac", "-b:a", "128k", "-shortest"]);
    }
    cmd.arg(out);

    let output = cmd.output().with_context(|| format!("running {ff}"))?;
    if !output.status.success() {
        bail!(
            "ffmpeg render failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(serde_json::json!({
        "status": "rendered",
        "output": out.to_string_lossy(),
        "preview": true,
        "note": "proxy render only — 转场/特效/蒙版不在此渲染；权威出口是剪映内导出",
        "video_tracks_flattened": input_idx,
        "audio_segments_mixed": if has_audio { aidx - input_idx } else { 0 },
        "captions_burned": if burn_captions { drawtext.len() } else { 0 },
    }))
}

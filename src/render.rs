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

fn material_crop_filter(material: &Value) -> Result<Option<String>> {
    let Some(crop) = material.get("crop").and_then(Value::as_object) else {
        return Ok(None);
    };
    let value = |field: &str| -> Result<f64> {
        crop.get(field)
            .and_then(Value::as_f64)
            .with_context(|| format!("video crop requires numeric {field}"))
    };
    let left = value("upper_left_x")?.min(value("lower_left_x")?);
    let right = value("upper_right_x")?.max(value("lower_right_x")?);
    let top = value("upper_left_y")?.min(value("upper_right_y")?);
    let bottom = value("lower_left_y")?.max(value("lower_right_y")?);
    let width = right - left;
    let height = bottom - top;
    if [left, top, width, height]
        .iter()
        .any(|value| !value.is_finite())
        || left < 0.0
        || top < 0.0
        || width <= 0.0
        || height <= 0.0
        || right > 1.0
        || bottom > 1.0
    {
        bail!("video crop is outside the normalized source rectangle");
    }
    if left.abs() < 1e-9
        && top.abs() < 1e-9
        && (width - 1.0).abs() < 1e-9
        && (height - 1.0).abs() < 1e-9
    {
        return Ok(None);
    }
    Ok(Some(format!(
        "crop=iw*{width:.9}:ih*{height:.9}:iw*{left:.9}:ih*{top:.9}"
    )))
}

fn segment_clip_scale(segment: &Value) -> Result<(f64, f64)> {
    let Some(scale) = segment.pointer("/clip/scale") else {
        return Ok((1.0, 1.0));
    };
    let x = scale["x"]
        .as_f64()
        .context("clip.scale.x must be numeric")?;
    let y = scale["y"]
        .as_f64()
        .context("clip.scale.y must be numeric")?;
    if [x, y]
        .iter()
        .any(|value| !value.is_finite() || !(0.01..=10.0).contains(value))
    {
        bail!("clip scale must be finite and within [0.01, 10]");
    }
    Ok((x, y))
}

fn cjk_caption_font_file() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(windows_root) = std::env::var_os("WINDIR") {
        let fonts = PathBuf::from(windows_root).join("Fonts");
        candidates.extend([
            fonts.join("msyh.ttc"),
            fonts.join("msyhbd.ttc"),
            fonts.join("simhei.ttf"),
            fonts.join("simsun.ttc"),
        ]);
    }
    candidates.extend([
        PathBuf::from("/System/Library/Fonts/Hiragino Sans GB.ttc"),
        PathBuf::from("/System/Library/Fonts/STHeiti Light.ttc"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf"),
        PathBuf::from("/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc"),
        PathBuf::from("/usr/share/fonts/truetype/arphic/uming.ttc"),
    ]);
    candidates.into_iter().find(|path| path.is_file())
}

/// Build and run the ffmpeg graph. Returns the output path.
pub fn render(
    draft_dir: &Path,
    out: &PathBuf,
    scale: f64,
    burn_captions: bool,
    crf: i32,
) -> Result<Value> {
    if !scale.is_finite() || scale <= 0.0 || scale > 4.0 {
        bail!("render scale must be finite and within (0, 4]");
    }
    if !(0..=51).contains(&crf) {
        bail!("render crf must be between 0 and 51");
    }
    let tl = load_timeline(draft_dir)?;
    let ff =
        crate::probe::ffmpeg_path().ok_or_else(|| anyhow::anyhow!("ffmpeg not found on PATH"))?;
    let width = tl["canvas_config"]["width"].as_u64().unwrap_or(1920);
    let height = tl["canvas_config"]["height"].as_u64().unwrap_or(1080);
    let fps = tl["fps"]
        .as_f64()
        .filter(|fps| fps.is_finite() && (1.0..=240.0).contains(fps))
        .context("proxy draft fps must be a finite number in [1, 240]")?;
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
            let material = tl["materials"]["videos"]
                .as_array()
                .unwrap()
                .iter()
                .find(|m| m["id"] == s["material_id"])
                .with_context(|| format!("video material {} is missing", s["material_id"]))?;
            let path = material["path"].as_str().unwrap_or_default().to_string();
            if path.is_empty() {
                bail!("main-track material has no path");
            }
            let src_start = s["source_timerange"]["start"].as_i64().unwrap_or(0);
            let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
            let speed = s["speed"].as_f64().unwrap_or(1.0);
            inputs.push("-i".into());
            inputs.push(path);
            let is_photo = material["type"] == "photo";
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
            if let Some(crop) = material_crop_filter(material)? {
                chain.push(',');
                chain.push_str(&crop);
            }
            let (scale_x, scale_y) = segment_clip_scale(s)?;
            chain.push_str(&format!(
                ",scale={out_w}:{out_h}:force_original_aspect_ratio=decrease"
            ));
            if (scale_x - 1.0).abs() > 1e-9 || (scale_y - 1.0).abs() > 1e-9 {
                // 先按画布等比适配，再还原剪映片段缩放；裁切超出画布的部分，
                // 对小于画布的结果居中补边。尺寸取偶数以满足 yuv420p/x264。
                chain.push_str(&format!(
                    ",scale=ceil(iw*{scale_x:.9}/2)*2:ceil(ih*{scale_y:.9}/2)*2,crop=min(iw\\,{out_w}):min(ih\\,{out_h}):(iw-ow)/2:(ih-oh)/2"
                ));
            }
            chain.push_str(&format!(
                ",pad={out_w}:{out_h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps:.9},format=yuv420p"
            ));
            // ffmpeg 输入标签必须绑定真实输入流；`[vN]` 只作为本段滤镜输出。
            // 若把 `[vN]` 同时当输入，代理渲染会绕过 trim 并输出完整源文件。
            vparts.push(format!("[{input_idx}:v]{chain}[v{input_idx}]"));
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
    let caption_font = burn_captions.then(cjk_caption_font_file).flatten();
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
                if !text.is_ascii() && caption_font.is_none() {
                    bail!(
                        "proxy caption text contains non-ASCII characters, but no CJK-capable system font was found"
                    );
                }
                let start = s["target_timerange"]["start"].as_i64().unwrap_or(0);
                let dur = s["target_timerange"]["duration"].as_i64().unwrap_or(0);
                let font_size = (out_h as f64 / 640.0
                    * mat["font_size"].as_f64().unwrap_or(8.0).max(8.0))
                .round()
                .max(12.0);
                let y = s["clip"]["transform"]["y"].as_f64().unwrap_or(-0.6);
                let y_px = ((1.0 - (y + 1.0) / 2.0) * out_h as f64).round().max(0.0);
                let font = caption_font
                    .as_ref()
                    .map(|path| {
                        format!(
                            "fontfile='{}':",
                            drawtext_escape(path.to_string_lossy().as_ref())
                        )
                    })
                    .unwrap_or_default();
                drawtext.push(format!(
                    "drawtext={font}text='{}':fontsize={font_size}:fontcolor=white:borderw=2:bordercolor=black:x=(w-text_w)/2:y={y_px}:enable='between(t,{},{})'",
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
        "target_fps": fps,
        "audio_segments_mixed": if has_audio { aidx - input_idx } else { 0 },
        "captions_burned": if burn_captions { drawtext.len() } else { 0 },
        "caption_font_file": caption_font,
    }))
}

/// 执行 `jianying-render-batch/v1` 清单中的多个代理渲染任务。
#[allow(clippy::too_many_arguments)]
pub fn render_batch(
    manifest_path: &Path,
    out_dir: &Path,
    scale: f64,
    burn_captions: bool,
    crf: i32,
    continue_on_error: bool,
    overwrite: bool,
) -> Result<Value> {
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(manifest_path)?)?;
    if manifest["schema"].as_str() != Some("jianying-render-batch/v1") {
        bail!("unsupported render batch schema");
    }
    let jobs = manifest["jobs"]
        .as_array()
        .context("render batch jobs must be an array")?;
    if jobs.is_empty() {
        bail!("render batch requires at least one job");
    }
    let base = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(out_dir)?;
    let mut prepared = Vec::new();
    let mut outputs = std::collections::BTreeSet::new();
    for (index, job) in jobs.iter().enumerate() {
        let draft = job["draft"]
            .as_str()
            .with_context(|| format!("render batch job {index} requires draft"))?;
        let output = job["output"]
            .as_str()
            .with_context(|| format!("render batch job {index} requires output"))?;
        let output_path = Path::new(output);
        if output_path.is_absolute()
            || output_path
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            bail!("render batch output must stay inside --out-dir: {output}");
        }
        let draft_path = Path::new(draft);
        let draft_path = if draft_path.is_absolute() {
            draft_path.to_path_buf()
        } else {
            base.join(draft_path)
        };
        crate::draft::validate_bundle(&draft_path)
            .with_context(|| format!("render batch job {index} has invalid draft"))?;
        let target = out_dir.join(output_path);
        if !outputs.insert(target.clone()) {
            bail!("duplicate render batch output: {}", target.display());
        }
        if target.exists() && !overwrite {
            bail!(
                "render batch output {} exists; pass --overwrite to replace it",
                target.display()
            );
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        prepared.push((draft_path, target));
    }

    let mut results = Vec::new();
    let mut failures = 0usize;
    for (index, (draft, output)) in prepared.iter().enumerate() {
        match render(draft, output, scale, burn_captions, crf) {
            Ok(result) => {
                results.push(serde_json::json!({"index":index,"ok":true,"result":result}))
            }
            Err(error) if continue_on_error => {
                failures += 1;
                results.push(
                    serde_json::json!({"index":index,"ok":false,"error":format!("{error:#}"),
                    "draft":draft,"output":output}),
                );
            }
            Err(error) => bail!("render batch job {index} failed: {error:#}"),
        }
    }
    Ok(
        serde_json::json!({"schema":"jianying-render-batch-result/v1","jobs":results.len(),
        "succeeded":results.len()-failures,"failed":failures,"results":results,
        "preview":true}),
    )
}

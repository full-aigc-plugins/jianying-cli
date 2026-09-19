//! ffprobe wrapper: measure real media before it enters a draft.

use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, serde::Serialize)]
pub struct MediaInfo {
    pub path: String,
    pub duration_us: i64,
    pub width: u64,
    pub height: u64,
    pub has_video: bool,
    pub has_audio: bool,
    /// Single-image file (png/jpeg/webp...). pyJianYingDraft types these as
    /// `photo` materials with a 3-hour nominal duration.
    pub is_image: bool,
}

pub fn ffprobe_path() -> Option<String> {
    which("ffprobe")
}

pub fn ffmpeg_path() -> Option<String> {
    which("ffmpeg")
}

fn which(bin: &str) -> Option<String> {
    let path = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(bin);
        if candidate.is_file() {
            return candidate.to_str().map(str::to_owned);
        }
        #[cfg(windows)]
        let candidate = dir.join(format!("{bin}.exe"));
        #[cfg(windows)]
        if candidate.is_file() {
            return candidate.to_str().map(str::to_owned);
        }
    }
    None
}

pub fn probe(media: &Path) -> Result<MediaInfo> {
    let ff = ffprobe_path()
        .ok_or_else(|| anyhow::anyhow!("ffprobe not found on PATH (brew install ffmpeg)"))?;
    let out = Command::new(&ff)
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
        .with_context(|| format!("running {ff}"))?;
    if !out.status.success() {
        bail!(
            "ffprobe failed on {}: {}",
            media.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let v: Value = serde_json::from_slice(&out.stdout)?;
    let format_name_early = v["format"]["format_name"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let is_image_early = format_name_early.contains("image")
        || (format_name_early.ends_with("_pipe")
            && !format_name_early.starts_with("mov")
            && !["mp4", "mkv", "webm", "avi", "flv", "ts"]
                .iter()
                .any(|f| format_name_early.contains(f)));
    let stream_duration = || {
        v["streams"].as_array().and_then(|ss| {
            ss.iter()
                .find_map(|s| s["duration"].as_str().and_then(|d| d.parse::<f64>().ok()))
        })
    };
    let parsed = v["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(stream_duration);
    let duration_us = match parsed {
        Some(s) => (s * 1_000_000.0).round() as i64,
        // single images carry no duration; pyJYD assigns a 3h nominal value
        None if is_image_early => 10_800_000_000,
        None => bail!("no duration reported for {}", media.display()),
    };
    let is_image = is_image_early;
    let mut info = MediaInfo {
        path: media.to_string_lossy().into_owned(),
        duration_us,
        width: 0,
        height: 0,
        has_video: false,
        has_audio: false,
        is_image,
    };
    if let Some(streams) = v["streams"].as_array() {
        for s in streams {
            match s["codec_type"].as_str() {
                Some("video") => {
                    if info.width == 0 {
                        info.width = s["width"].as_u64().unwrap_or(0);
                        info.height = s["height"].as_u64().unwrap_or(0);
                    }
                    info.has_video = true;
                }
                Some("audio") => info.has_audio = true,
                _ => {}
            }
        }
    }
    if !info.has_video && !info.has_audio && !info.is_image {
        bail!("{} has no video or audio streams", media.display());
    }
    Ok(info)
}

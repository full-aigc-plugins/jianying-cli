//! 真实剪映验收证据的失败关闭校验。

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

/// 校验一份真实编辑器验收记录及其原生导出制品。
///
/// 该校验只提升可重复验证的证据：编辑器/CLI 身份、草稿与批准绑定、来源清单、
/// 原生输出摘要和媒体流。界面观察仍必须由记录者提供，不能由文件存在性推断。
pub fn verify(
    evidence_path: &Path,
    ffprobe_cmd: Option<&Path>,
    require_human_review: bool,
) -> Result<Value> {
    let evidence_bytes = std::fs::read(evidence_path)
        .with_context(|| format!("reading acceptance evidence {}", evidence_path.display()))?;
    let document: Value = serde_json::from_slice(&evidence_bytes)
        .with_context(|| format!("parsing acceptance evidence {}", evidence_path.display()))?;

    expect_str(&document, "/schema", "jianying-video-acceptance/v1")?;
    let case_id = required_str(&document, "/case_id")?;
    let scenario = required_str(&document, "/scenario")?;
    let editor_name = required_str(&document, "/environment/editor/name")?;
    let editor_version = required_str(&document, "/environment/editor/version")?;
    let editor_build = required_str(&document, "/environment/editor/bundle_version")?;
    let runtime_identity = required_str(&document, "/environment/editor/runtime_identity")?;
    validate_prefixed_sha256(runtime_identity, "runtime_identity")?;

    let cli_version = required_str(&document, "/environment/cli/version")?;
    let release_ref = required_str(&document, "/environment/cli/release_ref")?;
    expect_str(&document, "/environment/cli/contract_state", "released")?;
    if release_ref != format!("v{cli_version}") {
        bail!("cli_release_identity_mismatch: release_ref must equal v<version>");
    }
    validate_sha256(
        required_str(&document, "/environment/cli/sha256")?,
        "cli.sha256",
    )?;

    validate_sha256(
        required_str(&document, "/workflow/draft_digest")?,
        "draft_digest",
    )?;
    required_str(&document, "/workflow/approval_id")?;
    required_str(&document, "/workflow/published_draft")?;

    for (pointer, label) in [
        ("/evidence_levels/cold_reopen", "cold_reopen"),
        (
            "/evidence_levels/continuous_playback",
            "continuous_playback",
        ),
        ("/evidence_levels/native_export", "native_export"),
    ] {
        expect_str(&document, pointer, "passed")
            .with_context(|| format!("{label} evidence is not passed"))?;
    }
    for pointer in [
        "/observations/cold_reopen",
        "/observations/continuous_playback",
        "/observations/native_export",
        "/native_output/video/export_observation",
    ] {
        required_str(&document, pointer)?;
    }

    let human_review = required_str(&document, "/evidence_levels/human_review")?;
    if require_human_review && human_review != "passed" {
        bail!("human_review_required: expected passed, found {human_review}");
    }
    let visual_review = required_str(&document, "/evidence_levels/agent_visual_review")?;
    if !matches!(visual_review, "passed" | "passed_with_minor") {
        bail!("visual_review_not_accepted: found {visual_review}");
    }

    expect_str(
        &document,
        "/source_evidence/source_attribution_status",
        "verified",
    )?;
    let source_manifest_path = PathBuf::from(required_str(&document, "/source_evidence/manifest")?);
    let source_manifest_bytes = std::fs::read(&source_manifest_path).with_context(|| {
        format!(
            "reading source provenance manifest {}",
            source_manifest_path.display()
        )
    })?;
    let source_manifest: Value = serde_json::from_slice(&source_manifest_bytes)
        .context("parsing source provenance manifest")?;
    validate_source_manifest(&source_manifest)?;

    let output_path = PathBuf::from(required_str(&document, "/native_output/video/path")?);
    let output_bytes = std::fs::read(&output_path)
        .with_context(|| format!("reading native output {}", output_path.display()))?;
    let actual_output_sha256 = sha256(&output_bytes);
    let expected_output_sha256 = required_str(&document, "/native_output/video/sha256")?;
    validate_sha256(expected_output_sha256, "native_output.video.sha256")?;
    if actual_output_sha256 != expected_output_sha256 {
        bail!("output_sha256_mismatch: native output content drifted");
    }
    let expected_size = required_u64(&document, "/native_output/video/size_bytes")?;
    if output_bytes.len() as u64 != expected_size {
        bail!(
            "output_size_mismatch: expected {expected_size}, found {}",
            output_bytes.len()
        );
    }

    let contact_sheet_path = PathBuf::from(required_str(
        &document,
        "/native_output/contact_sheet/path",
    )?);
    let contact_sheet_bytes = std::fs::read(&contact_sheet_path)
        .with_context(|| format!("reading contact sheet {}", contact_sheet_path.display()))?;
    let expected_contact_sheet_sha256 =
        required_str(&document, "/native_output/contact_sheet/sha256")?;
    validate_sha256(expected_contact_sheet_sha256, "contact_sheet.sha256")?;
    if sha256(&contact_sheet_bytes) != expected_contact_sheet_sha256 {
        bail!("contact_sheet_sha256_mismatch: visual evidence content drifted");
    }

    let probe = probe_media(&output_path, ffprobe_cmd)?;
    validate_probe(&document, &probe)?;

    Ok(json!({
        "schema":"jianying-runtime-acceptance-verification/v1",
        "result":"passed",
        "case_id":case_id,
        "scenario":scenario,
        "highest_evidence_level":"native-export",
        "human_review":human_review,
        "editor":{
            "name":editor_name,
            "version":editor_version,
            "build":editor_build,
            "runtime_identity":runtime_identity
        },
        "cli":{
            "version":cli_version,
            "release_ref":release_ref
        },
        "artifact":{
            "path":output_path,
            "sha256":actual_output_sha256,
            "size_bytes":output_bytes.len()
        },
        "source_manifest":{
            "path":source_manifest_path,
            "sha256":sha256(&source_manifest_bytes)
        },
        "evidence_sha256":sha256(&evidence_bytes),
        "checks":[
            "runtime_identity",
            "released_cli_identity",
            "draft_digest",
            "approval_id",
            "source_attribution",
            "cold_reopen_observation",
            "continuous_playback_observation",
            "native_export_observation",
            "output_sha256",
            "output_size",
            "contact_sheet_sha256",
            "video_stream",
            "audio_stream"
        ]
    }))
}

fn validate_source_manifest(document: &Value) -> Result<()> {
    for pointer in ["/license/provider", "/license/url", "/license/summary"] {
        required_str(document, pointer)?;
    }
    let assets = document
        .pointer("/assets")
        .and_then(Value::as_array)
        .filter(|assets| !assets.is_empty())
        .context("missing or empty source provenance assets")?;
    for (index, asset) in assets.iter().enumerate() {
        required_str(asset, "/id")
            .with_context(|| format!("source asset {index} is missing id"))?;
        required_str(asset, "/local_path")
            .with_context(|| format!("source asset {index} is missing local_path"))?;
        validate_sha256(
            required_str(asset, "/sha256")?,
            &format!("source asset {index} sha256"),
        )?;
    }
    Ok(())
}

fn probe_media(path: &Path, ffprobe_cmd: Option<&Path>) -> Result<Value> {
    let command = ffprobe_cmd
        .map(PathBuf::from)
        .or_else(|| crate::probe::ffprobe_path().map(PathBuf::from))
        .context("ffprobe_not_found: pass --ffprobe-cmd")?;
    let output = Command::new(&command)
        .args([
            "-v",
            "error",
            "-show_streams",
            "-show_format",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .with_context(|| format!("running {}", command.display()))?;
    if !output.status.success() {
        bail!(
            "ffprobe_failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("ffprobe_invalid_json")
}

fn validate_probe(document: &Value, probe: &Value) -> Result<()> {
    let streams = probe
        .pointer("/streams")
        .and_then(Value::as_array)
        .context("ffprobe_missing_streams")?;
    let video = streams
        .iter()
        .find(|stream| stream["codec_type"] == "video")
        .context("ffprobe_missing_video_stream")?;
    let audio = streams
        .iter()
        .find(|stream| stream["codec_type"] == "audio")
        .context("ffprobe_missing_audio_stream")?;

    compare_str(
        video,
        "/codec_name",
        required_str(document, "/native_output/video/video_codec")?,
        "video_codec_mismatch",
    )?;
    compare_u64(
        video,
        "/width",
        required_u64(document, "/native_output/video/width")?,
        "video_width_mismatch",
    )?;
    compare_u64(
        video,
        "/height",
        required_u64(document, "/native_output/video/height")?,
        "video_height_mismatch",
    )?;
    compare_str(
        audio,
        "/codec_name",
        required_str(document, "/native_output/video/audio_codec")?,
        "audio_codec_mismatch",
    )?;
    compare_u64(
        audio,
        "/channels",
        required_u64(document, "/native_output/video/audio_channels")?,
        "audio_channels_mismatch",
    )?;
    let sample_rate = required_str(audio, "/sample_rate")?
        .parse::<u64>()
        .context("invalid ffprobe sample_rate")?;
    let expected_sample_rate = required_u64(document, "/native_output/video/audio_sample_rate")?;
    if sample_rate != expected_sample_rate {
        bail!("audio_sample_rate_mismatch: expected {expected_sample_rate}, found {sample_rate}");
    }

    let duration = required_str(probe, "/format/duration")?
        .parse::<f64>()
        .context("invalid ffprobe duration")?;
    let expected_duration = required_f64(document, "/native_output/video/duration_seconds")?;
    if (duration - expected_duration).abs() > 0.05 {
        bail!("duration_mismatch: expected {expected_duration}, found {duration}");
    }
    let fps = parse_ratio(required_str(video, "/avg_frame_rate")?)?;
    let expected_fps = required_f64(document, "/native_output/video/fps")?;
    if (fps - expected_fps).abs() > 0.01 {
        bail!("fps_mismatch: expected {expected_fps}, found {fps}");
    }
    Ok(())
}

fn compare_str(document: &Value, pointer: &str, expected: &str, error_code: &str) -> Result<()> {
    let actual = required_str(document, pointer)?;
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("{error_code}: expected {expected}, found {actual}");
    }
    Ok(())
}

fn compare_u64(document: &Value, pointer: &str, expected: u64, error_code: &str) -> Result<()> {
    let actual = required_u64(document, pointer)?;
    if actual != expected {
        bail!("{error_code}: expected {expected}, found {actual}");
    }
    Ok(())
}

fn parse_ratio(value: &str) -> Result<f64> {
    let (numerator, denominator) = value
        .split_once('/')
        .context("invalid ffprobe frame rate")?;
    let numerator = numerator.parse::<f64>().context("invalid frame rate")?;
    let denominator = denominator.parse::<f64>().context("invalid frame rate")?;
    if denominator == 0.0 {
        bail!("invalid ffprobe frame rate denominator");
    }
    Ok(numerator / denominator)
}

fn required_str<'a>(document: &'a Value, pointer: &str) -> Result<&'a str> {
    document
        .pointer(pointer)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("missing or empty {pointer}"))
}

fn required_u64(document: &Value, pointer: &str) -> Result<u64> {
    document
        .pointer(pointer)
        .and_then(Value::as_u64)
        .with_context(|| format!("missing or invalid {pointer}"))
}

fn required_f64(document: &Value, pointer: &str) -> Result<f64> {
    document
        .pointer(pointer)
        .and_then(Value::as_f64)
        .with_context(|| format!("missing or invalid {pointer}"))
}

fn expect_str(document: &Value, pointer: &str, expected: &str) -> Result<()> {
    let actual = required_str(document, pointer)?;
    if actual != expected {
        bail!("invalid {pointer}: expected {expected}, found {actual}");
    }
    Ok(())
}

fn validate_sha256(value: &str, field: &str) -> Result<()> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("invalid {field}: expected 64 hexadecimal characters");
    }
    Ok(())
}

fn validate_prefixed_sha256(value: &str, field: &str) -> Result<()> {
    let digest = value
        .strip_prefix("sha256:")
        .with_context(|| format!("invalid {field}: expected sha256:<digest>"))?;
    validate_sha256(digest, field)
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("jianying-{label}-{unique}"));
    fs::create_dir_all(&path).expect("create temp dir");
    path
}

fn write_executable(path: &Path, body: &str) {
    fs::write(path, body).expect("write executable");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("chmod");
    }
}

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .expect("run jianying")
}

fn fixture(root: &Path) -> (PathBuf, PathBuf) {
    let output = root.join("native.mov");
    fs::write(&output, b"native-output").expect("write output");
    let output_sha256 = format!("{:x}", Sha256::digest(b"native-output"));
    let contact_sheet = root.join("contact-sheet.jpg");
    fs::write(&contact_sheet, b"contact-sheet").expect("write contact sheet");
    let contact_sheet_sha256 = format!("{:x}", Sha256::digest(b"contact-sheet"));

    let source_manifest = root.join("source-provenance.json");
    fs::write(
        &source_manifest,
        serde_json::to_vec_pretty(&json!({
            "case_id": "ACCEPTANCE-01",
            "license": {
                "provider": "fixture-provider",
                "url": "https://example.invalid/license",
                "summary": "fixture license"
            },
            "assets": [{
                "id": "fixture-source",
                "local_path": root.join("source.mp4"),
                "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }]
        }))
        .unwrap(),
    )
    .expect("write source provenance");

    let evidence = root.join("acceptance.json");
    fs::write(
        &evidence,
        serde_json::to_vec_pretty(&json!({
            "schema": "jianying-video-acceptance/v1",
            "case_id": "ACCEPTANCE-01",
            "scenario": "fixture",
            "status": "native_export_verified_human_review_pending",
            "created_at": "2026-09-22T03:58:00+08:00",
            "environment": {
                "os": "fixture-os",
                "editor": {
                    "name": "JianYing",
                    "version": "11.5.13243",
                    "bundle_version": "11.6.0-beta2",
                    "edition": "professional",
                    "runtime_identity": "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
                },
                "cli": {
                    "version": "1.6.19",
                    "release_ref": "v1.6.19",
                    "contract_state": "released",
                    "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                }
            },
            "workflow": {
                "job_schema": "jianying-job/v2",
                "published_draft": root.join("draft"),
                "draft_digest": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "approval_id": "approval-fixture-01"
            },
            "source_evidence": {
                "manifest": source_manifest,
                "source_attribution_status": "verified"
            },
            "evidence_levels": {
                "cold_reopen": "passed",
                "continuous_playback": "passed",
                "native_export": "passed",
                "agent_visual_review": "passed_with_minor",
                "human_review": "pending"
            },
            "observations": {
                "cold_reopen": "editor restarted and reopened the named draft",
                "continuous_playback": "timeline advanced from start to end",
                "native_export": "editor reported export success"
            },
            "native_output": {
                "video": {
                    "path": output,
                    "sha256": output_sha256,
                    "size_bytes": 13,
                    "duration_seconds": 13.0,
                    "width": 1080,
                    "height": 1920,
                    "fps": 30,
                    "video_codec": "h264",
                    "audio_codec": "aac",
                    "audio_channels": 2,
                    "audio_sample_rate": 44100,
                    "black_frames_detected": false,
                    "export_observation": "native editor reported export success"
                },
                "contact_sheet": {
                    "path": contact_sheet,
                    "sha256": contact_sheet_sha256
                }
            }
        }))
        .unwrap(),
    )
    .expect("write acceptance evidence");

    let ffprobe = root.join("ffprobe");
    write_executable(
        &ffprobe,
        r#"#!/bin/sh
cat <<'JSON'
{"format":{"duration":"13.000000","size":"13"},"streams":[{"codec_type":"video","codec_name":"h264","width":1080,"height":1920,"avg_frame_rate":"30/1"},{"codec_type":"audio","codec_name":"aac","sample_rate":"44100","channels":2}]}
JSON
"#,
    );
    (evidence, ffprobe)
}

#[test]
fn runtime_acceptance_verify_recomputes_native_artifact_and_media_identity() {
    let root = temp_dir("acceptance-pass");
    let (evidence, ffprobe) = fixture(&root);
    let output = run(&[
        "--json",
        "runtime",
        "acceptance",
        "verify",
        evidence.to_str().unwrap(),
        "--ffprobe-cmd",
        ffprobe.to_str().unwrap(),
    ]);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: Value = serde_json::from_slice(&output.stdout).expect("JSON envelope");
    assert_eq!(envelope["data"]["result"], "passed");
    assert_eq!(envelope["data"]["highest_evidence_level"], "native-export");
    assert_eq!(envelope["data"]["case_id"], "ACCEPTANCE-01");
    assert_eq!(envelope["data"]["human_review"], "pending");
}

#[test]
fn runtime_acceptance_verify_fails_closed_on_missing_binding_or_output_drift() {
    let root = temp_dir("acceptance-fail");
    let (evidence, ffprobe) = fixture(&root);
    let mut document: Value = serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
    document["workflow"]
        .as_object_mut()
        .unwrap()
        .remove("approval_id");
    fs::write(&evidence, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    let missing_approval = run(&[
        "--json",
        "runtime",
        "acceptance",
        "verify",
        evidence.to_str().unwrap(),
        "--ffprobe-cmd",
        ffprobe.to_str().unwrap(),
    ]);
    assert!(!missing_approval.status.success());
    let missing_approval_text = format!(
        "{}{}",
        String::from_utf8_lossy(&missing_approval.stdout),
        String::from_utf8_lossy(&missing_approval.stderr)
    );
    assert!(missing_approval_text.contains("approval_id"));

    let (evidence, ffprobe) = fixture(&root);
    let document: Value = serde_json::from_slice(&fs::read(&evidence).unwrap()).unwrap();
    let output_path = document["native_output"]["video"]["path"].as_str().unwrap();
    fs::write(output_path, b"tampered-output").unwrap();
    let drifted = run(&[
        "--json",
        "runtime",
        "acceptance",
        "verify",
        evidence.to_str().unwrap(),
        "--ffprobe-cmd",
        ffprobe.to_str().unwrap(),
    ]);
    assert!(!drifted.status.success());
    let drifted_text = format!(
        "{}{}",
        String::from_utf8_lossy(&drifted.stdout),
        String::from_utf8_lossy(&drifted.stderr)
    );
    assert!(drifted_text.contains("output_sha256_mismatch"));
}

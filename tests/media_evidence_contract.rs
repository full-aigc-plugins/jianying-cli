use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[test]
#[cfg(unix)]
fn evidence_is_content_bound_and_reports_keyframes_silence_peak_and_loudness() {
    let root = std::env::temp_dir().join(format!("jianying-evidence-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let media = root.join("input.mp4");
    std::fs::write(&media, b"stable fixture bytes").unwrap();
    let ffprobe = root.join("ffprobe");
    std::fs::write(&ffprobe, r#"#!/bin/sh
case "$*" in
  *-show_frames*) printf '%s\n' '{"frames":[{"best_effort_timestamp_time":"0.000000","pict_type":"I"},{"best_effort_timestamp_time":"2.500000","pict_type":"I"}]}' ;;
  *) printf '%s\n' '{"format":{"duration":"5.000000"},"streams":[{"codec_type":"video"},{"codec_type":"audio"}]}' ;;
esac
"#).unwrap();
    executable(&ffprobe);
    let ffmpeg = root.join("ffmpeg");
    std::fs::write(
        &ffmpeg,
        r#"#!/bin/sh
echo 'silence_start: 1.0' >&2
echo 'silence_end: 1.5 | silence_duration: 0.5' >&2
echo 'I: -16.2 LUFS' >&2
echo 'Peak: -1.4 dBFS' >&2
echo 'mean_volume: -19.0 dB' >&2
echo 'max_volume: -1.2 dB' >&2
"#,
    )
    .unwrap();
    executable(&ffmpeg);

    let output = run(&[
        "media",
        "evidence",
        &media.to_string_lossy(),
        "--ffprobe-cmd",
        &ffprobe.to_string_lossy(),
        "--ffmpeg-cmd",
        &ffmpeg.to_string_lossy(),
        "--json",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let data = &envelope["data"];
    assert_eq!(data["schema"], "jianying-media-evidence/v1");
    assert_eq!(data["source"]["bytes"], 20);
    assert_eq!(data["source"]["sha256"].as_str().unwrap().len(), 64);
    assert_eq!(data["video"]["keyframes"][1]["time_us"], 2_500_000);
    assert_eq!(data["audio"]["silences"][0]["start_us"], 1_000_000);
    assert_eq!(data["audio"]["integrated_lufs"], -16.2);
    assert_eq!(data["audio"]["true_peak_dbfs"], -1.4);
    assert_eq!(data["audio"]["mean_volume_db"], -19.0);
    assert_eq!(data["audio"]["max_volume_db"], -1.2);
    let _ = std::fs::remove_dir_all(root);
}

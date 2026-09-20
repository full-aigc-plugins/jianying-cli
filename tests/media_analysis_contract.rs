use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .output()
        .unwrap()
}

fn data(output: &Output) -> serde_json::Value {
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()["data"].clone()
}

#[cfg(unix)]
fn executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

#[test]
#[cfg(unix)]
fn scene_and_silence_analysis_are_deterministic_black_box_commands() {
    let root = std::env::temp_dir().join(format!("jianying-analysis-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let media = root.join("input.mp4");
    std::fs::write(&media, b"fixture").unwrap();
    let fake = root.join("fake-ffmpeg");
    std::fs::write(
        &fake,
        r##"#!/bin/sh
echo 'Duration: 00:00:06.000, start: 0.000000' >&2
case "$*" in
  *silencedetect*)
    echo 'silence_start: 1.0' >&2
    echo 'silence_end: 2.0 | silence_duration: 1.0' >&2
    echo 'silence_start: 5.0' >&2
    ;;
  *)
    echo 'frame:0 pts:1 pts_time:1.0' >&2
    echo 'lavfi.scene_score=0.5' >&2
    echo 'frame:1 pts:3 pts_time:3.0' >&2
    echo 'lavfi.scene_score=0.9' >&2
    ;;
esac
exit 0
"##,
    )
    .unwrap();
    executable(&fake);

    let scenes = data(&run(&[
        "media",
        "scenes",
        &media.to_string_lossy(),
        "--ffmpeg-cmd",
        &fake.to_string_lossy(),
        "--min-gap",
        "0",
        "--json",
    ]));
    assert_eq!(scenes["duration_us"], 6_000_000);
    assert_eq!(scenes["cuts"][0]["time_us"], 1_000_000);
    assert_eq!(scenes["segments"].as_array().unwrap().len(), 3);

    let silence = data(&run(&[
        "media",
        "silence",
        &media.to_string_lossy(),
        "--ffmpeg-cmd",
        &fake.to_string_lossy(),
        "--pad",
        "0.1",
        "--json",
    ]));
    assert_eq!(silence["silences"][0]["start_us"], 1_100_000);
    assert_eq!(silence["silences"][1]["end_us"], 5_900_000);
    assert_eq!(silence["keeps"].as_array().unwrap().len(), 3);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn retake_detection_keeps_the_later_attempt_and_emits_edit_spans() {
    let root = std::env::temp_dir().join(format!("jianying-retakes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let srt = root.join("takes.srt");
    std::fs::write(
        &srt,
        "1\n00:00:00,000 --> 00:00:02,000\nthis is the intended sentence\n\n\
         2\n00:00:03,000 --> 00:00:05,000\nthis is the intended sentence\n\n\
         3\n00:00:06,000 --> 00:00:07,000\nfinal unique line here\n",
    )
    .unwrap();
    let report = data(&run(&[
        "media",
        "retakes",
        "--srt",
        &srt.to_string_lossy(),
        "--json",
    ]));
    assert_eq!(report["retakes"].as_array().unwrap().len(), 1);
    assert_eq!(report["cuts"][0]["start_us"], 0);
    assert_eq!(report["cuts"][0]["end_us"], 2_000_000);
    assert_eq!(report["keeps"][0]["start_us"], 2_000_000);
    let _ = std::fs::remove_dir_all(root);
}

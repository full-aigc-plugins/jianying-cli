//! pyJianYingDraft 补齐合同：文本模板克隆、导入时的素材类型校验与时间窗收窄。

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_jianying"))
        .args(args)
        .env(
            "JIANYING_CLI_PROCESS_LIST",
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/empty-processes.txt"
            ),
        )
        .output()
        .unwrap()
}

fn run_ok(args: &[&str]) -> Value {
    let output = run(args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice::<Value>(&output.stdout).unwrap()["data"].clone()
}

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("jianying-parity-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn media_ready() -> bool {
    Command::new("ffmpeg").arg("-version").output().is_ok()
        && Command::new("ffprobe").arg("-version").output().is_ok()
}

fn read_timeline(draft: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(draft.join("draft_content.json")).unwrap()).unwrap()
}

fn write_timeline(draft: &Path, value: &Value) {
    fs::write(
        draft.join("draft_content.json"),
        serde_json::to_string_pretty(value).unwrap(),
    )
    .unwrap();
}

fn build_text_draft(root: &Path, name: &str, text: &str) -> PathBuf {
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":name,
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"text","segments":[
            {"start_us":0,"duration_us":1_500_000,"text":text}
        ]}]
    });
    let plan_path = root.join(format!("{name}.json"));
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join(name);
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    draft
}

fn build_video_draft(root: &Path, name: &str, duration_us: i64) -> PathBuf {
    let clip = root.join(format!("{name}.mp4"));
    let status = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=3",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .status()
        .unwrap();
    assert!(status.success());
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":name,
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":duration_us,"source":format!("{name}.mp4")}
        ]}]
    });
    let plan_path = root.join(format!("{name}.json"));
    fs::write(&plan_path, serde_json::to_string(&plan).unwrap()).unwrap();
    let draft = root.join(name);
    run_ok(&[
        "project",
        "build",
        &plan_path.to_string_lossy(),
        "--out",
        &draft.to_string_lossy(),
        "--json",
    ]);
    draft
}

#[test]
fn clone_text_copies_style_and_recalculates_ranges() {
    let root = temp_root("clone");
    // 模板文本比新文本长，样式 range 必须重算，否则草稿一致性校验会拒绝
    let draft = build_text_draft(&root, "draft", "模板文字四字");
    let segments = run_ok(&[
        "timeline",
        "segments",
        &draft.to_string_lossy(),
        "--track",
        "text",
        "--json",
    ]);
    let template_id = segments[0]["id"].as_str().unwrap().to_owned();

    let created = run_ok(&[
        "template",
        "clone-text",
        &draft.to_string_lossy(),
        "--from-segment",
        &template_id,
        "--text",
        "新字",
        "--start",
        "2000ms",
        "--json",
    ]);
    assert_eq!(created["start_us"], json!(2_000_000));
    assert_eq!(created["duration_us"], json!(1_500_000));
    assert_ne!(created["segment_id"].as_str().unwrap(), template_id);

    let timeline = read_timeline(&draft);
    let text_track = timeline["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "text")
        .unwrap();
    let segments = text_track["segments"].as_array().unwrap();
    assert_eq!(segments.len(), 2, "克隆后应有两段文本");
    let mut starts: Vec<i64> = segments
        .iter()
        .map(|segment| segment["target_timerange"]["start"].as_i64().unwrap())
        .collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![0, 2_000_000], "新段应落在请求的位置");

    let material_id = created["material_id"].as_str().unwrap();
    let material = timeline["materials"]["texts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|material| material["id"] == json!(material_id))
        .expect("克隆出的文本素材");
    let content: Value = serde_json::from_str(material["content"].as_str().unwrap()).unwrap();
    assert_eq!(content["text"], json!("新字"));
    // 越界 range 会让整个克隆失败，因此 verify 通过本身就是证据
    assert_eq!(
        run_ok(&["project", "verify", &draft.to_string_lossy(), "--json"])["ok"],
        json!(true)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_track_clamps_a_window_beyond_the_media() {
    if !media_ready() {
        return;
    }
    let root = temp_root("clamp");
    let source = build_video_draft(&root, "src", 2_000_000);
    let target = build_text_draft(&root, "dst", "占位");

    // 手工把源草稿的窗口改成超出素材时长，模拟"模板里的不可能窗口"
    let mut timeline = read_timeline(&source);
    for track in timeline["tracks"].as_array_mut().unwrap() {
        if track["type"] != "video" {
            continue;
        }
        for segment in track["segments"].as_array_mut().unwrap() {
            segment["source_timerange"]["duration"] = json!(9_000_000);
            segment["target_timerange"]["duration"] = json!(9_000_000);
        }
    }
    write_timeline(&source, &timeline);

    run_ok(&[
        "template",
        "import-track",
        &target.to_string_lossy(),
        &source.to_string_lossy(),
        "video",
        "--json",
    ]);
    let imported = read_timeline(&target);
    let video_track = imported["tracks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|track| track["type"] == "video")
        .expect("导入后的视频轨");
    assert_eq!(
        video_track["segments"][0]["source_timerange"]["duration"],
        json!(3_000_000),
        "越界窗口必须收窄到素材真实时长"
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn import_track_rejects_a_material_on_the_wrong_track_type() {
    let root = temp_root("type");
    let source = build_text_draft(&root, "text-src", "素材");
    let target = build_text_draft(&root, "target", "目标");

    // 把文本轨改造成 video 轨（保留 texts 素材），类型必然不匹配
    let mut timeline = read_timeline(&source);
    for track in timeline["tracks"].as_array_mut().unwrap() {
        if track["type"] == "text" {
            track["type"] = json!("video");
            track["name"] = json!("imported-video");
        }
    }
    write_timeline(&source, &timeline);

    let output = run(&[
        "template",
        "import-track",
        &target.to_string_lossy(),
        &source.to_string_lossy(),
        "video",
        "--json",
    ]);
    assert_eq!(output.status.code(), Some(1), "类型不匹配必须失败");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("cannot be imported onto a video track"),
        "错误信息必须说明类型不匹配: {text}"
    );
    let _ = fs::remove_dir_all(root);
}

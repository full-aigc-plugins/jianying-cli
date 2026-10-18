//! 自检：一条命令跑通目标里的六条验收线，并给出可复核的 PASS/FAIL/SKIP 矩阵。
//!
//! 与 `cargo test` 的分工：测试验证代码正确性，自检验证**这台机器上的成品能力**——
//! 用户拿到二进制后不需要源码、不需要剪映客户端，就能确认"智能剪辑"这条链是通的。
//! 依赖 ffmpeg 的检查项在缺依赖时标记 SKIP，绝不假装通过。

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Instant;

struct Check {
    id: &'static str,
    title: &'static str,
    status: &'static str,
    detail: Value,
    elapsed_ms: u128,
}

fn pass(id: &'static str, title: &'static str, detail: Value, elapsed: Instant) -> Check {
    Check {
        id,
        title,
        status: "pass",
        detail,
        elapsed_ms: elapsed.elapsed().as_millis(),
    }
}

fn fail(id: &'static str, title: &'static str, detail: Value, elapsed: Instant) -> Check {
    Check {
        id,
        title,
        status: "fail",
        detail,
        elapsed_ms: elapsed.elapsed().as_millis(),
    }
}

fn skip(id: &'static str, title: &'static str, reason: &str) -> Check {
    Check {
        id,
        title,
        status: "skip",
        detail: json!({"reason": reason}),
        elapsed_ms: 0,
    }
}

fn tool_ready(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// 执行全部自检；`workdir` 为临时工作目录（调用方负责是否保留）。
pub fn run(workdir: &Path) -> Result<Value> {
    std::fs::create_dir_all(workdir)
        .with_context(|| format!("cannot create {}", workdir.display()))?;
    let mut checks: Vec<Check> = Vec::new();

    // 1) 能力清单：入口面存在且规模合理
    let started = Instant::now();
    match crate::capabilities::manifest() {
        Ok(manifest) => {
            let entries = manifest["capabilities"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let invalid = entries
                .iter()
                .filter(|entry| {
                    ![
                        "supported",
                        "partial",
                        "external_dependency",
                        "missing",
                        "unsupported",
                    ]
                    .contains(&entry["status"].as_str().unwrap_or_default())
                })
                .count();
            let detail = json!({"entries": entries.len(), "invalid_status": invalid});
            checks.push(if entries.len() >= 100 && invalid == 0 {
                pass("capabilities", "能力清单可读且自洽", detail, started)
            } else {
                fail("capabilities", "能力清单可读且自洽", detail, started)
            });
        }
        Err(error) => checks.push(fail(
            "capabilities",
            "能力清单可读且自洽",
            json!({"error": format!("{error:#}")}),
            started,
        )),
    }

    // 2) 技能注册表：工作流映射存在
    let started = Instant::now();
    let registry = crate::skills::registry();
    let skills = registry["skills"].as_array().cloned().unwrap_or_default();
    let detail = json!({
        "skills": skills.len(),
        "supported": skills.iter().filter(|item| item["status"] == "supported").count(),
        "partial": skills.iter().filter(|item| item["status"] == "partial").count(),
        "missing": skills.iter().filter(|item| item["status"] == "missing").count(),
    });
    checks.push(if skills.len() >= 10 {
        pass("skills", "工作流技能注册表可用", detail, started)
    } else {
        fail("skills", "工作流技能注册表可用", detail, started)
    });

    // 3) MCP 工具面
    let started = Instant::now();
    let registry_tools = crate::mcp::ToolRegistry::standard();
    let tools = registry_tools.list();
    let names: Vec<&str> = tools.iter().map(|tool| tool.name()).collect();
    let detail = json!({"tools": names.len(), "names": names});
    checks.push(if tools.len() >= 15 {
        pass("mcp", "MCP 工具面完整", detail, started)
    } else {
        fail("mcp", "MCP 工具面完整", detail, started)
    });

    // 4) 计划编排：组合 + 追加 + 回滚
    let started = Instant::now();
    match plan_roundtrip(workdir.join("plan")) {
        Ok(detail) => checks.push(pass("plan", "计划可组合、可回滚", detail, started)),
        Err(error) => checks.push(fail(
            "plan",
            "计划可组合、可回滚",
            json!({"error": format!("{error:#}")}),
            started,
        )),
    }

    // 5) 草稿构建 + 结构校验（无需外部依赖）
    let started = Instant::now();
    match draft_roundtrip(workdir.join("draft")) {
        Ok(detail) => checks.push(pass("draft", "草稿可构建并通过校验", detail, started)),
        Err(error) => checks.push(fail(
            "draft",
            "草稿可构建并通过校验",
            json!({"error": format!("{error:#}")}),
            started,
        )),
    }

    // 6/7) 依赖 ffmpeg 的环节：缺依赖就 SKIP
    if !(tool_ready("ffmpeg") && tool_ready("ffprobe")) {
        checks.push(skip(
            "media",
            "素材探测与深度评估",
            "ffmpeg/ffprobe 不在 PATH 中",
        ));
        checks.push(skip(
            "render",
            "代理渲染与单帧导出",
            "ffmpeg/ffprobe 不在 PATH 中",
        ));
    } else {
        let started = Instant::now();
        match media_roundtrip(workdir.join("media")) {
            Ok(detail) => checks.push(pass("media", "素材探测与深度评估", detail, started)),
            Err(error) => checks.push(fail(
                "media",
                "素材探测与深度评估",
                json!({"error": format!("{error:#}")}),
                started,
            )),
        }
        let started = Instant::now();
        match render_roundtrip(workdir.join("render")) {
            Ok(detail) => checks.push(pass("render", "代理渲染与单帧导出", detail, started)),
            Err(error) => checks.push(fail(
                "render",
                "代理渲染与单帧导出",
                json!({"error": format!("{error:#}")}),
                started,
            )),
        }
    }

    let failures = checks.iter().filter(|check| check.status == "fail").count();
    let skipped = checks.iter().filter(|check| check.status == "skip").count();
    let passed = checks.len() - failures - skipped;
    Ok(json!({
        "schema": "jianying-selftest/v1",
        "ok": failures == 0,
        "workdir": workdir.to_string_lossy(),
        "checks": checks.iter().map(|check| json!({
            "id": check.id,
            "title": check.title,
            "status": check.status,
            "elapsed_ms": check.elapsed_ms,
            "detail": check.detail,
        })).collect::<Vec<_>>(),
        "summary": {"total": checks.len(), "pass": passed, "fail": failures, "skip": skipped},
    }))
}

fn plan_roundtrip(dir: PathBuf) -> Result<Value> {
    std::fs::create_dir_all(&dir)?;
    let a = dir.join("a.json");
    let b = dir.join("b.json");
    for (path, name, seconds) in [(&a, "a", 1_i64), (&b, "b", 2)] {
        let plan = json!({
            "schema":"jianying-cli-plan/v1","name":name,
            "canvas":{"width":320,"height":180,"fps":30},
            "tracks":[{"type":"text","segments":[
                {"start_us":0,"duration_us":seconds * 1_000_000,"text":"x"}
            ]}]
        });
        std::fs::write(path, serde_json::to_string(&plan)?)?;
    }
    let out = dir.join("out.json");
    let composed = crate::plan_ops::compose(&out, &[a.clone(), b.clone()], None, false)?;
    let appended = crate::plan_ops::append(&out, &b, None)?;
    let reverted = crate::plan_ops::revert(&out, None)?;
    let composed_us = composed["duration_us"].as_i64().unwrap_or(0);
    let appended_us = appended["duration_us"].as_i64().unwrap_or(0);
    let reverted_us = reverted["duration_us"].as_i64().unwrap_or(0);
    if composed_us != 3_000_000 || appended_us != 5_000_000 || reverted_us != 3_000_000 {
        anyhow::bail!(
            "unexpected durations: composed={composed_us} appended={appended_us} reverted={reverted_us}"
        );
    }
    Ok(json!({
        "composed_us": composed_us,
        "appended_us": appended_us,
        "reverted_us": reverted_us,
        "snapshot_used": true,
    }))
}

fn draft_roundtrip(dir: PathBuf) -> Result<Value> {
    std::fs::create_dir_all(&dir)?;
    let plan_path = dir.join("plan.json");
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"selftest",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"text","segments":[
            {"start_us":0,"duration_us":1_000_000,"text":"自检字幕"}
        ]}]
    });
    std::fs::write(&plan_path, serde_json::to_string(&plan)?)?;
    let plan = crate::plan::Plan::load(&plan_path)?;
    let out = dir.join("draft");
    let report = crate::draft::build(&plan, &dir, &out, None, &|path| crate::probe::probe(path))?;
    let verdict = crate::draft::verify(&out)?;
    if verdict["ok"] != json!(true) {
        anyhow::bail!("draft verify failed: {verdict}");
    }
    Ok(json!({
        "out_dir": report.out_dir,
        "duration_us": report.duration_us,
        "tracks": report.tracks,
        "verify": verdict["ok"],
    }))
}

fn media_roundtrip(dir: PathBuf) -> Result<Value> {
    std::fs::create_dir_all(&dir)?;
    let clip = dir.join("clip.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .status()
        .context("running ffmpeg")?;
    if !status.success() {
        anyhow::bail!("ffmpeg could not synthesise a clip");
    }
    let info = crate::probe::probe(&clip)?;
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"selftest-media",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2_000_000,"source":"clip.mp4"}
        ]}]
    });
    let plan_path = dir.join("plan.json");
    std::fs::write(&plan_path, serde_json::to_string(&plan)?)?;
    let plan = crate::plan::Plan::load(&plan_path)?;
    let out = dir.join("draft");
    crate::draft::build(&plan, &dir, &out, None, &|path| crate::probe::probe(path))?;
    let report = crate::eval_ops::evaluate_deep(&out)?;
    if report["issues"]
        .as_array()
        .map(|issues| issues.iter().any(|issue| issue["code"] == "media-missing"))
        .unwrap_or(false)
    {
        anyhow::bail!("deep eval reported media-missing on a draft we just built");
    }
    Ok(json!({
        "media_duration_us": info.duration_us,
        "score": report["score"],
        "media_checked": report["media_checked"],
    }))
}

fn render_roundtrip(dir: PathBuf) -> Result<Value> {
    std::fs::create_dir_all(&dir)?;
    let clip = dir.join("clip.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=320x180:d=2",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-y",
        ])
        .arg(&clip)
        .status()
        .context("running ffmpeg")?;
    if !status.success() {
        anyhow::bail!("ffmpeg could not synthesise a clip");
    }
    let plan = json!({
        "schema":"jianying-cli-plan/v1","name":"selftest-render",
        "canvas":{"width":320,"height":180,"fps":30},
        "tracks":[{"type":"video","segments":[
            {"start_us":0,"duration_us":2_000_000,"source":"clip.mp4"}
        ]}]
    });
    let plan_path = dir.join("plan.json");
    std::fs::write(&plan_path, serde_json::to_string(&plan)?)?;
    let plan = crate::plan::Plan::load(&plan_path)?;
    let out = dir.join("draft");
    crate::draft::build(&plan, &dir, &out, None, &|path| crate::probe::probe(path))?;

    let video = dir.join("preview.mp4");
    let rendered = crate::render::render(&out, &video, 0.5, false, 28)?;
    let frame = dir.join("frame.png");
    let still = crate::render::render_frame(&out, &frame, 500_000, 0.5)?;
    let video_bytes = std::fs::metadata(&video)
        .map(|meta| meta.len())
        .unwrap_or(0);
    let frame_bytes = std::fs::metadata(&frame)
        .map(|meta| meta.len())
        .unwrap_or(0);
    if video_bytes == 0 || frame_bytes == 0 {
        anyhow::bail!("render produced empty artefacts (video={video_bytes}, frame={frame_bytes})");
    }
    Ok(json!({
        "rendered_status": rendered["status"],
        "frame_status": still["status"],
        "video_bytes": video_bytes,
        "frame_bytes": frame_bytes,
    }))
}

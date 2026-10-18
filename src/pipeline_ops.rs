//! 批量流水线：目录里的每份计划 → 构建 → 评分 → 渲染，一次命令出多片。
//!
//! 这是"没有客户端也能批量出片"的可验收形态：每份计划独立成败，任何一步失败只影响
//! 该条目，整体结果汇总成一份 JSON；`--fail-under` 让它在 CI 里可用（有低于阈值的
//! 成片就退出码 1）。

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// 单条计划的处理结果。
struct Item {
    name: String,
    plan: String,
    score: Option<i64>,
    draft: Option<String>,
    render: Option<String>,
    issues: Vec<String>,
    error: Option<String>,
}

impl Item {
    fn ok(&self, fail_under: Option<i64>) -> bool {
        if self.error.is_some() || self.render.is_none() {
            return false;
        }
        match fail_under {
            Some(threshold) => self.score.unwrap_or(0) >= threshold,
            None => true,
        }
    }
}

/// 处理 `input` 目录下所有 `*.json` 计划，输出到 `out_root`。
pub fn run(
    input: &Path,
    out_root: &Path,
    fail_under: Option<i64>,
    scale: f64,
    crf: i32,
) -> Result<Value> {
    if !input.is_dir() {
        anyhow::bail!("pipeline input {} is not a directory", input.display());
    }
    let mut plans: Vec<PathBuf> = std::fs::read_dir(input)
        .with_context(|| format!("cannot read {}", input.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect();
    plans.sort();
    if plans.is_empty() {
        anyhow::bail!("no *.json plans found in {}", input.display());
    }

    let mut items: Vec<Item> = Vec::new();
    for plan_path in plans {
        let name = plan_path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("plan")
            .to_owned();
        let mut item = Item {
            name: name.clone(),
            plan: plan_path.to_string_lossy().into_owned(),
            score: None,
            draft: None,
            render: None,
            issues: Vec::new(),
            error: None,
        };
        match process_one(&plan_path, out_root, &name, scale, crf) {
            Ok((draft_dir, report, preview)) => {
                item.score = report["score"].as_i64();
                item.issues = report["issues"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|issue| issue["code"].as_str().map(str::to_owned))
                    .collect();
                item.draft = Some(draft_dir.to_string_lossy().into_owned());
                item.render = Some(preview.to_string_lossy().into_owned());
            }
            Err(error) => item.error = Some(format!("{error:#}")),
        }
        items.push(item);
    }

    let total = items.len();
    let succeeded = items.iter().filter(|item| item.ok(fail_under)).count();
    let payload = json!({
        "schema": "jianying-pipeline/v1",
        "input": input.to_string_lossy(),
        "out": out_root.to_string_lossy(),
        "fail_under": fail_under,
        "total": total,
        "succeeded": succeeded,
        "failed": total - succeeded,
        "items": items.iter().map(|item| json!({
            "name": item.name,
            "plan": item.plan,
            "ok": item.ok(fail_under),
            "score": item.score,
            "issues": item.issues,
            "draft": item.draft,
            "render": item.render,
            "error": item.error,
        })).collect::<Vec<_>>(),
    });
    Ok(payload)
}

fn process_one(
    plan_path: &Path,
    out_root: &Path,
    name: &str,
    scale: f64,
    crf: i32,
) -> Result<(PathBuf, Value, PathBuf)> {
    let plan = crate::plan::Plan::load(plan_path)?;
    let plan_dir = plan_path.parent().unwrap_or_else(|| Path::new("."));
    let draft_dir = out_root.join(name).join("draft");
    let preview = out_root.join(name).join("preview.mp4");
    crate::draft::build(&plan, plan_dir, &draft_dir, None, &|path| {
        crate::probe::probe(path)
    })?;
    let report = crate::eval_ops::evaluate(&draft_dir)?;
    crate::render::render(&draft_dir, &preview, scale, false, crf)?;
    Ok((draft_dir, report, preview))
}

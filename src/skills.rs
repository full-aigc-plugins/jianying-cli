//! 技能注册表：把"工作流级"的能力（字幕、配音、粗剪、出片…）映射到具体命令。
//!
//! 剪映 11.6 自带 19 个技能目录（见 `docs/reference/jianying-11.6-feature-map.md`），
//! 本模块用同一套词汇描述**本工具**能做什么，并显式标注缺口。每条技能的
//! `commands` 是 argv 前缀，合同测试会校验它们确实存在于命令目录中，防止文档漂移。

use serde_json::{json, Value};

/// 一条技能定义。
struct Skill {
    id: &'static str,
    name: &'static str,
    intent: &'static str,
    /// supported / partial / missing
    status: &'static str,
    /// argv 前缀，例如 ["timeline", "speed"]
    commands: &'static [&'static [&'static str]],
    evidence: &'static str,
    /// 缺口说明（status != supported 时必填）
    gap: &'static str,
}

const SKILLS: &[Skill] = &[
    Skill {
        id: "rough-cut",
        name: "粗剪",
        intent: "把素材按顺序铺上主轨，删除/裁剪不要的段落",
        status: "supported",
        commands: &[
            &["project", "quickstart"],
            &["timeline", "trim"],
            &["timeline", "split"],
            &["timeline", "remove"],
            &["timeline", "move"],
        ],
        evidence: "tests/timeline_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "captioning",
        name: "字幕",
        intent: "导入 SRT/ASS、改样式、翻译、导出",
        status: "supported",
        commands: &[
            &["captions", "import-srt"],
            &["captions", "style"],
            &["captions", "style-ranges"],
            &["captions", "export-ass"],
        ],
        evidence: "tests/captions_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "voiceover",
        name: "配音",
        intent: "用本地或云厂商 TTS 生成旁白并落到时间线",
        status: "supported",
        commands: &[&["media", "tts"]],
        evidence: "tests/media_mutation_contract.rs",
        gap: "",
    },
    Skill {
        id: "text-effects",
        name: "花字与气泡",
        intent: "给文本片段挂花字/气泡，或复用用户用过的花字",
        status: "supported",
        commands: &[&["resource", "apply"]],
        evidence: "tests/resource_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "audio-mix",
        name: "音频处理",
        intent: "音量、淡入淡出、变速、音效挂载",
        status: "supported",
        commands: &[
            &["timeline", "volume"],
            &["timeline", "audio-fade"],
            &["timeline", "audio-effect"],
            &["timeline", "pitch"],
        ],
        evidence: "tests/timeline_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "review",
        name: "成片评估",
        intent: "对草稿打分、列出结构化问题并自动修正结构缺陷",
        status: "supported",
        commands: &[&["eval"], &["loop"]],
        evidence: "tests/eval_loop_contract.rs",
        gap: "",
    },
    Skill {
        id: "asset-reuse",
        name: "素材复用",
        intent: "从历史草稿反演出个人素材库，按名字复用到新片",
        status: "supported",
        commands: &[&["resource", "mine"], &["resource", "apply"]],
        evidence: "tests/resource_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "template-apply",
        name: "模板套用",
        intent: "复制已有草稿、替换文本与素材、导入轨道",
        status: "supported",
        commands: &[
            &["template", "duplicate"],
            &["template", "replace-text"],
            &["template", "import-track"],
        ],
        evidence: "tests/template_cli_contract.rs",
        gap: "",
    },
    Skill {
        id: "delivery",
        name: "出片",
        intent: "结构校验后渲染预览级成片",
        status: "supported",
        commands: &[&["project", "verify"], &["render", "proxy"]],
        evidence: "docs/vision/offline-intelligent-editing.md",
        gap: "",
    },
    Skill {
        id: "batch-render",
        name: "批量出片",
        intent: "按清单批量渲染多份预览",
        status: "supported",
        commands: &[&["render", "batch"]],
        evidence: "tests/render_batch_contract.rs",
        gap: "",
    },
    Skill {
        id: "platform-adapt",
        name: "平台适配",
        intent: "按发布平台切换画布比例（抖音 9:16 / B站 16:9 / 小红书 3:4）",
        status: "partial",
        commands: &[&["project", "build"]],
        evidence: "docs/reference/jianying-11.6-feature-map.md",
        gap: "画布是计划字段，没有平台预设与自动重排；需要按平台重新排版时要手工给坐标",
    },
    Skill {
        id: "speaker-cut",
        name: "智能剪口播",
        intent: "按语句切分口播，去掉停顿与重复",
        status: "partial",
        commands: &[&["media", "transcribe"], &["timeline", "split"]],
        evidence: "tests/media_analysis_contract.rs",
        gap: "只有 ASR 时间戳与手工切分原语，没有去水词/去重复/去卡词的判定逻辑",
    },
    Skill {
        id: "beat-sync",
        name: "音乐卡点",
        intent: "按节拍点切分并重排片段",
        status: "missing",
        commands: &[],
        evidence: "docs/reference/jianying-11.6-feature-map.md",
        gap: "没有节拍检测与对齐算法；media analysis 只给响度/静音段",
    },
    Skill {
        id: "content-generation",
        name: "成片生成",
        intent: "从意图（题材/时长/平台）直接产出可剪辑的初稿",
        status: "missing",
        commands: &[],
        evidence: "docs/vision/offline-intelligent-editing.md",
        gap: "缺少意图 schema 与生成策略；现有 loop 只能修正已有草稿的结构",
    },
];

/// 全部技能的机器可读视图。
pub fn registry() -> Value {
    let skills: Vec<Value> = SKILLS.iter().map(to_value).collect();
    json!({
        "schema": "jianying-skills/v1",
        "total": skills.len(),
        "skills": skills,
    })
}

/// 按 id 或名称查找单条技能。
pub fn find(query: &str) -> Option<Value> {
    SKILLS
        .iter()
        .find(|skill| skill.id == query || skill.name == query)
        .map(to_value)
}

fn to_value(skill: &Skill) -> Value {
    json!({
        "id": skill.id,
        "name": skill.name,
        "intent": skill.intent,
        "status": skill.status,
        "commands": skill.commands
            .iter()
            .map(|argv| json!(argv.join(" ")))
            .collect::<Vec<_>>(),
        "evidence": skill.evidence,
        "gap": skill.gap,
    })
}

#[cfg(test)]
mod tests {
    use super::{registry, SKILLS};

    #[test]
    fn registry_ids_are_unique_and_gaps_are_documented() {
        let mut ids: Vec<&str> = SKILLS.iter().map(|skill| skill.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "技能 id 必须唯一");

        for skill in SKILLS {
            assert!(!skill.evidence.is_empty(), "{} 缺少证据", skill.id);
            if skill.status == "supported" {
                assert!(
                    !skill.commands.is_empty(),
                    "{} 是 supported 却没有命令",
                    skill.id
                );
                assert!(skill.gap.is_empty(), "{} 是 supported 却写了缺口", skill.id);
            } else {
                assert!(
                    !skill.gap.is_empty(),
                    "{} 非 supported 必须写缺口",
                    skill.id
                );
            }
        }
        assert_eq!(registry()["total"], serde_json::json!(SKILLS.len()));
    }
}

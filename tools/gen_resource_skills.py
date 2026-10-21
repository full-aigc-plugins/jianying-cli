#!/usr/bin/env python3
"""Generate one Agent Skill per JianYing resource type from the embedded catalogs.

An automated edit needs to pick a concrete resource before it can write an action: which transition,
which filter, which voice. This script turns `catalogs/*.json` into skills that carry the whole
inventory as a reference table - 编号 (resource_id), 名称, effect_id, VIP, 时长/参数 - plus the rules
for binding those ids into a `jianying-job/v2` action.

    python tools/gen_resource_skills.py            # write .agents/skills/jianying-<type>/
    python tools/gen_resource_skills.py --check    # fail if the generated files are stale

Every row is copied from the catalog verbatim; the script refuses to emit an id that is not in the
catalog it came from, so a table can never drift from the data.
"""
import argparse
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOGS = ROOT / "catalogs"
SKILLS = ROOT / ".agents" / "skills"

# 资源类型 -> (skill 名, 说明, [(catalog 文件, 表头说明)])
TYPES = {
    "transition": {
        "title": "转场（transitions）",
        "when": "两个片段之间要过渡时使用：叠化、闪白、推移、故障等。",
        "catalogs": [("transitions.json", "转场")],
        "job_field": "transitionSegment / segment.transition",
        "note": "`is_overlap=true` 的转场会占用前后片段的重叠时长；`duration_us` 是建议时长。",
    },
    "filter": {
        "title": "滤镜（filters）",
        "when": "要给片段整体调色时使用：复古、胶片、冷调、美食等。",
        "catalogs": [("filters.json", "滤镜")],
        "job_field": "filterEffectSegment",
        "note": "滤镜按片段生效，参数在 `params` 里；VIP 滤镜需要专业权益。",
    },
    "effect": {
        "title": "特效（video effects）",
        "when": "要加氛围、粒子、光效、人物特效时使用。",
        "catalogs": [("video_scene_effects.json", "场景特效"), ("video_character_effects.json", "人物特效")],
        "job_field": "segment.effect / filterEffectSegment",
        "note": "场景特效覆盖整段画面；人物特效依赖识别到的人体，纯素材片段可能不触发。",
    },
    "text": {
        "title": "文本与花字（fonts + text animations）",
        "when": "要写字幕、标题、花字，或给文本加入场/出场/循环动画时使用。",
        "catalogs": [
            ("fonts.json", "字体/花字"),
            ("text_animations_in.json", "文本入场动画"),
            ("text_animations_out.json", "文本出场动画"),
            ("text_animations_loop.json", "文本循环动画"),
        ],
        "job_field": "textSegment（含 font、animation）",
        "note": "文本片段必须落在 text 轨道；字体 id 与动画 id 是两套编号，不要混用。",
    },
    "audio": {
        "title": "音频（音色 + 场景音效 + 音乐）",
        "when": "要配音、变声、加场景音效或音乐时使用。",
        "catalogs": [
            ("tone_effects.json", "音色"),
            ("audio_scene_effects.json", "场景音效"),
            ("speech_to_songs.json", "歌声转换"),
        ],
        "job_field": "audioSegment / segment.tone_effect",
        "note": "音色用于改变人声；场景音效是独立素材；在线音乐不在内嵌目录里，见 `media catalog` 之外的说明。",
    },
    "sticker": {
        "title": "贴纸（stickers）",
        "when": "要加表情、箭头、边框、动效贴纸时使用。",
        "catalogs": [],
        "job_field": "stickerSegment（`resource_id` 必填）",
        "note": (
            "贴纸没有内嵌目录：剪映的贴纸是在线素材，编号来自应用侧。可用 "
            "`jianying media harvest-enums` 把应用写出的真实 id 采集到用户目录，再按 id 引用。"
        ),
    },
    "caption": {
        "title": "字幕样式（caption styles）",
        "when": "要批量生成字幕并套用样式时使用。",
        "catalogs": [],
        "job_field": "textSegment + 识别/导入的 SRT",
        "note": (
            "字幕样式不单独成目录：样式由字体 + 文本动画 + 位置组合而成。字幕文本先用 "
            "`jianying media transcribe` 得到 SRT，再逐个写 textSegment。"
        ),
    },
}


def load_catalog(name):
    path = CATALOGS / name
    if not path.is_file():
        raise SystemExit(f"missing catalog: {path}")
    entries = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(entries, list):
        raise SystemExit(f"catalog {name} is not a list")
    return entries, hashlib.sha256(path.read_bytes()).hexdigest()


def render_table(entries, label, digest):
    lines = [
        f"# {label} 全量清单",
        "",
        f"共 {len(entries)} 条，另有 VIP {sum(1 for e in entries if e.get('vip'))} 条。"
        f"数据源 `catalogs/`，sha256 `{digest[:16]}…`。",
        "",
        "| 编号 resource_id | 名称 | effect_id | VIP | 备注 |",
        "| --- | --- | --- | --- | --- |",
    ]
    for entry in sorted(entries, key=lambda e: (e.get("name") or "", e.get("resource_id") or "")):
        name = (entry.get("name") or "").replace("|", "\\|")
        extra = []
        if entry.get("duration_us"):
            extra.append(f"{entry['duration_us'] / 1_000_000:.2f}s")
        if entry.get("is_overlap"):
            extra.append("overlap")
        if entry.get("params"):
            extra.append(f"params={len(entry['params'])}")
        lines.append(
            f"| `{entry.get('resource_id')}` | {name} | `{entry.get('effect_id') or ''}` | "
            f"{'VIP' if entry.get('vip') else ''} | {' · '.join(extra)} |"
        )
    return "\n".join(lines) + "\n"


def render_skill(key, spec):
    title = spec["title"]
    rows = []
    total = 0
    for name, label in spec["catalogs"]:
        entries, digest = load_catalog(name)
        total += len(entries)
        rows.append((name.replace(".json", ""), label, len(entries), digest))

    body = [
        "---",
        f"name: jianying-{key}",
        f"description: {title} 的官方素材清单与引用规则。Use when an edit needs a concrete {key} id from the JianYing catalog before writing a jianying-job/v2 action.",
        "license: Apache-2.0",
        "metadata:",
        "  generator: tools/gen_resource_skills.py",
        "---",
        "",
        f"# {title}",
        "",
        "## 何时使用",
        "",
        spec["when"],
        "",
        "## 怎么查（不要凭记忆编编号）",
        "",
        "```bash",
        f"jianying media catalog --domain {spec['catalogs'][0][0].replace('.json', '') if spec['catalogs'] else '<domain>'} --search 关键词 --json",
        "```",
        "",
        "`--include-vip` 才会返回 VIP 素材；不带它时只返回基础素材。",
        "",
        "## 怎么用（Job v2 绑定）",
        "",
        f"字段：`{spec['job_field']}`。每个 action 至少要写清三件事：**用哪个素材（resource_id）**、"
        "**放在什么时间（start_us / duration_us）**、**什么效果参数**。",
        "",
        "```json",
        "{",
        '  "schema": "jianying-job/v2",',
        '  "project": {"name": "demo"},',
        '  "operations": [',
        "    {",
        '      "op": "add_segment",',
        '      "track": "video-1",',
        '      "segment": {"material_id": "m1", "start_us": 0, "duration_us": 2000000}',
        "    }",
        "  ]",
        "}",
        "```",
        "",
        "> 具体段类型（transition / filterEffect / sticker / text / audio）的完整字段以 "
        "`schemas/jianying-job-v2.schema.json` 为准；本技能只负责“选哪个素材”。",
        "",
        "## 注意",
        "",
        f"- {spec['note']}",
        "- 编号是 `resource_id`（16 位以上数字串），不是 `effect_id`，两者不要混用。",
        "- 表里的每一行都由脚本从 `catalogs/` 复制而来，`--check` 会拦住手工改动。",
        "",
    ]

    if rows:
        body += ["## 清单", "", "| 目录 | 内容 | 条数 | 全量表 |", "| --- | --- | --- | --- |"]
        for stem, label, count, _digest in rows:
            body.append(f"| `{stem}.json` | {label} | {count} | [references/{stem}.md](references/{stem}.md) |")
        body += ["", f"合计 **{total}** 条。", ""]
        body += ["## 先看这几条（示例，不是全部）", ""]
        entries, _ = load_catalog(spec["catalogs"][0][0])
        for entry in sorted(entries, key=lambda e: (e.get("vip") is True, e.get("name") or ""))[:8]:
            body.append(f"- `{entry.get('resource_id')}` — {entry.get('name')}"
                        + ("（VIP）" if entry.get("vip") else ""))
        body.append("")
    else:
        body += ["## 清单", "", "该类型没有内嵌目录，见下面的采集方式。", ""]

    return "\n".join(body), rows


def generate():
    outputs = {}
    for key, spec in TYPES.items():
        skill_dir = SKILLS / f"jianying-{key}"
        skill_md, rows = render_skill(key, spec)
        outputs[skill_dir / "SKILL.md"] = skill_md
        for stem, label, _count, _digest in rows:
            entries, digest = load_catalog(f"{stem}.json")
            outputs[skill_dir / "references" / f"{stem}.md"] = render_table(entries, label, digest)
    return outputs


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail when the generated files are stale")
    args = parser.parse_args()

    outputs = generate()
    stale = []
    for path, content in outputs.items():
        current = path.read_text(encoding="utf-8") if path.is_file() else None
        if current != content:
            stale.append(path)
            if not args.check:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8", newline="\n")

    if args.check:
        for path in stale:
            print(f"stale: {path.relative_to(ROOT)}", file=sys.stderr)
        print(f"{len(outputs)} files, {len(stale)} stale")
        return 1 if stale else 0

    for path in stale:
        print(f"wrote {path.relative_to(ROOT)}")
    print(f"{len(outputs)} files, {len(stale)} updated")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

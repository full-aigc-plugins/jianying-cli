#!/usr/bin/env python3
"""把上游 README 里的用法片段逐条编号，并绑定到我们的对照物。

上游 README.md 有 31 段代码块、pypi_readme.md 有 1 段。片段是"用法示意"，
不都能直接跑（有的用 `<你的草稿文件夹>` 占位），所以这里做的是**逐条登记 + 映射**：
每段都有 sha256 与行号，`maps_to` 指向对应的对照用例或行为证据；
出现 `unmapped` 时合同测试失败，逼着新增片段被显式处理。

用法：
    python tools/pyjyd_readme_snippets.py --pyjyd-src <仓库> [--check]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "examples" / "pyjyd" / "README_SNIPPETS.json"
FILES = ["README.md", "pypi_readme.md"]

# 片段内容里的关键字 -> 我们的对照物。顺序敏感：越具体越靠前。
RULES: list[tuple[tuple[str, ...], str]] = [
    (("pip install",), "meta:install"),
    (("JianyingController", "Ctrl.export_draft", "export_draft"), "out_of_scope:controller-export"),
    (("Resource id:", "Effect id:"), "reference:resource-ids"),
    (("add_keyframe",), "behavior:audio-keyframe"),
    (("import_srt",), "case:import-srt"),
    (("add_chroma", "chroma"), "case:chroma"),
    (("set_mix_mode", "MixModeType"), "case:mix-mode"),
    (("add_effect", "AudioSceneEffectType", "ToneEffectType"), "case:audio-effect"),
    (("load_template", "replace_text", "replace_material", "import_track"), "behavior:template"),
    (("DraftFolder", "create_draft", "save()"), "case:demo-tutorial"),
    (("append_track", "TrackSpec", "add_segment"), "case:scriptfile-building"),
    (("MaskType", "add_mask"), "behavior:mask"),
    (("VideoSceneEffectType", "FilterType", "add_filter"), "behavior:effect-filter"),
    (("TextIntro", "TextOutro", "TextLoopAnim", "auto_wrapping"), "behavior:text"),
    (("ClipSettings", "add_animation", "add_transition"), "behavior:visual"),
    (("timetz", "trange", "tim("), "behavior:time"),
]


def classify(body: str) -> str:
    for needles, target in RULES:
        if any(needle in body for needle in needles):
            return target
    return "unmapped"


def blocks(text: str):
    for match in re.finditer(r"```(\w*)\n(.*?)```", text, re.S):
        yield match.group(1), match.group(2), text[: match.start()].count("\n") + 1


def build(src: Path) -> dict:
    entries = []
    for name in FILES:
        path = src / name
        if not path.is_file():
            raise SystemExit(f"missing upstream file: {path}")
        for index, (lang, body, line) in enumerate(blocks(path.read_text(encoding="utf-8"))):
            entries.append(
                {
                    "file": name,
                    "index": index,
                    "lang": lang or "text",
                    "line": line,
                    "lines": len(body.splitlines()),
                    "sha256": hashlib.sha256(body.encode("utf-8")).hexdigest(),
                    "first_line": body.strip().splitlines()[0][:80] if body.strip() else "",
                    "maps_to": classify(body),
                }
            )
    return {
        "schema": "jianying-pyjyd-readme-snippets/v1",
        "upstream": {"repository": "GuanYixuan/pyJianYingDraft", "files": FILES},
        "total": len(entries),
        "snippets": entries,
        "out_of_scope_reasons": {
            "controller-export": "JianyingController 驱动剪映客户端 UI 导出，"
            "需要客户端常驻；与本仓库“无客户端”目标直接冲突"
            "（见 docs/adr/ADR-003）",
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    document = build(args.pyjyd_src)
    rendered = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != rendered:
            print("README_SNIPPETS.json is stale; rerun without --check", file=sys.stderr)
            return 1
    else:
        OUT.write_text(rendered, encoding="utf-8", newline="\n")

    unmapped = [item for item in document["snippets"] if item["maps_to"] == "unmapped"]
    # out_of_scope 必须有理由，不允许只写一个标签
    reasons = document["out_of_scope_reasons"]
    for item in document["snippets"]:
        if item["maps_to"].startswith("out_of_scope:"):
            key = item["maps_to"].split(":", 1)[1]
            if key not in reasons:
                unmapped.append({"file": item["file"], "index": item["index"], "reason": "missing rationale"})
    print(
        json.dumps(
            {
                "ok": not unmapped,
                "total": document["total"],
                "unmapped": [f'{item["file"]}#{item["index"]}' for item in unmapped],
            }
        )
    )
    return 1 if unmapped else 0


if __name__ == "__main__":
    raise SystemExit(main())

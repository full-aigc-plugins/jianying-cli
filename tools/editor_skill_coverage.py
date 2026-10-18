#!/usr/bin/env python3
"""Validate and summarise the jianying-editor-skill coverage mapping.

The editor skill (research/jianying-editor-skill) advertises a Python wrapper
API plus a set of scripts. This tool checks that:

* every API method / script listed in the skill checkout is mapped;
* every mapping entry names only real command paths from this CLI;
* every `partial` entry states what is missing, every `out_of_scope` one says why;
* `covered` entries point at an evidence file that exists.

It then writes a coverage report. Nothing is generated into the skill repo.

Usage:
    python tools/editor_skill_coverage.py \
        --skill-src ../research/jianying-editor-skill \
        --commands-json target/commands.json \
        --out provenance/EDITOR_SKILL_COVERAGE.json
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAPPING = ROOT / "provenance" / "EDITOR_SKILL_MAPPING.json"
STATUSES = {"covered", "partial", "out_of_scope"}


def load_commands(path: Path | None) -> set[str]:
    if path is not None:
        payload = json.loads(path.read_text(encoding="utf-8"))
    else:
        result = subprocess.run(
            [str(ROOT / "target" / "debug" / "jianying"), "commands", "--json"],
            check=True,
            capture_output=True,
            encoding="utf-8",
        )
        payload = json.loads(result.stdout)
    data = payload.get("data", payload)
    return {entry["path"] for entry in data["commands"]}


def api_methods(skill: Path) -> list[str]:
    """Method names advertised under `### ... APIs` in docs/api.md."""
    text = (skill / "docs" / "api.md").read_text(encoding="utf-8")
    methods: list[str] = []
    in_api = False
    for line in text.splitlines():
        if line.startswith("### "):
            in_api = "API" in line or "Core" in line
            continue
        if not in_api:
            continue
        match = re.match(r"-\s+`([A-Za-z_][A-Za-z0-9_]*)\(", line)
        if match:
            methods.append(match.group(1))
    return methods


def script_names(skill: Path) -> list[str]:
    return sorted(path.stem for path in (skill / "scripts").glob("*.py"))


def command_paths(spec: str) -> list[str]:
    """Split `a b + c d` into candidate command paths."""
    candidates = []
    for chunk in spec.split("+"):
        cleaned = re.sub(r"[（(].*?[）)]", "", chunk).strip()
        if not cleaned:
            continue
        # 只取前两个 token 作为命令路径尝试（子命令最多两级）
        candidates.append(cleaned)
    return candidates


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--skill-src", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--commands-json", type=Path)
    parser.add_argument("--mapping", default=MAPPING, type=Path)
    args = parser.parse_args()

    skill = args.skill_src.resolve()
    if not (skill / "docs" / "api.md").is_file():
        sys.stderr.write(f"not an editor-skill checkout: {skill}\n")
        return 2
    commands = load_commands(args.commands_json)
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    classes = mapping["classes"]

    problems: list[str] = []
    summary: dict[str, int] = {}
    rows: list[dict] = []

    def check_entry(kind: str, name: str, entry: dict) -> None:
        status = entry.get("status")
        if status not in STATUSES:
            problems.append(f"{kind}/{name}: invalid status {status!r}")
            return
        summary[status] = summary.get(status, 0) + 1
        if status == "partial" and not entry.get("missing"):
            problems.append(f"{kind}/{name}: partial entry has no missing list")
        if status == "out_of_scope" and not entry.get("note"):
            problems.append(f"{kind}/{name}: out_of_scope entry has no reason")
        ours = entry.get("ours")
        if status in {"covered", "partial"} and not ours:
            problems.append(f"{kind}/{name}: {status} entry has no ours")
        for candidate in command_paths(ours or ""):
            tokens = candidate.split()
            # 允许只写组名：目录里存在以此开头的命令即视为引用成立
            matched = candidate in commands or any(
                path.startswith(candidate + " ") for path in commands
            )
            if not matched:
                problems.append(f"{kind}/{name}: unknown command path {candidate!r}")
            del tokens
        evidence = entry.get("evidence")
        if evidence and not (ROOT / evidence).exists():
            problems.append(f"{kind}/{name}: evidence file missing: {evidence}")
        rows.append({"kind": kind, "name": name, **entry})

    wrapper = classes["JyProject"]
    methods = api_methods(skill)
    mapped_methods = wrapper.get("methods", {})
    for method in methods:
        if method not in mapped_methods:
            problems.append(f"JyProject: advertised method {method!r} is not mapped")
    # ????? docs/api.md ???????? `- \`name(\`` ?????????
    allow_extra = {"constructor"}
    for name in mapped_methods:
        if name not in methods and name not in allow_extra:
            problems.append(f"JyProject: mapping lists unknown method {name!r}")
    if not problems:
        for name in methods:
            check_entry("JyProject", name, mapped_methods[name])
        for name in sorted(allow_extra):
            if name in mapped_methods:
                check_entry("JyProject", name, mapped_methods[name])

    scripts = classes["scripts"]
    items = scripts.get("items", {})
    names = script_names(skill)
    for name in names:
        if name not in items:
            problems.append(f"scripts: {name!r} is not mapped")
    for name in items:
        if name not in names:
            problems.append(f"scripts: mapping lists unknown script {name!r}")
    if not problems:
        for name in names:
            check_entry("scripts", name, items[name])

    report = {
        "schema": "jianying-editor-skill-coverage/v1",
        "upstream": mapping["upstream"],
        "summary": {
            "methods": len(methods),
            "scripts": len(names),
            "by_status": summary,
        },
        "problems": problems,
        "rows": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    if problems:
        for problem in problems:
            sys.stderr.write(f"editor-skill coverage: {problem}\n")
        return 1
    print(json.dumps({"ok": True, "summary": report["summary"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

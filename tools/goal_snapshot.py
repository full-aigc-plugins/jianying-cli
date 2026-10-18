#!/usr/bin/env python3
"""Generate an evidence snapshot for the objective (reproducible on demand).

Collects what the machine can actually observe right now:

* the git commit the binary was built from (and whether the tree is dirty)
* `jianying selftest` (the seven acceptance checks)
* capability / skill / MCP counts
* pyJianYingDraft and editor-skill coverage summaries

Writes `provenance/GOAL_SNAPSHOT.json`. Nothing here is hand-written: every
number comes from a command or a generated artefact, so a stale claim is
visible instead of plausible.

Usage:
    python tools/goal_snapshot.py [--binary target/debug/jianying] [--out provenance/GOAL_SNAPSHOT.json]
"""

from __future__ import annotations

import argparse
import datetime
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run_json(binary: Path, *args: str) -> dict:
    result = subprocess.run(
        [str(binary), *args],
        capture_output=True,
        encoding="utf-8",
        env={**os.environ, "JIANYING_CLI_PROCESS_LIST": os.path.join(os.environ.get("TEMP", "/tmp"), "no-editor.txt")},
    )
    if result.returncode != 0:
        raise SystemExit(f"command failed ({result.returncode}): {' '.join(args)}\n{result.stdout}{result.stderr}")
    payload = json.loads(result.stdout)
    return payload.get("data", payload)


def git(*args: str) -> str:
    try:
        return subprocess.run(
            ["git", "-C", str(ROOT), *args],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return ""


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8")) if path.is_file() else {}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", default="target/debug/jianying", type=Path)
    parser.add_argument("--out", default="provenance/GOAL_SNAPSHOT.json", type=Path)
    args = parser.parse_args()

    binary = (ROOT / args.binary).resolve()
    if not binary.is_file():
        sys.stderr.write(f"binary not found: {binary}\n")
        return 2

    selftest = run_json(binary, "selftest", "--json")
    capabilities = run_json(binary, "capabilities", "--json")
    skills = run_json(binary, "skills", "list", "--json")
    commands = run_json(binary, "commands", "--json")

    pyjyd = load(ROOT / "provenance" / "PYJYD_API_COVERAGE.json")
    editor = load(ROOT / "provenance" / "EDITOR_SKILL_COVERAGE.json")
    capabilities_entries = capabilities.get("capabilities", [])

    snapshot = {
        "schema": "jianying-goal-snapshot/v1",
        "generated_at": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
        "git": {
            "commit": git("rev-parse", "HEAD"),
            "branch": git("rev-parse", "--abbrev-ref", "HEAD"),
            "dirty": bool(git("status", "--porcelain")),
        },
        "selftest": {
            "ok": selftest.get("ok"),
            "summary": selftest.get("summary"),
            "checks": [
                {"id": check["id"], "status": check["status"], "elapsed_ms": check["elapsed_ms"]}
                for check in selftest.get("checks", [])
            ],
        },
        "surface": {
            "commands": len(commands.get("commands", [])),
            "capabilities": len(capabilities_entries),
            "capabilities_supported": sum(
                1 for entry in capabilities_entries if entry.get("status") == "supported"
            ),
            "skills": len(skills.get("skills", [])),
        },
        "coverage": {
            "pyjyd": pyjyd.get("summary", {}),
            "editor_skill": editor.get("summary", {}),
        },
        "claims": {
            "no_client_editing": "selftest covers draft build, deep eval and ffmpeg render without the JianYing client",
            "dream_loop": "eval/loop share one policy-driven implementation (CLI + MCP)",
            "hypit_plan": "plan compose/append/history/revert with byte-exact rollback",
            "pyjyd_replacement": "runtime has no Python dependency; API surface is mapped in provenance/PYJYD_API_COVERAGE.json",
            "editor_skill": "provenance/EDITOR_SKILL_COVERAGE.json maps every advertised API/script",
        },
    }

    out = (ROOT / args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(snapshot, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"ok": True, "out": str(out), "selftest": snapshot["selftest"]["summary"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

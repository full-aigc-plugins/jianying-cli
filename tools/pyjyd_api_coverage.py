"""Generate the pyJianYingDraft public-API coverage report.

The inventory is derived from the pinned upstream checkout with `ast` only (no
import, no execution). Coverage is curated in `provenance/PYJYD_API_MAPPING.json`;
this tool merges the two so the counts cannot drift from the mapping.

Usage:
    python tools/pyjyd_api_coverage.py --pyjyd-src <path/to/pyJianYingDraft> \
        --out provenance/PYJYD_API_COVERAGE.json
"""

from __future__ import annotations

import argparse
import ast
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MAPPING = ROOT / "provenance" / "PYJYD_API_MAPPING.json"


def public(node: ast.AST, name: str) -> bool:
    return not name.startswith("_")


def inventory(source: Path) -> list[dict]:
    """Collect public classes and their public methods."""
    rows: list[dict] = []
    for path in sorted(source.glob("*.py")):
        if path.name.startswith("_"):
            continue
        tree = ast.parse(path.read_text(encoding="utf-8"))
        for node in tree.body:
            if not isinstance(node, ast.ClassDef) or not public(node, node.name):
                continue
            module = path.stem
            methods = [
                item.name
                for item in node.body
                if isinstance(item, (ast.FunctionDef, ast.AsyncFunctionDef))
                and public(item, item.name)
            ]
            rows.append({"module": module, "name": node.name, "methods": methods})
    return rows


def reference_commit(source: Path) -> str:
    out = subprocess.check_output(
        ["git", "-C", str(source.parent), "rev-parse", "HEAD"], text=True
    ).strip()
    if len(out) != 40:
        raise SystemExit(f"not a commit: {out!r}")
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--mapping", type=Path, default=MAPPING)
    args = parser.parse_args()

    source = args.pyjyd_src.resolve()
    mapping = json.loads(args.mapping.read_text(encoding="utf-8"))
    if mapping.get("schema") != "jianying-pyjyd-api-mapping/v1":
        raise SystemExit(f"unsupported mapping schema: {mapping.get('schema')!r}")
    classes: dict = mapping["classes"]

    rows: list[dict] = []
    for item in inventory(source):
        key = f"{item['module']}.{item['name']}"
        entry = classes.get(key)
        if entry is None:
            entry = {"status": "uncovered", "ours": "", "evidence": "", "missing": []}
        status = entry["status"]
        if status not in {"covered", "partial", "uncovered", "out_of_scope"}:
            raise SystemExit(f"{key}: bad status {status!r}")
        missing = set(entry.get("missing") or [])
        unknown = missing - set(item["methods"])
        if unknown:
            raise SystemExit(f"{key}: mapping lists methods that do not exist: {sorted(unknown)}")
        if status == "covered" and missing:
            raise SystemExit(f"{key}: covered classes cannot list missing methods")
        if status in {"partial", "uncovered"} and not entry.get("ours"):
            raise SystemExit(f"{key}: {status} entries must record what we do have")
        methods = [
            {
                "name": name,
                "status": "uncovered" if name in missing else ("out_of_scope" if status == "out_of_scope" else "covered"),
            }
            for name in item["methods"]
        ]
        rows.append(
            {
                "class": key,
                "module": item["module"],
                "name": item["name"],
                "status": status,
                "ours": entry.get("ours", ""),
                "evidence": entry.get("evidence", ""),
                "note": entry.get("note", ""),
                "methods": methods,
            }
        )

    extra = set(classes) - {row["class"] for row in rows}
    if extra:
        raise SystemExit(f"mapping references unknown classes: {sorted(extra)}")

    summary: dict[str, int] = {}
    for row in rows:
        summary[row["status"]] = summary.get(row["status"], 0) + 1
    method_summary: dict[str, int] = {}
    for row in rows:
        for method in row["methods"]:
            method_summary[method["status"]] = method_summary.get(method["status"], 0) + 1

    report = {
        "schema": "jianying-pyjyd-api-coverage/v1",
        "upstream": {
            "project": "GuanYixuan/pyJianYingDraft",
            "commit": reference_commit(source),
        },
        "definitions": {
            "covered": "本工具能用计划字段或命令直接表达同一语义",
            "partial": "只覆盖了类的一部分方法或语义受限，缺口写在 missing/note",
            "uncovered": "没有对应能力",
            "out_of_scope": "有意排除（客户端 UI 自动化等 L3 行为）",
        },
        "summary": {
            "classes": len(rows),
            "by_status": summary,
            "uncovered_classes": sorted(row["class"] for row in rows if row["status"] == "uncovered"),
            "methods": sum(len(row["methods"]) for row in rows),
            "methods_by_status": method_summary,
        },
        "classes": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with open(args.out, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(json.dumps(report, ensure_ascii=False, indent=2) + "\n")
    print(
        json.dumps(
            {
                "classes": report["summary"]["classes"],
                "by_status": summary,
                "methods_by_status": method_summary,
            },
            ensure_ascii=False,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())

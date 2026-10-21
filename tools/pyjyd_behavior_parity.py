#!/usr/bin/env python3
"""行为型示例对照执行器：同一组输入分别喂给 pyJianYingDraft 与 jianying-cli。

上游 5 个测试模块不产出整份草稿，只断言 API 语义。这里按
`examples/pyjyd/behavior_cases.py` 的用例表逐个执行两边，比较
`{"outcome": ..., ...}` 里的字段，结论写进
`provenance/PYJYD_BEHAVIOR_PARITY.json`。

用法：
    python tools/pyjyd_behavior_parity.py --pyjyd-src <仓库> [--bin <二进制>]
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXAMPLES = ROOT / "examples" / "pyjyd"
SCHEMA = "jianying-pyjyd-behavior-parity/v1"


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    parser.add_argument("--bin", type=Path, default=ROOT / "target" / "release" / "jianying.exe")
    parser.add_argument("--report", type=Path, default=ROOT / "provenance" / "PYJYD_BEHAVIOR_PARITY.json")
    args = parser.parse_args()

    env = dict(os.environ)
    # 用例在当前进程内运行，因此也要写回 os.environ
    os.environ["PYJYD_REPO"] = str(args.pyjyd_src.resolve())
    env["PYJYD_REPO"] = str(args.pyjyd_src.resolve())
    env["PYTHONPATH"] = str(ROOT / "tools")
    env["JIANYING_CLI_PROCESS_LIST"] = str(ROOT / "tests" / "fixtures" / "empty-processes.txt")
    env["PYTHONUTF8"] = "1"

    sys.path.insert(0, str(EXAMPLES))
    sys.path.insert(0, str(EXAMPLES / "upstream"))
    sys.path.insert(0, str(args.pyjyd_src.resolve()))
    sys.path.insert(0, str(args.pyjyd_src.resolve() / "pyJianYingDraft"))
    import behavior_cases  # noqa: PLC0415

    records = []
    with tempfile.TemporaryDirectory(prefix="pyjyd-behavior-") as temporary:
        for case in behavior_cases.CASES:
            work = Path(temporary) / case["id"]
            work.mkdir(parents=True, exist_ok=True)
            behavior_cases.prepare_media(work)
            record = {
                "id": case["id"],
                "module": case["module"],
                "covers": case["covers"],
                "status": "passed",
                "differences": [],
            }
            try:
                reference, ours = case["scenario"](args.bin.resolve(), work, env)
            except Exception as error:  # noqa: BLE001 - 用例本身出错要报出来
                record["status"] = "failed"
                record["error"] = f"{type(error).__name__}: {error}"
                records.append(record)
                continue
            approved = case.get("approved") or {}
            record["approved_differences"] = approved
            for key in case["compare"]:
                if reference.get(key) != ours.get(key):
                    if key in approved:
                        record.setdefault("approved_keys", []).append(key)
                        continue
                    record["differences"].append(
                        f"{key}: ours={ours.get(key)!r} ref={reference.get(key)!r}"
                    )
            if record["differences"]:
                record["status"] = "differs"
            record["reference"] = reference
            record["ours"] = ours
            records.append(record)

    report = {
        "schema": SCHEMA,
        "pyjyd": {
            "commit": subprocess.run(
                ["git", "-C", str(args.pyjyd_src), "rev-parse", "HEAD"],
                capture_output=True, text=True,
            ).stdout.strip(),
        },
        "summary": {
            "total": len(records),
            "passed": sum(1 for record in records if record["status"] == "passed"),
            "differs": sum(1 for record in records if record["status"] == "differs"),
            "failed": sum(1 for record in records if record["status"] == "failed"),
            "covered_tests": sorted({test for record in records for test in record["covers"]}),
        },
        "cases": records,
    }
    args.report.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    summary = report["summary"]
    print(
        json.dumps(
            {
                "ok": summary["failed"] == 0 and summary["differs"] == 0,
                "total": summary["total"],
                "passed": summary["passed"],
                "differs": summary["differs"],
                "failed": summary["failed"],
                "covered_tests": len(summary["covered_tests"]),
            }
        )
    )
    return 0 if summary["failed"] == 0 and summary["differs"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""生成 examples/pyjyd/CASES.json：逐个清点上游示例与测试，并绑定我们的对照方式。

上游 pyJianYingDraft 没有 examples/ 目录，可运行示例是 `demo.py` 与 `tests/test_*.py`。
这个脚本从上游源码解析出每一个 `def test_*`，确保新版本上游新增用例时清单不会静默漏掉：
`tests/pyjyd_examples_contract.rs` 会把清单和上游文件逐条对账。

用法：
    python tools/pyjyd_examples_manifest.py --pyjyd-src <仓库> [--check]
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "examples" / "pyjyd" / "CASES.json"
UPSTREAM = ROOT / "examples" / "pyjyd" / "upstream"

# 每个上游模块绑定到我们的对照方式。
MODULES = {
    "demo.py": {
        "category": "draft",
        "case": "cases/demo-tutorial/case.json",
        "command": "jianying project build cases/demo-tutorial/plan.json --out draft",
        "evidence": ["examples/pyjyd/cases/demo-tutorial/case.json", "tests/pyjyd_examples_contract.rs"],
        "compare": "语义比对：轨道/片段时长、源窗口、音量、速度、关键帧通道、素材引用种类",
    },
    "tests/test_scriptfile_building.py": {
        "category": "draft",
        "case": "cases/scriptfile-building/case.json",
        "command": "jianying project build cases/scriptfile-building/plan.json --out draft",
        "evidence": ["tests/plan_validation.rs", "src/plan.rs"],
        "compare": "轨道顺序、片段总时长、素材自动登记、轨道排序",
    },
    "tests/test_import_srt.py": {
        "category": "draft",
        "case": "cases/import-srt/case.json",
        "command": "jianying captions import-srt <draft> <file.srt> --track captions",
        "evidence": ["tests/captions_cli_contract.rs"],
        "compare": "SRT 解析、时间偏移、样式引用、新轨插入位置",
    },
    "tests/test_audio_effect_export.py": {
        "category": "draft",
        "case": "cases/audio-effect/case.json",
        "command": "jianying project build cases/audio-effect/plan.json --out draft",
        "evidence": ["tests/timeline_cli_contract.rs"],
        "compare": "音频特效素材字段：type/sub_type/time_range/resource_id",
    },
    "tests/test_video_chroma_export.py": {
        "category": "draft",
        "case": "cases/chroma/case.json",
        "command": "jianying project build cases/chroma/plan.json --out draft",
        "evidence": ["tests/timeline_cli_contract.rs"],
        "compare": "色度抠图素材：color/intensity/edge_smooth/shadow/spill 与 version",
    },
    "tests/test_video_mix_mode_export.py": {
        "category": "draft",
        "case": "cases/mix-mode/case.json",
        "command": "jianying project build cases/mix-mode/plan.json --out draft",
        "evidence": ["src/plan.rs"],
        "compare": "重复设置混合同模式时只保留最后一次",
    },
    "tests/test_template_loading.py": {
        "category": "behavior",
        "command": "jianying template list|import-track|replace-text|replace-material",
        "evidence": ["tests/template_cli_contract.rs", "tests/template_semantics_contract.rs"],
        "compare": "模板加载容错、导入轨道顺序、类型过滤、显式插入位置",
    },
    "tests/test_track_append_api.py": {
        "category": "behavior",
        "command": "jianying timeline add-track <draft> <type> --name <name>",
        "evidence": ["tests/timeline_cli_contract.rs"],
        "compare": "追加轨道返回引用、同类型第二轨必须命名、片段必须属于本草稿",
    },
    "tests/test_track_insert_api.py": {
        "category": "behavior",
        "command": "jianying timeline add-track <draft> <type> --index <n>",
        "evidence": ["tests/timeline_cli_contract.rs"],
        "compare": "按参考轨道/索引插入、拒绝多重定位、越界索引",
    },
    "tests/test_track_basics.py": {
        "category": "behavior",
        "command": "jianying project build（计划校验：重叠即拒绝）",
        "evidence": ["tests/plan_validation.rs"],
        "compare": "重叠拒绝（含 1µs 边界）、类型匹配、秒级节拍对齐、轨道结束时间",
    },
    "tests/test_time_util.py": {
        "category": "behavior",
        "command": "所有时间参数（0s / 1500ms / 1000000us）",
        "evidence": ["tests/ops_behavior.rs"],
        "compare": "tim/trange 解析与取整：负数偏移、duration 语义、绝对边界四舍五入",
    },
}


def test_functions(path: Path) -> list[str]:
    text = path.read_text(encoding="utf-8")
    return re.findall(r"^def (test_\w+)", text, re.M)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    commit = subprocess.run(
        ["git", "-C", str(args.pyjyd_src), "rev-parse", "HEAD"],
        capture_output=True, text=True,
    ).stdout.strip()

    cases = []
    for source, spec in MODULES.items():
        upstream_path = UPSTREAM / source
        if not upstream_path.is_file():
            print(f"missing copied upstream file: {source}", file=sys.stderr)
            return 1
        tests = test_functions(upstream_path)
        spec = dict(spec)
        if spec.get("category") == "draft":
            spec["status"] = "compared"
        elif spec.get("category") == "behavior":
            spec["status"] = "compared"
            spec["artifact"] = "provenance/PYJYD_BEHAVIOR_PARITY.json"
        # 用例 id 取 case.json 所在目录名，与 provenance 产物里的 id 保持一致
        case_id = Path(spec["case"]).parent.name if spec.get("case") else Path(source).stem
        case = {
            "id": case_id,
            "source": source,
            "upstream_tests": tests,
            **spec,
        }
        cases.append(case)

    document = {
        "schema": "jianying-pyjyd-examples/v1",
        "upstream": {
            "repository": "GuanYixuan/pyJianYingDraft",
            "commit": commit,
            "license": "Apache-2.0",
            "copied_to": "examples/pyjyd/upstream/",
        },
        "total_tests": sum(len(case["upstream_tests"]) for case in cases),
        "readme_snippets": "examples/pyjyd/README_SNIPPETS.json",
        "cases": cases,
    }

    rendered = json.dumps(document, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != rendered:
            print("CASES.json is stale; rerun without --check", file=sys.stderr)
            return 1
        print(json.dumps({"ok": True, "cases": len(cases), "tests": document["total_tests"]}))
        return 0

    OUT.write_text(rendered, encoding="utf-8", newline="\n")
    print(json.dumps({"ok": True, "cases": len(cases), "tests": document["total_tests"]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

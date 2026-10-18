#!/usr/bin/env python3
"""pyJianYingDraft 示例对照执行器。

对 `examples/pyjyd/cases/*/case.json` 里每个示例：

1. 按 `media` 声明的 ffmpeg 参数生成等价合成素材；
2. 跑 `reference`（逐个复刻上游示例的 pyJianYingDraft 调用）产出参照草稿；
3. 跑 `jianying project build`（外加 `post_build` 里的 CLI 命令）产出我们的草稿；
4. 对两份草稿做语义比对（忽略 id/时间戳/绝对路径），把结论写进
   `provenance/PYJYD_EXAMPLES_PARITY.json`。

用法：
    python tools/pyjyd_examples_parity.py --pyjyd-src <pyJianYingDraft 仓库>
                                         [--bin target/release/jianying.exe]
                                         [--report provenance/PYJYD_EXAMPLES_PARITY.json]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CASES = ROOT / "examples" / "pyjyd" / "cases"
SCHEMA = "jianying-pyjyd-examples-parity/v1"

VOLATILE = {
    "id", "material_id", "raw_segment_id", "local_material_id", "music_id",
    "draft_id", "create_time", "update_time", "tm_draft_create",
    "tm_draft_modified", "import_time", "import_time_ms", "md5",
}


def run(argv: list[str], *, env: dict | None = None) -> str:
    process = subprocess.run(argv, capture_output=True, text=True, env=env)
    if process.returncode != 0:
        raise RuntimeError(
            f"command failed ({process.returncode}): {' '.join(argv)}\n"
            f"stdout={process.stdout}\nstderr={process.stderr}"
        )
    return process.stdout


def load_draft(path: Path) -> dict:
    return json.loads((path / "draft_content.json").read_text(encoding="utf-8"))


def bucket_of(draft: dict) -> dict[str, str]:
    """materials 桶名 -> 素材 id，用于把 extra_material_refs 翻译成可比的种类。"""
    mapping: dict[str, str] = {}
    for kind, items in (draft.get("materials") or {}).items():
        for item in items or []:
            identifier = item.get("id")
            if identifier:
                mapping[identifier] = kind
    return mapping


def ref_kinds(segment: dict, kinds: dict[str, str]) -> list[str]:
    resolved = [kinds.get(ref, "dangling") for ref in segment.get("extra_material_refs") or []]
    return sorted(resolved)


def segment_summary(segment: dict, kinds: dict[str, str]) -> dict:
    target = segment.get("target_timerange") or {}
    source = segment.get("source_timerange") or {}
    return {
        "start": target.get("start"),
        "duration": target.get("duration"),
        "source_duration": source.get("duration"),
        "volume": segment.get("volume"),
        "speed": segment.get("speed"),
        "channels": sorted(
            channel.get("property_type", "")
            for channel in segment.get("common_keyframes") or []
        ),
        "ref_kinds": ref_kinds(segment, kinds),
    }


def draft_summary(draft: dict) -> dict:
    kinds = bucket_of(draft)
    tracks: dict[str, list] = {}
    for track in draft.get("tracks", []):
        segments = [segment_summary(segment, kinds) for segment in track.get("segments", [])]
        tracks.setdefault(track.get("type"), []).extend(segments)
    for kind in tracks:
        tracks[kind].sort(key=lambda item: (item["start"] or 0, item["duration"] or 0))
    return {"duration": draft.get("duration"), "tracks": tracks}


def compare(ours: dict, reference: dict, policy: dict) -> list[str]:
    """比较两份草稿；只有 policy 里明确列出的素材种类差异才不算失败。"""
    extra_ok = set(policy.get("approved_extra_kinds") or [])
    approved_values = set(policy.get("approved_value_differences") or [])
    missing_ok = set(policy.get("approved_missing_kinds") or [])
    differences: list[str] = []

    def note(message: str) -> None:
        differences.append(message)

    if ours["duration"] != reference["duration"]:
        note(f"duration: ours={ours['duration']} ref={reference['duration']}")
    ours_tracks = ours["tracks"]
    reference_tracks = reference["tracks"]
    for kind in sorted(set(ours_tracks) | set(reference_tracks)):
        left = ours_tracks.get(kind) or []
        right = reference_tracks.get(kind) or []
        if len(left) != len(right):
            note(f"{kind}: segment count ours={len(left)} ref={len(right)}")
            continue
        for index, (left_segment, right_segment) in enumerate(zip(left, right)):
            for key in ("start", "duration", "source_duration", "volume", "speed"):
                if left_segment[key] != right_segment[key]:
                    if f"{kind}.{key}" in approved_values:
                        continue
                    note(
                        f"{kind}[{index}].{key}: ours={left_segment[key]} ref={right_segment[key]}"
                    )
            if left_segment["channels"] != right_segment["channels"]:
                note(
                    f"{kind}[{index}].channels: ours={left_segment['channels']} "
                    f"ref={right_segment['channels']}"
                )
            ours_kinds = left_segment["ref_kinds"]
            ref_kinds = right_segment["ref_kinds"]
            extra = [item for item in ours_kinds if ours_kinds.count(item) > ref_kinds.count(item)]
            missing = [item for item in ref_kinds if ref_kinds.count(item) > ours_kinds.count(item)]
            unexpected_extra = [item for item in extra if item not in extra_ok]
            unexpected_missing = [item for item in missing if item not in missing_ok]
            if unexpected_extra:
                note(f"{kind}[{index}].extra_kinds: ours has {sorted(set(unexpected_extra))} only")
            if unexpected_missing:
                note(
                    f"{kind}[{index}].missing_kinds: ref has {sorted(set(unexpected_missing))} only"
                )
    return differences


def prepare_media(case: dict, target: Path) -> None:
    target.mkdir(parents=True, exist_ok=True)
    for media in case.get("media", []):
        output = target / media["name"]
        run(["ffmpeg", "-hide_banner", "-v", "error", *media["gen"], "-y", str(output)])


def build_reference(case: dict, case_dir: Path, media_dir: Path, out: Path, pyjyd_src: Path, env: dict) -> None:
    out.mkdir(parents=True, exist_ok=True)
    script = ROOT / "examples" / "pyjyd" / case["reference"]
    reference_env = dict(env)
    reference_env["PYJYD_REPO"] = str(pyjyd_src)
    reference_env["PYTHONPATH"] = str(ROOT / "tools")
    run([sys.executable, str(script), str(media_dir), str(out)], env=reference_env)


def text_segment_id(binary: Path, draft: Path, env: dict) -> str:
    """??????? id?? post_build ????????"""
    payload = json.loads(
        run([str(binary), "timeline", "segments", str(draft), "--track", "text", "--json"], env=env)
    )
    segments = payload.get("data") or []
    if not segments:
        raise RuntimeError("case needs a text segment but the draft has none")
    return segments[0]["id"]


def build_ours(case: dict, case_dir: Path, media_dir: Path, out: Path, binary: Path, env: dict) -> None:
    plan = case_dir / "plan.json"
    shutil.copyfile(plan, media_dir / "plan.json")
    run([str(binary), "project", "build", str(media_dir / "plan.json"), "--out", str(out), "--json"], env=env)
    for command in case.get("post_build", []):
        argv = [str(binary), *command]
        argv = [part.format(ours=str(out), text_id=text_segment_id(binary, out, env)) for part in argv]
        run(argv, env=env)


def run_case(case_path: Path, binary: Path, pyjyd_src: Path, env: dict) -> dict:
    case = json.loads(case_path.read_text(encoding="utf-8"))
    case_dir = case_path.parent
    record = {
        "id": case["id"],
        "title": case.get("title", ""),
        "upstream": case.get("upstream", {}),
        "status": "failed",
        "differences": [],
    }
    with tempfile.TemporaryDirectory(prefix=f"pyjyd-{case['id']}-") as temporary:
        root = Path(temporary)
        media_dir = root / "media"
        prepare_media(case, media_dir)
        ours_dir = root / "ours"
        reference_dir = root / "reference"
        try:
            build_reference(case, case_dir, media_dir, reference_dir, pyjyd_src, env)
            build_ours(case, case_dir, media_dir, ours_dir, binary, env)
            ours = draft_summary(load_draft(ours_dir))
            reference_draft = next(reference_dir.glob("*/draft_content.json")).parent
            reference = draft_summary(load_draft(reference_draft))
            record["differences"] = compare(ours, reference, case.get("policy") or {})
            record["status"] = "passed" if not record["differences"] else "differs"
        except RuntimeError as error:
            record["error"] = str(error).splitlines()[0]
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    parser.add_argument("--bin", type=Path, default=ROOT / "target" / "release" / "jianying.exe")
    parser.add_argument("--report", type=Path, default=ROOT / "provenance" / "PYJYD_EXAMPLES_PARITY.json")
    args = parser.parse_args()

    env = dict(os.environ)
    env["JIANYING_CLI_PROCESS_LIST"] = str(Path(tempfile.gettempdir()) / "no-editor.txt")
    env["PYTHONUTF8"] = "1"

    cases = sorted(path for path in CASES.glob("*/case.json"))
    if not cases:
        print("no cases found", file=sys.stderr)
        return 2
    records = [run_case(path, args.bin.resolve(), args.pyjyd_src.resolve(), env) for path in cases]

    passed = sum(1 for record in records if record["status"] == "passed")
    report = {
        "schema": SCHEMA,
        "pyjyd": {
            "commit": subprocess.run(
                ["git", "-C", str(args.pyjyd_src), "rev-parse", "HEAD"],
                capture_output=True, text=True,
            ).stdout.strip(),
        },
        "binary": str(args.bin),
        "summary": {
            "total": len(records),
            "passed": passed,
            "differs": sum(1 for record in records if record["status"] == "differs"),
            "failed": sum(1 for record in records if record["status"] == "failed"),
        },
        "cases": records,
    }
    args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"ok": report["summary"]["failed"] == 0, **report["summary"]}))
    return 0 if report["summary"]["failed"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())

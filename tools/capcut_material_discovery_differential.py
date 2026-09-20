#!/usr/bin/env python3
"""Black-box material discovery differential against pinned capcut-cli."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import tempfile
from typing import Any


PINNED = "49f70e3b07f1a236d45acb9b49a70c141dd1ee98"


def invoke(argv: list[str]) -> Any:
    process = subprocess.run(argv, text=True, capture_output=True, check=False)
    if process.returncode != 0:
        raise RuntimeError(
            f"command failed ({process.returncode}): {argv!r}\n"
            f"stdout={process.stdout}\nstderr={process.stderr}"
        )
    return json.loads(process.stdout, strict=False)


def rust_data(binary: pathlib.Path, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"])["data"]


def upstream_data(entry: pathlib.Path, *args: str) -> Any:
    return invoke(["node", str(entry), *args])


def write_fixture(project: pathlib.Path) -> None:
    timeline = json.loads((project / "draft_content.json").read_text(encoding="utf-8"))
    timeline["tracks"] = []
    timeline["materials"] = {
        "videos": [
            {
                "id": "ABCDEF0011223344",
                "name": "fallback.mp4",
                "material_name": "hero.mp4",
                "path": "/fixture/hero.mp4",
                "duration": 3_000_000,
                "type": "video",
                "width": 1920,
            },
            {"id": "video-secondary", "path": "/fixture/secondary.mp4"},
        ],
        "audios": [
            {
                "id": "audio-primary",
                "name": "voice.wav",
                "path": "/fixture/voice.wav",
                "duration": 2_000_000,
            }
        ],
        "texts": [],
    }
    encoded = json.dumps(timeline, ensure_ascii=False, indent=2)
    (project / "draft_content.json").write_text(encoded, encoding="utf-8")
    (project / "draft_info.json").write_text(encoded, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--upstream", type=pathlib.Path, required=True)
    parser.add_argument(
        "--rust-bin", type=pathlib.Path, default=pathlib.Path("target/debug/jianying")
    )
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()

    revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != PINNED:
        raise RuntimeError(f"upstream revision {revision} does not match {PINNED}")
    entry = args.upstream / "dist/index.js"
    if not entry.is_file():
        raise RuntimeError("upstream dist/index.js is missing; run npm ci && npm run build")
    rust = args.rust_bin.resolve()
    if not rust.is_file():
        raise RuntimeError(f"Rust binary is missing: {rust}")

    results: list[dict[str, Any]] = []
    with tempfile.TemporaryDirectory(prefix="jianying-capcut-material-diff-") as temporary:
        project = pathlib.Path(temporary) / "draft"
        rust_data(rust, "project", "init", "material-differential", "--out", str(project))
        write_fixture(project)

        rust_counts = rust_data(rust, "media", "materials", str(project))
        upstream_counts = upstream_data(entry, "materials", str(project))
        assert rust_counts == upstream_counts

        rust_videos = rust_data(
            rust, "media", "materials", str(project), "--type", "videos"
        )
        upstream_videos = upstream_data(
            entry, "materials", str(project), "--type", "videos"
        )
        assert rust_videos == upstream_videos
        results.append(
            {
                "command": "materials",
                "status": "exact",
                "variants": ["type-counts", "type-filter-summaries"],
            }
        )

        rust_detail = rust_data(rust, "media", "material", str(project), "abcdef")
        upstream_detail = upstream_data(entry, "material", str(project), "abcdef")
        assert rust_detail == upstream_detail
        results.append(
            {
                "command": "material",
                "status": "exact",
                "variants": ["case-insensitive-id-prefix", "lossless-detail"],
            }
        )

    report = {
        "schema": "jianying-capcut-material-discovery-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_material_discovery_differential.py",
        "rust_cli": "provided-by-runner",
        "cases": results,
        "passed": len(results),
        "failed": 0,
    }
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

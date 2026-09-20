#!/usr/bin/env python3
"""Black-box scene, silence, and retake differential against pinned capcut-cli."""

from __future__ import annotations

import argparse
import json
import os
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
    return json.loads(process.stdout)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--upstream", type=pathlib.Path, required=True)
    parser.add_argument("--rust-bin", type=pathlib.Path, default=pathlib.Path("target/debug/jianying"))
    parser.add_argument("--output", type=pathlib.Path)
    args = parser.parse_args()
    revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != PINNED:
        raise RuntimeError(f"upstream revision {revision} does not match {PINNED}")
    entry = args.upstream / "dist/index.js"
    rust = args.rust_bin.resolve()

    cases: list[dict[str, str]] = []
    with tempfile.TemporaryDirectory(prefix="jianying-media-analysis-diff-") as temporary:
        root = pathlib.Path(temporary)
        media = root / "input.mp4"
        media.write_bytes(b"fixture")
        fake = root / "fake-ffmpeg"
        fake.write_text(
            "#!/bin/sh\n"
            "echo 'Duration: 00:00:06.000, start: 0.000000' >&2\n"
            "case \"$*\" in\n"
            "  *silencedetect*) echo 'silence_start: 1.0' >&2; "
            "echo 'silence_end: 2.0 | silence_duration: 1.0' >&2; "
            "echo 'silence_start: 5.0' >&2;;\n"
            "  *) echo 'frame:0 pts:1 pts_time:1.0' >&2; "
            "echo 'lavfi.scene_score=0.5' >&2; "
            "echo 'frame:1 pts:3 pts_time:3.0' >&2; "
            "echo 'lavfi.scene_score=0.9' >&2;;\n"
            "esac\nexit 0\n",
            encoding="utf-8",
        )
        fake.chmod(fake.stat().st_mode | 0o111)

        rust_scenes = invoke([
            str(rust), "media", "scenes", str(media), "--ffmpeg-cmd", str(fake),
            "--min-gap", "0", "--json",
        ])["data"]
        upstream_scenes = invoke([
            "node", str(entry), "detect-scenes", str(media), "--ffmpeg-cmd", str(fake),
            "--min-gap", "0",
        ])
        assert rust_scenes == upstream_scenes, (rust_scenes, upstream_scenes)
        cases.append({"command": "detect-scenes", "status": "exact"})

        rust_silence = invoke([
            str(rust), "media", "silence", str(media), "--ffmpeg-cmd", str(fake),
            "--pad", "0.1", "--json",
        ])["data"]
        upstream_silence = invoke([
            "node", str(entry), "detect-silence", str(media), "--ffmpeg-cmd", str(fake),
            "--pad", "0.1",
        ])
        assert rust_silence == upstream_silence, (rust_silence, upstream_silence)
        cases.append({"command": "detect-silence", "status": "exact"})

        srt = root / "takes.srt"
        srt.write_text(
            "1\n00:00:00,000 --> 00:00:02,000\nthis is the intended sentence\n\n"
            "2\n00:00:03,000 --> 00:00:05,000\nthis is the intended sentence\n\n"
            "3\n00:00:06,000 --> 00:00:07,000\nfinal unique line here\n",
            encoding="utf-8",
        )
        rust_retakes = invoke([
            str(rust), "media", "retakes", "--srt", str(srt), "--json",
        ])["data"]
        upstream_retakes = invoke([
            "node", str(entry), "detect-retakes", "--srt", str(srt),
        ])
        assert rust_retakes == upstream_retakes, (rust_retakes, upstream_retakes)
        cases.append({"command": "detect-retakes", "status": "exact"})

    report = {
        "schema": "jianying-capcut-media-analysis-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_media_analysis_differential.py",
        "cases": cases,
        "passed": len(cases),
        "failed": 0,
    }
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

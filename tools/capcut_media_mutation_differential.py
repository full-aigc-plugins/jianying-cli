#!/usr/bin/env python3
"""Black-box media-write differential against pinned capcut-cli."""

from __future__ import annotations

import argparse
import json
import pathlib
import shutil
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


def rust(binary: pathlib.Path, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"])["data"]


def upstream(entry: pathlib.Path, *args: str) -> Any:
    return invoke(["node", str(entry), *args])


def rebase(project: pathlib.Path) -> None:
    path = project / "draft_meta_info.json"
    value = json.loads(path.read_text(encoding="utf-8"))
    value["draft_fold_path"] = str(project.resolve())
    value["draft_root_path"] = str(project.parent.resolve())
    value["draft_json_file"] = str((project / "draft_content.json").resolve())
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2), encoding="utf-8")


def pair(root: pathlib.Path, base: pathlib.Path, name: str) -> tuple[pathlib.Path, pathlib.Path]:
    left, right = root / f"rust-{name}", root / f"upstream-{name}"
    shutil.copytree(base, left)
    shutil.copytree(base, right)
    rebase(left)
    rebase(right)
    return left, right


def timeline(project: pathlib.Path) -> dict[str, Any]:
    return json.loads((project / "draft_content.json").read_text(encoding="utf-8"))


def first_segment(project: pathlib.Path, kind: str) -> dict[str, Any]:
    for track in timeline(project)["tracks"]:
        if track.get("type") == kind and track.get("segments"):
            return track["segments"][0]
    raise AssertionError(f"no {kind} segment")


def ffmpeg(*args: str) -> None:
    subprocess.run(
        ["ffmpeg", "-hide_banner", "-loglevel", "error", "-y", *args], check=True
    )


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
    binary = args.rust_bin.resolve()
    if not entry.is_file() or not binary.is_file():
        raise RuntimeError("build both the pinned upstream and Rust CLI before running")

    cases: list[dict[str, str]] = []
    with tempfile.TemporaryDirectory(prefix="jianying-media-write-diff-") as temporary:
        root = pathlib.Path(temporary)
        video = root / "red.mp4"
        replacement = root / "blue.mp4"
        audio = root / "tone.wav"
        ffmpeg("-f", "lavfi", "-i", "color=c=red:s=320x240:d=2", "-an", "-c:v", "libx264", str(video))
        ffmpeg("-f", "lavfi", "-i", "color=c=blue:s=640x360:d=1", "-an", "-c:v", "libx264", str(replacement))
        ffmpeg("-f", "lavfi", "-i", "sine=frequency=440:duration=2", str(audio))
        base = root / "base"
        rust(binary, "project", "init", "media-diff", "--out", str(base))

        for command, kind, source in [("add-video", "video", video), ("add-audio", "audio", audio)]:
            rproj, uproj = pair(root, base, command)
            r = rust(binary, "media", command, str(rproj), str(source), "0s", "1s")
            u = upstream(entry, command, str(uproj), str(source), "0s", "1s")
            assert (r["start_us"], r["duration_us"]) == (u["start_us"], u["duration_us"])
            assert first_segment(rproj, kind)["target_timerange"] == first_segment(uproj, kind)["target_timerange"]
            cases.append({"command": command, "status": "semantic", "reason": "Rust stages local assets into the draft transaction"})

        rproj, uproj = pair(root, base, "replace")
        rid = rust(binary, "media", "add-video", str(rproj), str(video), "0s", "1s")["segment_id"]
        uid = upstream(entry, "add-video", str(uproj), str(video), "0s", "1s")["segment_id"]
        r = rust(binary, "media", "replace", str(rproj), rid, str(replacement), "--retime")
        u = upstream(entry, "replace-media", str(uproj), uid, str(replacement), "--retime")
        assert r["new_duration_us"] == u["new_duration_us"] == 1_000_000
        assert r["retimed"] == u["retimed"] is True
        cases.append({"command": "replace-media", "status": "semantic", "reason": "IDs and staged paths are intentionally transaction-local"})

        rproj, uproj = pair(root, base, "relink")
        rust(binary, "media", "add-video", str(rproj), str(video), "0s", "1s")
        upstream(entry, "add-video", str(uproj), str(video), "0s", "1s")
        for project in (rproj, uproj):
            value = timeline(project)
            value["materials"]["videos"][0]["path"] = str(root / "missing" / "red.mp4")
            encoded = json.dumps(value, ensure_ascii=False, indent=2)
            (project / "draft_content.json").write_text(encoded, encoding="utf-8")
            (project / "draft_info.json").write_text(encoded, encoding="utf-8")
        r = rust(binary, "media", "relink", str(rproj), "--dir", str(root), "--stage")
        u = upstream(entry, "relink", str(uproj), "--dir", str(root), "--stage")
        assert r["relinked_count"] == u["relinked"] == 1
        assert r["missing"] == u["still_missing"] == 0
        assert r["staged"] == u["staged"] == 1
        cases.append({"command": "relink", "status": "semantic", "reason": "Transaction directories and absolute paths differ"})

        rproj, uproj = pair(root, base, "sfx")
        r = rust(binary, "media", "sfx", str(rproj), "echo", "1s", "500ms")
        u = upstream(entry, "add-sfx", str(uproj), "echo", "1s", "500ms")
        assert {key: r[key] for key in ("name", "slug", "start_us", "duration_us")} == {
            key: u[key] for key in ("name", "slug", "start_us", "duration_us")
        }
        cases.append({"command": "add-sfx", "status": "exact-fields"})

        generator = root / "fake-tts.sh"
        generator.write_text(
            "#!/bin/sh\nffmpeg -hide_banner -loglevel error -y -f lavfi -i sine=frequency=600:duration=1 \"$1\"\n",
            encoding="utf-8",
        )
        generator.chmod(0o755)
        template = f"{generator} {{out}} {{text}}"
        rproj, uproj = pair(root, base, "tts")
        r = rust(binary, "media", "tts", str(rproj), "0s", "--text", "hello world", "--tts-cmd", template)
        u = upstream(entry, "tts", str(uproj), "0s", "--text", "hello world", "--tts-cmd", template)
        assert r["text_delivery"] == u["text_delivery"] == "argv"
        assert r["duration_us"] == u["duration_us"] == 1_000_000
        cases.append({"command": "tts", "status": "semantic", "reason": "Generated filenames and IDs are nondeterministic"})

    report = {
        "schema": "jianying-capcut-media-mutation-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_media_mutation_differential.py",
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

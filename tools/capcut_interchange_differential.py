#!/usr/bin/env python3
"""Black-box OTIO export differential against pinned capcut-cli."""

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


def rust_data(binary: pathlib.Path, *args: str) -> Any:
    return invoke([str(binary), *args, "--json"])["data"]


def upstream_data(entry: pathlib.Path, *args: str) -> Any:
    return invoke(["node", str(entry), *args])


def segment(identifier: str, material_id: str, start: int, duration: int) -> dict[str, Any]:
    return {
        "id": identifier,
        "material_id": material_id,
        "target_timerange": {"start": start, "duration": duration},
        "source_timerange": {"start": 0, "duration": duration},
        "speed": 1,
        "volume": 1,
        "visible": True,
        "clip": None,
        "extra_material_refs": [],
        "render_index": 0,
    }


def write_fixture(project: pathlib.Path) -> None:
    timeline = json.loads((project / "draft_content.json").read_text(encoding="utf-8"))
    timeline.update(
        {
            "id": "draft-otio",
            "name": "OTIO test",
            "duration": 3_000_000,
            "fps": 30,
            "tracks": [
                {
                    "id": "TV",
                    "type": "video",
                    "name": "Video 1",
                    "segments": [
                        segment("seg-a", "vid-a", 0, 1_000_000),
                        {
                            **segment("seg-b", "vid-b", 2_000_000, 1_000_000),
                            "source_timerange": {"start": 500_000, "duration": 2_000_000},
                            "speed": 2,
                        },
                    ],
                },
                {
                    "id": "TA",
                    "type": "audio",
                    "name": "Audio 1",
                    "segments": [segment("seg-c", "aud-a", 0, 3_000_000)],
                },
                {
                    "id": "TT",
                    "type": "text",
                    "name": "Captions",
                    "segments": [segment("seg-t", "txt-a", 0, 1_000_000)],
                },
            ],
            "materials": {
                "videos": [
                    {
                        "id": "vid-a",
                        "path": "/media/a.mp4",
                        "material_name": "a.mp4",
                        "duration": 5_000_000,
                        "type": "video",
                    },
                    {
                        "id": "vid-b",
                        "path": "",
                        "material_name": "missing",
                        "duration": 0,
                        "type": "video",
                    },
                ],
                "audios": [
                    {
                        "id": "aud-a",
                        "path": "/media/song.mp3",
                        "name": "song.mp3",
                        "duration": 10_000_000,
                        "type": "audio",
                    }
                ],
                "texts": [
                    {"id": "txt-a", "content": json.dumps({"text": "你好", "styles": []}, ensure_ascii=False)}
                ],
            },
        }
    )
    encoded = json.dumps(timeline, ensure_ascii=False, indent=2)
    (project / "draft_content.json").write_text(encoded, encoding="utf-8")
    (project / "draft_info.json").write_text(encoded, encoding="utf-8")


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
    if not entry.is_file() or not rust.is_file():
        raise RuntimeError("built upstream dist/index.js and Rust binary are required")

    with tempfile.TemporaryDirectory(prefix="jianying-capcut-interchange-diff-") as temporary:
        root = pathlib.Path(temporary)
        project = root / "draft"
        rust_data(rust, "project", "init", "otio-differential", "--out", str(project))
        write_fixture(project)

        upstream_plain = upstream_data(entry, "export-timeline", str(project))
        rust_plain = rust_data(rust, "project", "export-timeline", str(project))
        assert rust_plain == upstream_plain

        upstream_markers = upstream_data(
            entry, "export-timeline", str(project), "--captions", "markers"
        )
        rust_markers = rust_data(
            rust, "project", "export-timeline", str(project), "--captions", "markers"
        )
        assert rust_markers == upstream_markers

        output = root / "cut.otio"
        upstream_summary = upstream_data(
            entry, "export-timeline", str(project), "--out", str(output)
        )
        upstream_document = json.loads(output.read_text(encoding="utf-8"))
        rust_summary = rust_data(
            rust,
            "project",
            "export-timeline",
            str(project),
            "--out",
            str(output),
        )
        rust_document = json.loads(output.read_text(encoding="utf-8"))
        assert rust_summary == upstream_summary
        assert rust_document == upstream_document

        marker_output = root / "markers.otio"
        rust_data(
            rust, "project", "export-timeline", str(project), "--out", str(marker_output),
            "--captions", "markers", "--quiet"
        )
        upstream_import = root / "upstream-import"
        rust_import = root / "rust-import"
        upstream_import_result = upstream_data(
            entry, "import-timeline", str(marker_output), "--out", str(upstream_import),
            "--template", "bundled"
        )
        rust_import_result = rust_data(
            rust, "project", "import-timeline", str(marker_output), "--out", str(rust_import),
            "--quiet"
        )
        for key in ["mode", "tracks", "clips", "gaps", "captions"]:
            assert rust_import_result[key] == upstream_import_result[key]
        assert len(rust_import_result["placeholders"]) == len(upstream_import_result["placeholders"])

        def timeline_shape(path: pathlib.Path) -> list[dict[str, Any]]:
            timeline = json.loads((path / "draft_content.json").read_text(encoding="utf-8"))
            return [
                {
                    "type": track["type"],
                    "name": track["name"],
                    "segments": [
                        {
                            "target": segment["target_timerange"],
                            "source": segment["source_timerange"],
                            "speed": segment["speed"],
                            "volume": segment["volume"],
                        }
                        for segment in track.get("segments", [])
                    ],
                }
                for track in timeline["tracks"]
                if track["type"] in {"video", "audio", "text"} and track.get("segments")
            ]

        assert timeline_shape(rust_import) == timeline_shape(upstream_import)

        upstream_into = root / "upstream-into"
        rust_into = root / "rust-into"
        shutil.copytree(project, upstream_into)
        shutil.copytree(project, rust_into)
        upstream_before = (upstream_into / "draft_content.json").read_bytes()
        rust_before = (rust_into / "draft_content.json").read_bytes()
        upstream_dry = upstream_data(
            entry, "import-timeline", str(marker_output), "--into", str(upstream_into),
            "--dry-run"
        )
        rust_dry = rust_data(
            rust, "project", "import-timeline", str(marker_output), "--into", str(rust_into),
            "--dry-run", "--quiet"
        )
        assert upstream_dry["dryRun"] is True and rust_dry["dryRun"] is True
        assert (upstream_into / "draft_content.json").read_bytes() == upstream_before
        assert (rust_into / "draft_content.json").read_bytes() == rust_before

    report = {
        "schema": "jianying-capcut-interchange-differential/v1",
        "upstream": {"repository": "renezander030/capcut-cli", "commit": revision},
        "runner": "tools/capcut_interchange_differential.py",
        "rust_cli": "provided-by-runner",
        "cases": [
            {
                "command": "export-timeline",
                "status": "exact",
                "variants": ["stdout-document", "file-and-summary", "caption-markers"],
            },
            {
                "command": "import-timeline",
                "status": "normalized-equivalent",
                "variants": ["new-draft", "caption-markers", "placeholders", "dry-run"],
                "approved": ["generated-ids", "template-metadata", "missing-path-normalized-empty"],
            },
        ],
        "passed": 2,
        "failed": 0,
    }
    encoded = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

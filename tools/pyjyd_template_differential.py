"""Generate fixed pyJianYingDraft template timing/style observations."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from copy import deepcopy
from pathlib import Path


def segment(segment_id: str, start: int, duration: int) -> dict:
    return {
        "id": segment_id,
        "material_id": "material",
        "target_timerange": {"start": start, "duration": duration},
        "source_timerange": {"start": 0, "duration": duration},
        "speed": 1.0,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--pyjyd-src", type=Path, required=True)
    args = parser.parse_args()
    source = args.pyjyd_src.resolve()
    sys.path.insert(0, str(source.parent))

    from pyJianYingDraft._script_file_template import _ScriptFileTemplateOps
    from pyJianYingDraft.template_mode import (
        ExtendMode,
        ImportedMediaSegment,
        ImportedMediaTrack,
        ImportedSegment,
        ImportedTextTrack,
        ShrinkMode,
    )
    from pyJianYingDraft.time_util import Timerange

    timing_inputs = [
        ("cut_head", 1, 1_000_000, "cut_head", []),
        ("cut_tail", 1, 1_000_000, "cut_tail", []),
        ("cut_tail_align", 1, 1_000_000, "cut_tail_align", []),
        ("shrink", 1, 1_000_000, "shrink", []),
        ("extend_head", 1, 3_000_000, "cut_tail", ["extend_head"]),
        ("extend_tail", 1, 3_000_000, "cut_tail", ["extend_tail"]),
        ("push_tail", 1, 3_000_000, "cut_tail", ["push_tail"]),
        (
            "fallback_cut_material_tail",
            0,
            3_000_000,
            "cut_tail",
            ["extend_head", "cut_material_tail"],
        ),
    ]
    timing_cases = []
    base = [
        segment("previous", 0, 1_000_000),
        segment("target", 2_000_000, 2_000_000),
        segment("following", 4_500_000, 1_000_000),
    ]
    for name, index, duration, shrink, extend in timing_inputs:
        raw_segments = deepcopy(base)
        if index == 0:
            raw_segments = [segment("target", 0, 2_000_000), segment("following", 2_500_000, 1_000_000)]
        elif name == "extend_tail":
            raw_segments[2]["target_timerange"]["start"] = 5_500_000
        track = object.__new__(ImportedMediaTrack)
        track.segments = []
        for raw in raw_segments:
            imported = object.__new__(ImportedMediaSegment)
            imported.raw_data = deepcopy(raw)
            imported.material_id = raw["material_id"]
            imported.target_timerange = Timerange(**raw["target_timerange"])
            imported.source_timerange = Timerange(**raw["source_timerange"])
            track.segments.append(imported)
        track.process_timerange(
            index,
            Timerange(0, duration),
            ShrinkMode(shrink),
            [ExtendMode(mode) for mode in extend],
        )
        timing_cases.append(
            {
                "name": name,
                "segment_index": index,
                "source_start_us": 0,
                "source_duration_us": duration,
                "shrink_mode": shrink,
                "extend_modes": extend,
                "segments": [
                    {
                        **item.raw_data,
                        "material_id": item.material_id,
                        "target_timerange": item.target_timerange.export_json(),
                        "source_timerange": item.source_timerange.export_json(),
                    }
                    for item in track.segments
                ],
            }
        )

    owner = object.__new__(_ScriptFileTemplateOps)
    owner.imported_materials = {
        "texts": [
            {
                "id": "text-material",
                "content": json.dumps(
                    {
                        "text": "ABCD",
                        "styles": [
                            {"range": [0, 4], "bold": False},
                            {"range": [1, 3], "bold": True},
                        ],
                    },
                    ensure_ascii=False,
                ),
            }
        ]
    }
    text_track = object.__new__(ImportedTextTrack)
    text_segment = object.__new__(ImportedSegment)
    text_segment.material_id = "text-material"
    text_segment.target_timerange = Timerange(0, 1_000_000)
    text_segment.raw_data = {}
    text_track.segments = [text_segment]
    owner.replace_text(text_track, 0, "ABCDEFGH", recalc_style=True)
    styled = json.loads(owner.imported_materials["texts"][0]["content"])

    commit = subprocess.check_output(
        ["git", "-C", str(source.parent), "rev-parse", "HEAD"], text=True
    ).strip()
    report = {
        "schema": "jianying-pyjyd-template-differential/v1",
        "upstream_commit": commit,
        "timing_cases": timing_cases,
        "style_cases": [
            {
                "name": "ascii_proportional_growth",
                "old_text": "ABCD",
                "new_text": "ABCDEFGH",
                "styles": styled["styles"],
            }
        ],
        "approved_differences": [
            "Rust applies proportional style ranges to UTF-16 code units so emoji offsets remain valid; upstream Python uses Unicode code-point length."
        ],
    }
    json.dump(report, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

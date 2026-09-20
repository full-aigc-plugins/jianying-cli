#!/usr/bin/env python3
"""Import the fixed MIT capcut-cli enum metadata as an offline Rust asset."""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess


PINNED = "49f70e3b07f1a236d45acb9b49a70c141dd1ee98"
EXPECTED = {
    "capcut": {
        "transitions": 116, "masks": 9, "image_intros": 43, "image_outros": 23,
        "image_combos": 108, "text_intros": 76, "text_outros": 68,
        "text_loop_anims": 53, "scene_effects": 345, "character_effects": 95,
        "audio_effects": 15,
    },
    "jianying": {
        "transitions": 362, "masks": 6, "image_intros": 95, "image_outros": 72,
        "image_combos": 123, "text_intros": 144, "text_outros": 97,
        "text_loop_anims": 92, "scene_effects": 912, "character_effects": 227,
        "audio_effects": 43, "fonts": 335, "filters": 468,
    },
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("upstream", type=pathlib.Path)
    parser.add_argument("--output", type=pathlib.Path, required=True)
    args = parser.parse_args()
    revision = subprocess.check_output(
        ["git", "-C", str(args.upstream), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != PINNED:
        raise RuntimeError(f"upstream revision {revision} does not match {PINNED}")
    source = args.upstream / "src/enums.json"
    value = json.loads(source.read_text(encoding="utf-8"))
    actual = {
        namespace: {category: len(entries) for category, entries in categories.items()}
        for namespace, categories in value.items()
    }
    if actual != EXPECTED:
        raise RuntimeError(f"enum catalogue shape drifted: {actual!r}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

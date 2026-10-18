#!/usr/bin/env python3
"""示例参照实现的公共部分。

上游 `tests/helpers.py` 用 `object.__new__` 造出假的视频/音频素材（不读真实文件），
测试再用 `script.dumps()` 直接拿到草稿 JSON。参照构建器沿用同一套做法：
不需要真实媒体，也不需要写草稿目录，只要把 JSON 落到输出目录即可。
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

_UPSTREAM = Path(__file__).resolve().parent / "upstream"
if str(_UPSTREAM) not in sys.path:
    sys.path.insert(0, str(_UPSTREAM))


def load_pyjyd():
    repo = os.environ.get("PYJYD_REPO")
    if not repo:
        raise SystemExit("PYJYD_REPO must point at a pyJianYingDraft checkout")
    for candidate in (repo, os.path.join(repo, "pyJianYingDraft")):
        if candidate not in sys.path:
            sys.path.insert(0, candidate)
    import pyJianYingDraft as draft  # noqa: PLC0415

    return draft


def write_dump(script, out_dir: Path, name: str) -> None:
    """把 ScriptFile.dumps() 的 JSON 写进 <out_dir>/<name>/draft_content.json。"""
    target = out_dir / name
    target.mkdir(parents=True, exist_ok=True)
    (target / "draft_content.json").write_text(script.dumps(), encoding="utf-8")
    print(f"reference built: {target}")


def fake_video_material(draft, name: str, media_dir: Path, duration: int = 5_000_000):
    """对应上游 tests/helpers.fake_video_material，但指向合成素材的真实路径。"""
    material = object.__new__(draft.VideoMaterial)
    material.material_id = "video-material-id"
    material.local_material_id = ""
    material.material_name = name
    material.path = str(media_dir / name)
    material.duration = duration
    material.width = 640
    material.height = 360
    material.crop_settings = draft.CropSettings()
    material.material_type = "video"
    return material


def fake_audio_material(draft, name: str, media_dir: Path, duration: int = 5_000_000):
    material = object.__new__(draft.AudioMaterial)
    material.material_id = "audio-material-id"
    material.material_name = name
    material.path = str(media_dir / name)
    material.duration = duration
    return material

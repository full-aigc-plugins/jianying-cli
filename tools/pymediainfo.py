"""差分测试专用的 pymediainfo 最小兼容层。

只实现固定 pyJianYingDraft 提交在 local_materials.py 中使用的 API，底层读取
系统 ffprobe JSON。它不进入 Rust 产物，也不修改或替代上游协议生成逻辑。
"""

from __future__ import annotations

import json
import subprocess
from pathlib import Path
from types import SimpleNamespace


class MediaInfo:
    def __init__(self, *, video_tracks, audio_tracks, image_tracks, general_tracks):
        self.video_tracks = video_tracks
        self.audio_tracks = audio_tracks
        self.image_tracks = image_tracks
        self.general_tracks = general_tracks

    @staticmethod
    def can_parse() -> bool:
        result = subprocess.run(
            ["ffprobe", "-version"], capture_output=True, text=True, check=False
        )
        return result.returncode == 0

    @classmethod
    def parse(cls, path, **_kwargs):
        result = subprocess.run(
            [
                "ffprobe",
                "-v",
                "error",
                "-show_streams",
                "-show_format",
                "-of",
                "json",
                str(path),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode != 0:
            raise ValueError(result.stderr.strip() or f"ffprobe failed for {path}")
        payload = json.loads(result.stdout)
        format_duration_ms = _seconds_to_ms(payload.get("format", {}).get("duration"))
        general_tracks = [SimpleNamespace(duration=format_duration_ms)]
        video_tracks = []
        audio_tracks = []
        image_tracks = []
        image_suffixes = {".png", ".jpg", ".jpeg", ".webp", ".bmp", ".tif", ".tiff"}
        is_image = Path(path).suffix.lower() in image_suffixes
        for stream in payload.get("streams", []):
            duration_ms = _seconds_to_ms(stream.get("duration")) or format_duration_ms
            track = SimpleNamespace(
                duration=duration_ms,
                width=stream.get("width"),
                height=stream.get("height"),
            )
            if stream.get("codec_type") == "audio":
                audio_tracks.append(track)
            elif stream.get("codec_type") == "video" and is_image:
                image_tracks.append(track)
            elif stream.get("codec_type") == "video":
                video_tracks.append(track)
        return cls(
            video_tracks=video_tracks,
            audio_tracks=audio_tracks,
            image_tracks=image_tracks,
            general_tracks=general_tracks,
        )


def _seconds_to_ms(value):
    if value in (None, "", "N/A"):
        return None
    return float(value) * 1000.0

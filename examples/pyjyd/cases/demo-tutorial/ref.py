#!/usr/bin/env python3
"""demo.py 的参照实现：逐句复刻上游 demo.py 的 pyJianYingDraft 调用。

上游 demo.py 读的是 readme_assets/tutorial 里的真实素材，这里换成同一个
脚本在合成素材上的等价执行，语义（时长、音量、淡入、入场动画、转场、
GIF 贴纸、模糊背景填充、气泡 + 花字文本）保持不变，便于与本 CLI 对照。

用法：python ref.py <media_dir> <draft_root>
"""

from __future__ import annotations

import os
import sys

_repo = os.environ.get("PYJYD_REPO")
if _repo:
    # 上游仓库根目录（包在其下一层）
    sys.path.insert(0, _repo)
    sys.path.insert(0, os.path.join(_repo, "pyJianYingDraft"))

import pyJianYingDraft as draft
from pyJianYingDraft import IntroType, TransitionType, trange, trange_seconds, tim


def main() -> int:
    media_dir, draft_root = sys.argv[1], sys.argv[2]
    folder = draft.DraftFolder(draft_root)
    script = folder.create_draft("demo", 1920, 1080, allow_replace=True)

    script.append_tracks([
        draft.TrackSpec(draft.TrackType.audio, "bgm"),
        draft.TrackSpec(draft.TrackType.video, "main_video"),
        draft.TrackSpec(draft.TrackType.text, "caption"),
    ])

    audio_segment = draft.AudioSegment(
        os.path.join(media_dir, "audio.mp3"),
        trange_seconds(0, duration=5),
        volume=0.6,
    )
    audio_segment.add_fade("1s", "0s")

    video_segment = draft.VideoSegment(
        os.path.join(media_dir, "video.mp4"),
        trange_seconds(0, duration=4.2),
    )
    video_segment.add_animation(IntroType.斜切)

    gif_material = draft.VideoMaterial(os.path.join(media_dir, "sticker.gif"))
    gif_segment = draft.VideoSegment(
        gif_material,
        trange(video_segment.end, gif_material.duration),
    )
    gif_segment.add_background_filling("blur", 0.0625)

    video_segment.add_transition(TransitionType.信号故障)

    script.add_segment(audio_segment, "bgm")
    script.add_segment(video_segment, "main_video")
    script.add_segment(gif_segment, "main_video")

    text_segment = draft.TextSegment(
        "据说pyJianYingDraft效果还不错?",
        video_segment.target_timerange,
        font=draft.FontType.文轩体,
        style=draft.TextStyle(color=(1.0, 1.0, 0.0)),
        clip_settings=draft.ClipSettings(transform_y=-0.8),
    )
    text_segment.add_animation(draft.TextOutro.故障闪动, duration=tim("1s"))
    text_segment.add_bubble("361595", "6742029398926430728")
    text_segment.add_effect("7296357486490144036")
    script.add_segment(text_segment, "caption")

    script.save()
    print(f"reference built: {draft_root}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

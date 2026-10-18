#!/usr/bin/env python3
"""tests/test_video_mix_mode_export.py：重复设置混合同模式只保留最后一次"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from reference_common import fake_video_material, load_pyjyd, write_dump


def main() -> int:
    media_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    draft = load_pyjyd()
    script = draft.ScriptFile(1920, 1080, 30, True)
    script.append_track(draft.TrackSpec(draft.TrackType.video))
    material = fake_video_material(draft, "clip.mp4", media_dir)
    segment = draft.VideoSegment(material, draft.trange("0s", "2s"))
    # 先设正片叠底、再设滤色：上游断言只保留最后一次
    segment.set_mix_mode(draft.MixModeType.正片叠底)
    segment.set_mix_mode(draft.MixModeType.滤色)
    script.add_segment(segment)
    write_dump(script, out_dir, "mix-mode")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

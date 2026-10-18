#!/usr/bin/env python3
"""tests/test_scriptfile_building.py??????????????"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from reference_common import fake_audio_material, fake_video_material, load_pyjyd, write_dump


def main() -> int:
    media_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    draft = load_pyjyd()
    script = draft.ScriptFile(1920, 1080, 30, True)
    script.append_track(draft.TrackSpec(draft.TrackType.video))
    material = fake_video_material(draft, "clip.mp4", media_dir)
    segment = draft.VideoSegment(material, draft.trange("0s", "2s"))
    segment.add_fade("0.2s", "0.3s")
    segment.add_animation(draft.IntroType.斜切)
    script.add_segment(segment)
    write_dump(script, out_dir, "scriptfile-building")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

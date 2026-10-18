#!/usr/bin/env python3
"""tests/test_audio_effect_export.py???????????"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from reference_common import fake_audio_material, fake_video_material, load_pyjyd, write_dump


def main() -> int:
    media_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    draft = load_pyjyd()
    script = draft.ScriptFile(1920, 1080, 30, True)
    script.append_track(draft.TrackSpec(draft.TrackType.audio))
    material = fake_audio_material(draft, "audio.mp3", media_dir)
    segment = draft.AudioSegment(material, draft.trange("0s", "2s"))
    from pyJianYingDraft.metadata import ToneEffectType

    segment.add_effect(ToneEffectType.机器人, [100])
    script.add_segment(segment)
    write_dump(script, out_dir, "audio-effect")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

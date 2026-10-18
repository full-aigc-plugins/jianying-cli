#!/usr/bin/env python3
"""tests/test_import_srt.py：导入 SRT 到新建文本轨"""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from reference_common import load_pyjyd, write_dump


def main() -> int:
    media_dir, out_dir = Path(sys.argv[1]), Path(sys.argv[2])
    draft = load_pyjyd()
    srt = media_dir / "captions.srt"
    script = draft.ScriptFile(1920, 1080, 30, True)
    script.import_srt(str(srt), "captions")
    write_dump(script, out_dir, "import-srt")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Reconcile an exported video against the draft it claims to come from.

4.3 asks for continuity between what the draft says and what comes out of the editor. This compares the
two directly: it reads the draft timeline (canvas, fps, last segment end, track and segment counts) and
probes the exported file with ffprobe, then reports every difference.

    python tools/export_reconciliation.py --draft <dir> --export <file> [--json] [--tolerance-ms 120]

Exit code 0 means the export agrees with the draft within tolerance; 1 means it does not, and the
findings say which fact disagreed. An encrypted draft is reported, never parsed.
"""
import argparse
import json
import subprocess
import sys
from pathlib import Path


def fail(kind, path, detail=""):
    return [{"kind": kind, "path": str(path), "detail": detail}]


def read_draft(directory):
    content = Path(directory) / "draft_content.json"
    if not content.is_file():
        return None, fail("draft_missing_content", content)
    raw = content.read_bytes()
    try:
        return json.loads(raw.decode("utf-8")), []
    except (UnicodeDecodeError, json.JSONDecodeError):
        head = raw.lstrip()[:64]
        if head and all(chr(b).isalnum() or chr(b) in "+/=:\r\n" for b in head):
            return None, fail("draft_content_encrypted", content)
        return None, fail("draft_content_unreadable", content)


def draft_facts(document, duration_source="all"):
    canvas = document.get("canvas_config") or {}
    duration = document.get("duration") if duration_source == "all" else None
    if duration is None:
        ends = []
        for track in document.get("tracks") or []:
            if duration_source == "video" and track.get("type") not in (None, "video"):
                continue
            for segment in track.get("segments") or []:
                target = segment.get("target_timerange") or {}
                ends.append(int(target.get("start") or 0) + int(target.get("duration") or 0))
        duration = max(ends) if ends else 0
    tracks = document.get("tracks") or []
    return {
        "width": canvas.get("width"),
        "height": canvas.get("height"),
        "ratio": canvas.get("ratio"),
        "fps": document.get("fps"),
        "duration_us": int(duration or 0),
        "tracks": len(tracks),
        "segments": sum(len(track.get("segments") or []) for track in tracks),
    }


def probe_export(path):
    completed = subprocess.run(
        ["ffprobe", "-v", "error", "-show_entries", "format=duration",
         "-show_entries", "stream=codec_type,codec_name,width,height,r_frame_rate", "-of", "json", str(path)],
        capture_output=True, text=True,
    )
    if completed.returncode != 0:
        return None, fail("export_unprobeable", path, completed.stderr.strip()[:200])
    payload = json.loads(completed.stdout)
    video = next((s for s in payload.get("streams", []) if s.get("codec_type") == "video"), {})
    numerator, _, denominator = (video.get("r_frame_rate") or "0/1").partition("/")
    try:
        fps = float(numerator) / float(denominator or 1)
    except (TypeError, ZeroDivisionError):
        fps = None
    return {
        "duration_s": float(payload.get("format", {}).get("duration") or 0),
        "codec": video.get("codec_name"),
        "width": video.get("width"),
        "height": video.get("height"),
        "fps": fps,
    }, []


def reconcile(draft, export, tolerance_ms):
    findings = []
    expected_ms = draft["duration_us"] / 1000.0
    actual_ms = export["duration_s"] * 1000.0
    delta_ms = actual_ms - expected_ms
    if abs(delta_ms) > tolerance_ms:
        findings.append({"kind": "duration_mismatch", "expected_ms": round(expected_ms, 1),
                         "actual_ms": round(actual_ms, 1), "delta_ms": round(delta_ms, 1),
                         "tolerance_ms": tolerance_ms})

    if draft["width"] and draft["height"] and export["width"] and export["height"]:
        expected_ratio = draft["width"] / draft["height"]
        actual_ratio = export["width"] / export["height"]
        if abs(expected_ratio - actual_ratio) > 0.01:
            findings.append({"kind": "aspect_ratio_mismatch",
                             "expected": f"{draft['width']}x{draft['height']}",
                             "actual": f"{export['width']}x{export['height']}"})

    if draft["fps"] and export["fps"] and abs(float(draft["fps"]) - export["fps"]) > 0.51:
        findings.append({"kind": "fps_mismatch", "expected": draft["fps"], "actual": export["fps"]})
    return findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--draft", required=True, help="draft directory")
    parser.add_argument("--export", required=True, help="exported video file")
    parser.add_argument("--tolerance-ms", type=float, default=120.0)
    parser.add_argument("--duration-source", choices=("all", "video"), default="all",
                        help="all tracks, or only video tracks (a proxy renders just those)")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    document, failures = read_draft(args.draft)
    if document is None:
        report = {"ok": False, "draft": args.draft, "export": args.export, "findings": failures}
    else:
        export, failures = probe_export(Path(args.export))
        if export is None:
            report = {"ok": False, "draft": args.draft, "export": args.export, "findings": failures}
        else:
            draft = draft_facts(document, args.duration_source)
            findings = failures + reconcile(draft, export, args.tolerance_ms)
            report = {"ok": not findings, "draft": args.draft, "export": args.export,
                      "draft_facts": draft, "export_facts": export, "findings": findings}

    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(f"ok={report['ok']}")
        if "draft_facts" in report:
            print(f"  draft : {report['draft_facts']}")
            print(f"  export: {report['export_facts']}")
        for finding in report["findings"]:
            print(f"  {finding['kind']}: {json.dumps(finding, ensure_ascii=False)[:160]}")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Copy a draft so it can be opened in 剪映 without sacrificing the original.

剪映 rewrites a plain draft into a Base64 envelope as soon as it saves it, so opening the only copy costs
the ability to lint or diff it afterwards. This makes a working copy, repoints every media path at the
copied assets (the CLI writes absolute paths), and reports what it did.

    python tools/prepare_editor_copy.py --source <draft dir> --name <new draft name> [--json]

Refuses to copy an encrypted source, because the copy would be unusable for the same checks.
"""
import argparse
import json
import shutil
from pathlib import Path

MEDIA_BUCKETS = ("videos", "audios", "images")


def read_document(directory):
    content = Path(directory) / "draft_content.json"
    if not content.is_file():
        return None, "draft_missing_content"
    raw = content.read_bytes()
    try:
        return json.loads(raw.decode("utf-8")), None
    except (UnicodeDecodeError, json.JSONDecodeError):
        head = raw.lstrip()[:64]
        if head and all(chr(b).isalnum() or chr(b) in "+/=:\r\n" for b in head):
            return None, "draft_content_encrypted"
        return None, "draft_content_unreadable"


def repoint_media(document, target):
    """把绝对路径改成副本自己的资产；只在副本里确实有同名文件时才改。"""
    moved = []
    for bucket in MEDIA_BUCKETS:
        for material in document.get("materials", {}).get(bucket) or []:
            path = material.get("path")
            if not path:
                continue
            original = Path(path)
            candidate = Path(target) / "assets" / bucket[:-1] / original.name
            if candidate.is_file() and candidate != original:
                material["path"] = str(candidate)
                moved.append({"bucket": bucket, "from": path, "to": str(candidate)})
    return moved


def prepare(source, target, overwrite=False):
    source, target = Path(source), Path(target)
    if not source.is_dir():
        return {"ok": False, "error": "source_missing", "source": str(source)}
    document, failure = read_document(source)
    if document is None:
        return {"ok": False, "error": failure, "source": str(source)}
    if target.exists():
        if not overwrite:
            return {"ok": False, "error": "target_exists", "target": str(target)}
        shutil.rmtree(target)
    shutil.copytree(source, target)
    moved = repoint_media(document, target)
    content = target / "draft_content.json"
    content.write_text(json.dumps(document, ensure_ascii=False), encoding="utf-8", newline="\n")
    missing = [
        material.get("path")
        for bucket in MEDIA_BUCKETS
        for material in document.get("materials", {}).get(bucket) or []
        if material.get("path") and not Path(material["path"]).is_file()
    ]
    return {
        "ok": not missing,
        "source": str(source),
        "target": str(target),
        "repointed": moved,
        "missing_media": missing,
        "next": "open this copy in 剪映; the original stays plain for linting and diffing",
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--name", required=True, help="directory name next to the source")
    parser.add_argument("--overwrite", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    source = Path(args.source).resolve()
    target = source.parent / args.name
    report = prepare(source, target, args.overwrite)
    if args.json:
        print(json.dumps(report, ensure_ascii=False, indent=2))
    else:
        print(f"ok={report['ok']} target={report.get('target')}")
        for item in report.get("repointed", []):
            print(f"  {item['bucket']}: {item['from']} -> {item['to']}")
        for path in report.get("missing_media", []):
            print(f"  missing: {path}")
        if "error" in report:
            print(f"  error: {report['error']}")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

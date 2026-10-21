#!/usr/bin/env python3
"""Lint an action plan before it runs: every resource id must exist, and only where it is allowed.

An agent writing a script of actions needs a cheap check between "I planned it" and "it runs". This
walks a plan or job document for `resource_id` / `effect_id` fields and answers three questions:

  1. does each id exist in the embedded catalog (and in which domain)?
  2. is a VIP-only id being used without a professional entitlement?
  3. are the time fields sane (start >= 0, duration > 0)?

    python tools/plan_resource_lint.py plan.json [--entitlement evidence.json] [--json]

Exit code 0 means every id resolved and no timing defect was found; 1 means at least one problem.
Nothing here mutates a draft or contacts the editor.
"""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CATALOGS = ROOT / "catalogs"

RESOURCE_KEYS = {"resource_id", "material_resource_id", "resourceId"}
# 只有这些父字段的编号在内嵌目录里；其余（如 text_effect 花字）目录未覆盖，不能判死
CATALOGUED_PARENTS = {"transition", "filter", "effect", "sticker", "video_effect", "audio_effect"}
EFFECT_KEYS = {"effect_id", "effectId"}
VIP_CAPABILITY = "official_assets.professional"


def load_index():
    """resource_id -> (domain, entry) 与 effect_id -> (domain, entry)。"""
    by_resource, by_effect = {}, {}
    index = json.loads((CATALOGS / "index.json").read_text(encoding="utf-8"))
    for domain in index["catalogs"]:
        path = CATALOGS / f"{domain}.json"
        if not path.is_file():
            continue
        for entry in json.loads(path.read_text(encoding="utf-8")):
            rid = entry.get("resource_id")
            if rid:
                by_resource.setdefault(str(rid), (domain, entry))
            eid = entry.get("effect_id")
            if eid:
                by_effect.setdefault(str(eid), (domain, entry))
    return by_resource, by_effect


def walk(value, path="$.", parent=None):
    if isinstance(value, dict):
        for key, child in value.items():
            here = f"{path}.{key}" if path != "$." else f"$.{key}"
            if key in RESOURCE_KEYS or key in EFFECT_KEYS:
                yield (key in RESOURCE_KEYS, str(child), here, parent)
            yield from walk(child, here, key)
    elif isinstance(value, list):
        for i, child in enumerate(value):
            yield from walk(child, f"{path}[{i}]", parent)


def entitlement_capabilities(path):
    if not path:
        return set()
    doc = json.loads(Path(path).read_text(encoding="utf-8"))
    return set(doc.get("capabilities") or [])


def lint(document, by_resource, by_effect, capabilities):
    findings = []
    for is_resource, value, where, parent in walk(document):
        table = by_resource if is_resource else by_effect
        hit = table.get(value)
        label = "resource_id" if is_resource else "effect_id"
        if hit is None:
            # 父字段不在已登记目录里时只能说“无法核对”，不能说“编号不存在”
            kind = "unknown_id" if parent in CATALOGUED_PARENTS else "unverified_id"
            findings.append({"kind": kind, "field": label, "id": value, "path": where, "parent": parent})
            continue
        domain, entry = hit
        if entry.get("vip") and VIP_CAPABILITY not in capabilities:
            findings.append({"kind": "vip_without_entitlement", "field": label, "id": value,
                             "path": where, "domain": domain, "name": entry.get("name")})

    for where, start, duration in timed_spans(document):
        if start is not None and start < 0:
            findings.append({"kind": "negative_start", "path": where, "start_us": start})
        if duration is not None and duration <= 0:
            findings.append({"kind": "non_positive_duration", "path": where, "duration_us": duration})
    return findings


def timed_spans(document, path="$"):
    if isinstance(document, dict):
        if "start_us" in document or "duration_us" in document:
            yield (path, document.get("start_us"), document.get("duration_us"))
        for key, child in document.items():
            yield from timed_spans(child, f"{path}.{key}")
    elif isinstance(document, list):
        for i, child in enumerate(document):
            yield from timed_spans(child, f"{path}[{i}]")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("documents", nargs="+", help="plan or job JSON files")
    parser.add_argument("--entitlement", help="entitlement evidence JSON; without it VIP ids are flagged")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    by_resource, by_effect = load_index()
    capabilities = entitlement_capabilities(args.entitlement)

    def blocking(findings):
        """`unverified_id` 只提示“目录未覆盖”，不阻塞。"""
        return [f for f in findings if f["kind"] != "unverified_id"]

    results = []
    for name in args.documents:
        path = Path(name)
        try:
            document = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as error:
            results.append({"document": str(path), "error": str(error), "findings": []})
            continue
        findings = lint(document, by_resource, by_effect, capabilities)
        results.append({"document": str(path), "findings": findings, "ok": not blocking(findings)})

    total = sum(len(blocking(r["findings"])) for r in results)
    payload = {
        "ok": total == 0 and all("error" not in r for r in results),
        "catalog_ids": {"resources": len(by_resource), "effects": len(by_effect)},
        "documents": results,
        "findings": total,
    }
    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print(f"catalog: {len(by_resource)} resource ids / {len(by_effect)} effect ids")
        for result in results:
            if "error" in result:
                print(f"  {result['document']}: {result['error']}")
                continue
            if result["ok"]:
                print(f"  {result['document']}: ok")
            for finding in result["findings"]:
                print(f"  {result['document']} {finding['kind']}: {finding.get('id', '')} at {finding['path']}")
    return 0 if payload["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())

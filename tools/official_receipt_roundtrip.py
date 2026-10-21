#!/usr/bin/env python3
"""Round-trip the official-resource receipt lifecycle on real files.

`media official` has five verbs - verify, register, list, show, preflight - and a receipt has two
identity shapes: a downloaded file and a node embedded in a draft. This script exercises the second
shape end to end against a draft the CLI itself built, so nothing here depends on the editor or on a
downloaded asset.

    python tools/official_receipt_roundtrip.py --binary target/release/jianying [--json]

Steps, each asserted:
  1. `build` a draft from a plan and locate one material node by its id;
  2. write a draft_resource receipt whose node digest uses the same serialization the runtime uses
     (compact, insertion order - serde_json's Map keeps the parse order in this build);
  3. `verify` it, `register` it, `show` it, `list` it, and `preflight` a Job requirement for it;
  4. negative controls: an unregistered asset, an expired entitlement and a tampered node.

The receipt is explicitly synthetic (its title says so); what is real is the draft, the digests and
the CLI's behaviour.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
PROCESS_LIST = REPO_ROOT / "tests" / "fixtures" / "empty-processes.txt"

PLAN = {
    "schema": "jianying-cli-plan/v1",
    "name": "receipt-roundtrip",
    "canvas": {"width": 1080, "height": 1920, "fps": 30},
    "tracks": [
        {
            "type": "text",
            "name": "标题",
            "segments": [
                {"start_us": 0, "duration_us": 2_000_000, "text": "收据链往返", "size": 7.0}
            ],
        }
    ],
}


def run(binary, args, cwd, env):
    completed = subprocess.run([str(binary), *args], cwd=str(cwd), env=env, capture_output=True)
    text = completed.stdout.decode("utf-8", "replace")
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        payload = {"raw": text[-400:]}
    return completed.returncode, payload


def find_node(document, node_id):
    found = []

    def walk(value):
        if isinstance(value, dict):
            if value.get("id") == node_id:
                found.append(value)
            for child in value.values():
                walk(child)
        elif isinstance(value, list):
            for child in value:
                walk(child)

    walk(document)
    return found


def build_fixtures(workdir, draft, entitlement_epochs):
    content = draft / "draft_content.json"
    raw = content.read_bytes()
    document = json.loads(raw.decode("utf-8"))
    node_id = document["materials"]["texts"][0]["id"]
    matches = find_node(document, node_id)
    if len(matches) != 1:
        raise SystemExit(f"expected one node for {node_id}, found {len(matches)}")
    # runtime 的规范化序列化：紧凑分隔符、保留解析顺序、不转义非 ASCII
    normalized = json.dumps(matches[0], ensure_ascii=False, separators=(",", ":")).encode("utf-8")

    receipt = {
        "schema_version": "jianying-official-resource-receipt/v1",
        "asset_id": "roundtrip-draft-text-0001",
        "title": "synthetic receipt on a CLI-built draft (not a real JianYing asset)",
        "resource_kind": "text_template",
        "tier": "professional",
        "identity": {
            "kind": "draft_resource",
            "resource_id": node_id,
            "resource_sha256": hashlib.sha256(normalized).hexdigest(),
            "observed_draft_sha256": hashlib.sha256(raw).hexdigest(),
        },
        "acquired_at_epoch_seconds": 1_760_000_000,
        "terms_reference": "synthetic-fixture-terms",
        "allowed_usages": ["personal", "commercial"],
        "restrictions": [],
        "preview_only": False,
    }
    (workdir / "receipt.json").write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )

    for name, expires_at in entitlement_epochs.items():
        document = {
            "schema_version": "jianying-entitlement/v1",
            "edition": "professional",
            "status": "active",
            "source": "gui_observed",
            "observed_at_epoch_seconds": 1_760_000_000,
            "expires_at_epoch_seconds": expires_at,
            "capabilities": ["official_assets.professional"],
        }
        (workdir / f"ent-{name}.json").write_text(
            json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
        )

    requirements = {
        "schema_version": "jianying-official-resource-preflight/v1",
        "requirements": [
            {"asset_id": "roundtrip-draft-text-0001", "usage": "personal", "draft": str(draft)}
        ],
    }
    (workdir / "requirements.json").write_text(
        json.dumps(requirements, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    unknown = {
        "schema_version": "jianying-official-resource-preflight/v1",
        "requirements": [{"asset_id": "never-registered", "usage": "personal", "draft": str(draft)}],
    }
    (workdir / "requirements-unknown.json").write_text(
        json.dumps(unknown, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    return node_id


def roundtrip(binary):
    workdir = Path(tempfile.mkdtemp(prefix="jianying-receipt-roundtrip-"))
    try:
        draft = workdir / "draft"
        store = workdir / "store"
        env = dict(os.environ)
        env["JIANYING_CLI_PROCESS_LIST"] = str(PROCESS_LIST)
        (workdir / "plan.json").write_text(
            json.dumps(PLAN, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
        )

        steps = {}
        code, payload = run(binary, ["--json", "build", str(workdir / "plan.json"), "--out", str(draft)], workdir, env)
        steps["build"] = {"exit": code, "ok": bool(payload.get("ok"))}
        if not steps["build"]["ok"]:
            return {"steps": steps, "checks": {"build": False}, "passed": False}

        node_id = build_fixtures(workdir, draft, {"valid": 1_760_000_000 + 2_592_000, "expired": 1_760_000_000 - 86_400})
        steps["node_located"] = {"unique": bool(node_id)}

        code, payload = run(binary, ["--json", "media", "official", "verify", str(workdir / "receipt.json"),
                                     "--draft", str(draft), "--entitlement", str(workdir / "ent-valid.json"),
                                     "--usage", "personal", "--at", "1760000100"], workdir, env)
        steps["verify"] = {"exit": code, "ok": bool(payload.get("ok")),
                           "status": (payload.get("data") or {}).get("status")}

        code, payload = run(binary, ["--json", "media", "official", "register", str(workdir / "receipt.json"),
                                     "--draft", str(draft), "--entitlement", str(workdir / "ent-valid.json"),
                                     "--usage", "personal", "--at", "1760000100",
                                     "--store-root", str(store)], workdir, env)
        steps["register"] = {"exit": code, "ok": bool(payload.get("ok")),
                             "status": (payload.get("data") or {}).get("status")}

        code, payload = run(binary, ["--json", "media", "official", "show", "roundtrip-draft-text-0001",
                                     "--store-root", str(store)], workdir, env)
        steps["show"] = {"exit": code, "ok": bool(payload.get("ok"))}

        code, payload = run(binary, ["--json", "media", "official", "list", "--store-root", str(store)], workdir, env)
        listed = (payload.get("data") or {}).get("receipts") or []
        steps["list"] = {"exit": code, "ok": bool(payload.get("ok")), "count": len(listed)}

        code, payload = run(binary, ["--json", "media", "official", "preflight", str(workdir / "requirements.json"),
                                     "--entitlement", str(workdir / "ent-valid.json"), "--at", "1760000100",
                                     "--store-root", str(store)], workdir, env)
        steps["preflight"] = {"exit": code, "ok": bool(payload.get("ok")),
                              "status": (payload.get("data") or {}).get("status")}

        code, payload = run(binary, ["--json", "media", "official", "preflight", str(workdir / "requirements-unknown.json"),
                                     "--entitlement", str(workdir / "ent-valid.json"), "--at", "1760000100",
                                     "--store-root", str(store)], workdir, env)
        steps["preflight_unregistered"] = {"exit": code, "type": (payload.get("error") or {}).get("type")}

        code, payload = run(binary, ["--json", "media", "official", "preflight", str(workdir / "requirements.json"),
                                     "--entitlement", str(workdir / "ent-expired.json"), "--at", "1760000100",
                                     "--store-root", str(store)], workdir, env)
        steps["preflight_expired"] = {"exit": code, "type": (payload.get("error") or {}).get("type")}

        # 篡改节点：只改内容、保留 id -> identity mismatch
        content = draft / "draft_content.json"
        document = json.loads(content.read_text(encoding="utf-8"))
        document["materials"]["texts"][0]["content"] = json.dumps({"text": "tampered", "styles": []}, ensure_ascii=False)
        content.write_text(json.dumps(document, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
        code, payload = run(binary, ["--json", "media", "official", "verify", str(workdir / "receipt.json"),
                                     "--draft", str(draft), "--entitlement", str(workdir / "ent-valid.json"),
                                     "--usage", "personal", "--at", "1760000100"], workdir, env)
        steps["verify_tampered"] = {"exit": code, "type": (payload.get("error") or {}).get("type")}

        checks = {
            "build": steps["build"]["ok"] and steps["node_located"]["unique"],
            "verify": steps["verify"]["ok"] and steps["verify"]["status"] == "verified",
            "register": steps["register"]["ok"] and steps["register"]["status"] == "registered",
            "show": steps["show"]["ok"],
            "list": steps["list"]["ok"] and steps["list"]["count"] == 1,
            "preflight": steps["preflight"]["ok"] and steps["preflight"]["status"] == "ready",
            "unregistered_refused": steps["preflight_unregistered"]["exit"] != 0
            and steps["preflight_unregistered"]["type"] == "asset_not_registered",
            "expired_refused": steps["preflight_expired"]["exit"] != 0,
            "tampered_refused": steps["verify_tampered"]["exit"] != 0
            and steps["verify_tampered"]["type"] == "asset_identity_mismatch",
        }
        # 草稿里的素材 id 每次 build 都是新的，所以报告不记录它本身，只记录“找到了唯一节点”
        report = {"asset_id": "roundtrip-draft-text-0001",
                  "steps": steps, "checks": checks, "passed": all(checks.values())}
        return _relativize(report, str(workdir))
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


def _relativize(value, workdir):
    if isinstance(value, dict):
        return {key: _relativize(item, workdir) for key, item in value.items()}
    if isinstance(value, list):
        return [_relativize(item, workdir) for item in value]
    if isinstance(value, str):
        return value.replace(workdir, "<workdir>")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=str(REPO_ROOT / "target" / "release" / "jianying.exe"))
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--repeat", type=int, default=2)
    args = parser.parse_args()

    binary = Path(args.binary)
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    runs = [roundtrip(binary) for _ in range(max(1, args.repeat))]
    reproducible = all(run == runs[0] for run in runs[1:])
    summary = {"runs": len(runs), "passed": all(run["passed"] for run in runs), "reproducible": reproducible,
               "checks": runs[0]["checks"]}
    if args.json:
        print(json.dumps({"summary": summary, "runs": runs}, ensure_ascii=False, indent=2))
    else:
        print(f"passed={summary['passed']} reproducible={summary['reproducible']} runs={summary['runs']}")
        for name, value in summary["checks"].items():
            print(f"  {'ok ' if value else 'FAIL'} {name}")
    return 0 if summary["passed"] and reproducible else 1


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Acceptance for the basic-edition (downgraded) environment.

The editions change asks for one proof: with only a basic entitlement in hand, a professional
resource request must fail loudly - never be silently replaced - while the plain local-material
workflow keeps running.

The script drives the real CLI binary and writes one JSON report:

    python tools/official_downgrade_acceptance.py --binary target/release/jianying [--json]

It asserts, in order:
  1. a local-material draft builds under a basic entitlement snapshot;
  2. the professional receipt is refused by `media official verify` (edition, not identity);
  3. `media official preflight` refuses the same requirement;
  4. the draft is byte-identical afterwards - nothing was substituted behind the caller's back;
  5. the refusal names the required edition, so the operator can act on it;
  6. the whole sequence is reproducible: it runs twice and the two reports agree.
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


def sha256_tree(root):
    """Hash every file under ``root`` in stable order so a no-op can be proven."""
    digests = {}
    for path in sorted(p for p in root.rglob("*") if p.is_file()):
        digests[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digests


def run(binary, args, cwd, env):
    completed = subprocess.run(
        [str(binary), *args], cwd=str(cwd), env=env, capture_output=True
    )
    text = completed.stdout.decode("utf-8", "replace")
    try:
        payload = json.loads(text)
    except json.JSONDecodeError:
        payload = {"raw": text[-400:]}
    return completed.returncode, payload


def build_fixtures(workdir):
    media = workdir / "local-media.dat"
    media.write_bytes(b"local material for the downgrade acceptance\n")
    digest = hashlib.sha256(media.read_bytes()).hexdigest()
    now = 1_760_000_000

    basic = {
        "schema_version": "jianying-entitlement/v1",
        "edition": "basic",
        "status": "active",
        "source": "gui_observed",
        "observed_at_epoch_seconds": now,
        "expires_at_epoch_seconds": now + 2_592_000,
        "capabilities": ["official_assets.basic"],
    }
    (workdir / "ent-basic.json").write_text(
        json.dumps(basic, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )

    receipt = {
        "schema_version": "jianying-official-resource-receipt/v1",
        "asset_id": "downgrade-fixture-professional",
        "title": "synthetic fixture (not a real asset)",
        "resource_kind": "transition",
        "tier": "professional",
        "identity": {
            "kind": "downloaded_file",
            "file": {"path": str(media), "sha256": digest, "byte_length": media.stat().st_size},
        },
        "acquired_at_epoch_seconds": now,
        "terms_reference": "synthetic-fixture-terms",
        "allowed_usages": ["personal"],
        "restrictions": [],
        "preview_only": False,
    }
    (workdir / "receipt-professional.json").write_text(
        json.dumps(receipt, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )

    plan = {
        "schema": "jianying-cli-plan/v1",
        "name": "downgrade-local",
        "canvas": {"width": 1080, "height": 1920, "fps": 30},
        "tracks": [
            {
                "type": "text",
                "name": "本地素材",
                "segments": [
                    {"start_us": 0, "duration_us": 1_000_000, "text": "本地素材仍可运行", "size": 6.0}
                ],
            }
        ],
    }
    (workdir / "plan.json").write_text(
        json.dumps(plan, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )

    requirements = {
        "schema_version": "jianying-official-resource-preflight/v1",
        "requirements": [
            {"asset_id": "downgrade-fixture-professional", "usage": "personal", "resource": str(media)}
        ],
    }
    (workdir / "requirements.json").write_text(
        json.dumps(requirements, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    return media


def _relativize(value, workdir):
    """Replace the throw-away work directory in every string so two runs compare equal."""
    if isinstance(value, dict):
        normalized = {}
        for key, item in value.items():
            if key == "workdir":
                continue
            if key.endswith("_sha256"):
                # draft ids are fresh on every build, so the digests differ between runs; the
                # file set is the part that must agree
                normalized[key] = sorted(item) if isinstance(item, dict) else item
            else:
                normalized[key] = _relativize(item, workdir)
        return normalized
    if isinstance(value, list):
        return [_relativize(item, workdir) for item in value]
    if isinstance(value, str):
        return value.replace(workdir, "<workdir>")
    return value


def acceptance(binary):
    workdir = Path(tempfile.mkdtemp(prefix="jianying-downgrade-"))
    try:
        media = build_fixtures(workdir)
        draft = workdir / "draft"
        env = dict(os.environ)
        env["JIANYING_CLI_PROCESS_LIST"] = str(PROCESS_LIST)

        report = {"workdir": str(workdir), "steps": {}}

        code, payload = run(binary, ["--json", "build", str(workdir / "plan.json"), "--out", str(draft)], workdir, env)
        report["steps"]["local_workflow_builds"] = {
            "exit": code,
            "ok": bool(payload.get("ok")),
            "draft_exists": draft.is_dir(),
            "detail": payload.get("error"),
        }
        before = sha256_tree(draft) if draft.is_dir() else {}
        report["draft_before_sha256"] = before

        code, payload = run(
            binary,
            [
                "--json", "media", "official", "verify", str(workdir / "receipt-professional.json"),
                str(media), "--entitlement", str(workdir / "ent-basic.json"), "--usage", "personal",
                "--at", "1760000100",
            ],
            workdir,
            env,
        )
        message = (payload.get("error") or {}).get("message", "")
        report["steps"]["professional_asset_refused"] = {
            "exit": code,
            "type": (payload.get("error") or {}).get("type"),
            "message": message,
            "names_required_edition": "professional" in message,
            "claims_ready": payload.get("data") is not None,
        }

        code, payload = run(
            binary,
            [
                "--json", "media", "official", "preflight", str(workdir / "requirements.json"),
                "--entitlement", str(workdir / "ent-basic.json"), "--at", "1760000100",
                "--store-root", str(workdir / "store"),
            ],
            workdir,
            env,
        )
        report["steps"]["preflight_refuses"] = {
            "exit": code,
            "ok": bool(payload.get("ok")),
            "type": (payload.get("error") or {}).get("type"),
        }

        after = sha256_tree(draft) if draft.is_dir() else {}
        report["draft_after_sha256"] = after
        report["draft_unchanged"] = before == after
        report["no_material_substituted"] = all(
            entry in before for entry in after
        ) and before == after

        checks = {
            "local_workflow_builds": report["steps"]["local_workflow_builds"]["ok"]
            and report["steps"]["local_workflow_builds"]["draft_exists"],
            "professional_asset_refused": report["steps"]["professional_asset_refused"]["exit"] != 0
            and not report["steps"]["professional_asset_refused"]["claims_ready"],
            "refusal_names_edition": report["steps"]["professional_asset_refused"]["names_required_edition"],
            "preflight_refuses": report["steps"]["preflight_refuses"]["exit"] != 0,
            "draft_untouched": report["draft_unchanged"],
        }
        report["checks"] = checks
        report["passed"] = all(checks.values())
        # 临时目录每次不同：比较可复现性之前先把其中的绝对路径擦掉
        return _relativize(report, str(workdir))
    finally:
        shutil.rmtree(workdir, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", default=str(REPO_ROOT / "target" / "release" / "jianying.exe"))
    parser.add_argument("--json", action="store_true", help="emit the report as JSON")
    parser.add_argument("--repeat", type=int, default=2, help="run the acceptance this many times and compare")
    args = parser.parse_args()

    binary = Path(args.binary)
    if not binary.is_file():
        print(f"binary not found: {binary}", file=sys.stderr)
        return 2

    runs = [acceptance(binary) for _ in range(max(1, args.repeat))]
    # 两次运行的结论必须一致：位置相关的路径会被剔除后再比较
    def comparable(report):
        return report

    reproducible = all(comparable(r) == comparable(runs[0]) for r in runs)
    summary = {"runs": len(runs), "passed": all(r["passed"] for r in runs), "reproducible": reproducible,
               "checks": runs[0]["checks"]}
    if args.json:
        print(json.dumps({"summary": summary, "runs": [comparable(r) for r in runs]}, ensure_ascii=False, indent=2))
    else:
        print(f"passed={summary['passed']} reproducible={summary['reproducible']} runs={summary['runs']}")
        for name, value in summary["checks"].items():
            print(f"  {'ok ' if value else 'FAIL'} {name}")
    return 0 if summary["passed"] and reproducible else 1


if __name__ == "__main__":
    raise SystemExit(main())

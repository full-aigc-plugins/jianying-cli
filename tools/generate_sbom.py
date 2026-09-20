#!/usr/bin/env python3
"""Generate a compact SPDX 2.3 SBOM from Cargo's locked metadata."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import tomllib
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def creation_timestamp() -> str:
    """使用 SOURCE_DATE_EPOCH 或当前 Git 提交时间生成可复现时间戳。"""
    raw_epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if raw_epoch is None:
        raw_epoch = subprocess.run(
            ["git", "-C", str(ROOT), "show", "-s", "--format=%ct", "HEAD"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    try:
        epoch = int(raw_epoch)
    except ValueError as error:
        raise SystemExit("SOURCE_DATE_EPOCH must be an integer Unix timestamp") from error
    if epoch < 0:
        raise SystemExit("SOURCE_DATE_EPOCH must not be negative")
    return (
        datetime.fromtimestamp(epoch, timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z")
    )


def spdx_id(value: str) -> str:
    return "SPDXRef-" + re.sub(r"[^A-Za-z0-9.-]", "-", value)


def package_id(package: dict[str, object]) -> str:
    return spdx_id(f"Package-{package['name']}-{package['version']}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    metadata = json.loads(result.stdout)
    packages = sorted(metadata["packages"], key=lambda item: (item["name"], item["version"]))
    lock_path = ROOT / "Cargo.lock"
    lock_bytes = lock_path.read_bytes()
    lock_packages = tomllib.loads(lock_bytes.decode("utf-8"))["package"]
    identity = lambda package: (
        package["name"], package["version"], package.get("source")
    )
    locked_by_identity = {identity(package): package for package in lock_packages}
    metadata_identities = {identity(package) for package in packages}
    if (len(locked_by_identity) != len(lock_packages)
            or metadata_identities != set(locked_by_identity)):
        raise SystemExit("cargo metadata package graph differs from Cargo.lock")
    root_package = next(
        item for item in packages if item["name"] == "jianying-cli" and item.get("source") is None
    )
    root_id = package_id(root_package)
    document_namespace = hashlib.sha256(
        b"jianying-cli-spdx-2.3\0" + lock_bytes
    ).hexdigest()
    entries = []
    relationships = []
    for package in packages:
        identifier = package_id(package)
        license_value = package.get("license") or "NOASSERTION"
        entry = {
            "SPDXID": identifier,
            "name": package["name"],
            "versionInfo": package["version"],
            "downloadLocation": package.get("source") or "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": license_value,
            "licenseDeclared": license_value,
            "copyrightText": "NOASSERTION",
        }
        locked = locked_by_identity[identity(package)]
        if checksum := locked.get("checksum"):
            entry["checksums"] = [{"algorithm": "SHA256", "checksumValue": checksum}]
        entries.append(entry)
        if identifier != root_id:
            relationships.append(
                {
                    "spdxElementId": root_id,
                    "relationshipType": "DEPENDS_ON",
                    "relatedSpdxElement": identifier,
                }
            )
    document = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"jianying-cli-{root_package['version']}",
        "documentNamespace": f"https://full-aigc-plugins.local/sbom/{document_namespace}",
        "creationInfo": {
            "created": creation_timestamp(),
            "creators": ["Tool: jianying-cli/tools/generate_sbom.py"],
        },
        "documentDescribes": [root_id],
        "packages": entries,
        "relationships": relationships,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(args.output), "packages": len(entries)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

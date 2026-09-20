#!/usr/bin/env python3
"""合并三个已验证平台摘要，生成插件可消费的 CLI release index。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


PLATFORMS = {"darwin-arm64", "darwin-x64", "win32-x64"}
CANONICAL_REPOSITORY = "full-aigc-plugins/jianying-cli"
SHA256 = re.compile(r"^[0-9a-f]{64}$")
SEMVER_TAG = re.compile(r"^v(\d+\.\d+\.\d+)$")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def build_index(input_dir: Path, repository: str, tag: str) -> dict:
    """校验平台摘要及压缩包后构造稳定 release index。"""
    match = SEMVER_TAG.fullmatch(tag)
    if match is None:
        raise ValueError("release tag must be v<semver>")
    if repository != CANONICAL_REPOSITORY:
        raise ValueError(f"repository must be the canonical repository {CANONICAL_REPOSITORY}")
    entries = []
    for path in sorted(input_dir.rglob("*.release-entry.json")):
        entry = json.loads(path.read_text(encoding="utf-8"))
        if entry.get("schema") != "jianying-cli-release-entry/v1":
            raise ValueError(f"unsupported release entry: {path}")
        entries.append((path, entry))
    platforms = [entry.get("platform") for _, entry in entries]
    if len(platforms) != len(set(platforms)) or set(platforms) != PLATFORMS:
        raise ValueError("release index requires exactly one entry for each supported platform")

    version = match.group(1)
    capability_schemas = {entry.get("capabilitySchema") for _, entry in entries}
    if capability_schemas != {"jianying-capabilities/v1"}:
        raise ValueError("release entries do not share the supported capability schema")
    source_commits = {entry.get("sourceCommit") for _, entry in entries}
    if len(source_commits) != 1 or not re.fullmatch(r"[0-9a-f]{40}", next(iter(source_commits), "")):
        raise ValueError("release entries do not share one valid source commit")
    artifacts = {}
    for entry_path, entry in entries:
        if entry.get("version") != version:
            raise ValueError(f"{entry['platform']}: version does not match release tag")
        if entry.get("contentState") != "released" or entry.get("releaseRef") != tag:
            raise ValueError(f"{entry['platform']}: artifact is not bound to the release tag")
        archive_name = entry.get("archiveName", "")
        extension = ".zip" if entry["platform"].startswith("win32") else ".tar.gz"
        expected_archive_name = f"jianying-cli-{version}-{entry['platform']}{extension}"
        if archive_name != expected_archive_name:
            raise ValueError(
                f"{entry['platform']}: archive must use exact archive name "
                f"{expected_archive_name}"
            )
        archive = entry_path.parent / archive_name
        if not archive.is_file() or sha256(archive) != entry.get("archiveSha256"):
            raise ValueError(f"{entry['platform']}: archive is missing or checksum differs")
        checksum_sidecar = archive.with_suffix(archive.suffix + ".sha256")
        expected_checksum_line = f"{entry.get('archiveSha256')}  {archive_name}"
        if (
            not checksum_sidecar.is_file()
            or checksum_sidecar.read_text(encoding="utf-8").strip()
            != expected_checksum_line
        ):
            raise ValueError(
                f"{entry['platform']}: archive checksum sidecar is missing or invalid"
            )
        for key in (
            "archiveSha256", "binarySha256", "capabilityManifestSha256", "sbomSha256"
        ):
            if not SHA256.fullmatch(entry.get(key, "")):
                raise ValueError(f"{entry['platform']}: invalid {key}")
        artifacts[entry["platform"]] = {
            "url": (
                f"https://github.com/{repository}/releases/download/{tag}/{archive_name}"
            ),
            "archiveSha256": entry["archiveSha256"],
            "binarySha256": entry["binarySha256"],
            "capabilityManifestSha256": entry["capabilityManifestSha256"],
            "sbomSha256": entry["sbomSha256"],
        }
    return {
        "schema": "jianying-cli-release-index/v1",
        "version": version,
        "releaseRef": tag,
        "repository": repository,
        "capabilitySchema": "jianying-capabilities/v1",
        "sourceCommit": next(iter(source_commits)),
        "artifacts": {platform: artifacts[platform] for platform in sorted(PLATFORMS)},
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input-dir", type=Path, required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        index = build_index(arguments.input_dir, arguments.repository, arguments.tag)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"ERROR: {error}")
        return 1
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_text(json.dumps(index, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(arguments.output), "platforms": len(index["artifacts"])}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

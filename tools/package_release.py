#!/usr/bin/env python3
"""Build a validated release bundle around an already-built jianying binary."""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from datetime import datetime, timezone
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PLATFORMS = {"darwin-arm64", "darwin-x64", "win32-x64"}
REQUIRED_RELEASE_CAPABILITIES = (
    "capabilities",
    "schema.job_v2",
    "schema.compile_v1_compat",
    "job.run",
    "project.compile",
    "media.asr_whisper_cpp",
    "runtime.discovery",
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def version() -> str:
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    match = re.search(r'^version = "(\d+\.\d+\.\d+)"$', cargo, re.MULTILINE)
    if not match:
        raise SystemExit("cannot read package version from Cargo.toml")
    return match.group(1)


def verify_required_capabilities(capabilities: dict[str, str]) -> None:
    """拒绝缺少正式 CLI 主入口的发布二进制。"""
    for required in REQUIRED_RELEASE_CAPABILITIES:
        if capabilities.get(required) != "supported":
            raise SystemExit(f"release binary does not support {required}")


def verify_windows_runtime_profile(capabilities: list[dict]) -> None:
    """真实 Windows canary 前禁止打包器签发受支持的 Windows Runtime Profile。"""
    profile = next(
        (
            item for item in capabilities
            if isinstance(item, dict) and item.get("id") == "runtime.profile.windows"
        ),
        None,
    )
    if (
        not isinstance(profile, dict)
        or profile.get("platform") != "windows"
        or profile.get("status") != "external_dependency"
        or profile.get("availability") != "unsupported"
    ):
        raise SystemExit(
            "Windows runtime profile must remain unsupported until a validated Windows canary exists"
        )


def verify_binary(binary: Path) -> dict:
    version_result = subprocess.run(
        [str(binary), "--version"], check=True, capture_output=True, text=True
    )
    if version() not in version_result.stdout:
        raise SystemExit("binary version does not match Cargo.toml")
    capability_result = subprocess.run(
        [str(binary), "capabilities", "--json"],
        check=True,
        capture_output=True,
        text=True,
    )
    envelope = json.loads(capability_result.stdout)
    capability_entries = envelope["data"]["capabilities"]
    capabilities = {item["id"]: item["status"] for item in capability_entries}
    verify_required_capabilities(capabilities)
    verify_windows_runtime_profile(capability_entries)
    return envelope["data"]


def verify_embedded_contract(embedded: dict) -> None:
    """拒绝用过期二进制搭配当前工作树的 capability 文件。"""
    source = json.loads(
        (ROOT / "provenance" / "CAPABILITIES.json").read_text(encoding="utf-8")
    )
    source["cli_version"] = version()
    identity_fields = ("contract_state", "release_ref", "source_commit")
    for field in identity_fields:
        source.pop(field, None)
    candidate = dict(embedded)
    for field in identity_fields:
        candidate.pop(field, None)
    # command_catalog 由 CLI handler 在运行时从同一源码生成，不属于静态来源文件。
    candidate.pop("command_catalog", None)
    if candidate != source:
        raise SystemExit(
            "binary capability contract does not match provenance/CAPABILITIES.json; "
            "rebuild the binary from the current checkout"
        )


def verify_embedded_identity(embedded: dict, identity: dict) -> None:
    """保证二进制握手身份与外部制品身份完全一致。"""
    actual = {key: embedded.get(key) for key in identity}
    if actual != identity:
        raise SystemExit(
            "binary release identity does not match packaging identity; rebuild with "
            "JIANYING_BUILD_CONTRACT_STATE, JIANYING_BUILD_RELEASE_REF and "
            "JIANYING_BUILD_SOURCE_COMMIT"
        )


def verify_sbom(sbom: Path, release_version: str) -> None:
    """验证 SPDX 结构及 Cargo.lock 全量包、来源、checksum 与许可证。"""
    try:
        document = json.loads(sbom.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise SystemExit(f"SBOM is not valid JSON: {error}") from error
    if document.get("spdxVersion") != "SPDX-2.3" or document.get("dataLicense") != "CC0-1.0":
        raise SystemExit("SBOM must be an SPDX-2.3 JSON document with CC0-1.0 data license")
    packages = document.get("packages")
    if not isinstance(packages, list) or not packages:
        raise SystemExit("SBOM must declare at least one package")
    roots = [
        package for package in packages
        if package.get("name") == "jianying-cli"
        and package.get("versionInfo") == release_version
    ]
    if len(roots) != 1:
        raise SystemExit("SBOM must describe exactly one matching jianying-cli root package")
    incomplete = sorted(
        str(package.get("name", "unknown"))
        for package in packages
        if package.get("licenseDeclared") in (None, "", "NOASSERTION")
    )
    if incomplete:
        raise SystemExit(
            "SBOM packages lack declared licenses: " + ", ".join(incomplete)
        )

    identifiers = [package.get("SPDXID") for package in packages]
    if (any(not isinstance(identifier, str) or not identifier for identifier in identifiers)
            or len(identifiers) != len(set(identifiers))):
        raise SystemExit("SBOM packages must have unique non-empty SPDX identifiers")

    try:
        lock = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise SystemExit(f"cannot read Cargo.lock for SBOM verification: {error}") from error
    locked_packages = lock.get("package")
    if not isinstance(locked_packages, list) or not locked_packages:
        raise SystemExit("Cargo.lock has no package inventory")

    expected_identities = Counter(
        (str(package.get("name", "")), str(package.get("version", "")))
        for package in locked_packages
    )
    observed_identities = Counter(
        (str(package.get("name", "")), str(package.get("versionInfo", "")))
        for package in packages
    )
    if observed_identities != expected_identities:
        missing = sorted((expected_identities - observed_identities).elements())
        extra = sorted((observed_identities - expected_identities).elements())
        raise SystemExit(
            "SBOM Cargo.lock package coverage differs: "
            f"missing={missing}; extra={extra}"
        )

    expected_by_identity: dict[tuple[str, str], list[dict]] = defaultdict(list)
    observed_by_identity: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for package in locked_packages:
        expected_by_identity[(package["name"], package["version"])].append(package)
    for package in packages:
        observed_by_identity[(package["name"], package["versionInfo"])].append(package)

    for identity, expected_entries in expected_by_identity.items():
        observed_entries = observed_by_identity[identity]
        expected_sources = Counter(
            str(package.get("source") or "NOASSERTION")
            for package in expected_entries
        )
        observed_sources = Counter(
            str(package.get("downloadLocation", ""))
            for package in observed_entries
        )
        if observed_sources != expected_sources:
            raise SystemExit(
                f"SBOM package {identity[0]} {identity[1]} source differs from Cargo.lock"
            )
        for expected in expected_entries:
            checksum = expected.get("checksum")
            if not checksum:
                continue
            source = str(expected.get("source") or "NOASSERTION")
            observed = next(
                package for package in observed_entries
                if package.get("downloadLocation") == source
            )
            checksums = observed.get("checksums")
            matched = (
                isinstance(checksums, list)
                and any(
                    isinstance(item, dict)
                    and item.get("algorithm") == "SHA256"
                    and item.get("checksumValue") == checksum
                    for item in checksums
                )
            )
            if not matched:
                raise SystemExit(
                    f"SBOM package {identity[0]} {identity[1]} checksum differs from Cargo.lock"
                )


def git(*arguments: str) -> str:
    """读取当前发布 checkout 的 Git 身份。"""
    return subprocess.run(
        ["git", "-C", str(ROOT), *arguments],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.strip()


def release_epoch() -> int:
    """读取可复现归档时间；默认绑定当前 Git 提交时间。"""
    raw_epoch = os.environ.get("SOURCE_DATE_EPOCH") or git(
        "show", "-s", "--format=%ct", "HEAD"
    )
    try:
        epoch = int(raw_epoch)
    except ValueError as error:
        raise SystemExit("SOURCE_DATE_EPOCH must be an integer Unix timestamp") from error
    if epoch < 0:
        raise SystemExit("SOURCE_DATE_EPOCH must not be negative")
    return epoch


def release_identity(release_version: str, explicit_ref: str | None) -> dict:
    """区分本地 canary 与不可变 tag 发布身份。"""
    release_ref = explicit_ref
    if release_ref is None and os.environ.get("GITHUB_REF", "").startswith("refs/tags/v"):
        release_ref = os.environ.get("GITHUB_REF_NAME")
    head = git("rev-parse", "HEAD")
    dirty = bool(git("status", "--porcelain"))
    if release_ref is None:
        return {
            "contract_state": "working_tree_unreleased" if dirty else "committed_unreleased",
            "release_ref": None,
            "source_commit": head,
        }
    expected = f"v{release_version}"
    if release_ref != expected:
        raise SystemExit(f"release ref must be {expected}")
    if dirty:
        raise SystemExit("release packaging requires a clean working tree")
    tagged = git("rev-parse", f"{release_ref}^{{commit}}")
    if tagged != head:
        raise SystemExit(f"release tag {release_ref} does not point to HEAD")
    return {
        "contract_state": "released",
        "release_ref": release_ref,
        "source_commit": head,
    }


def copy_contract_files(
    bundle: Path,
    sbom: Path,
    identity: dict,
    release_version: str,
    embedded: dict,
) -> None:
    shutil.copy2(ROOT / "LICENSE", bundle / "LICENSE")
    shutil.copy2(ROOT / "THIRD_PARTY_NOTICES.md", bundle / "THIRD_PARTY_NOTICES.md")
    shutil.copytree(ROOT / "schemas", bundle / "schemas")
    provenance = bundle / "provenance"
    provenance.mkdir()
    capabilities = dict(embedded)
    if capabilities.get("cli_version") != release_version:
        raise SystemExit("capability manifest version does not match Cargo.toml")
    capabilities.update(identity)
    (provenance / "CAPABILITIES.json").write_text(
        json.dumps(capabilities, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    shutil.copy2(sbom, bundle / "SBOM.spdx.json")


def write_manifest(bundle: Path, platform: str, release_version: str) -> None:
    files = []
    for file in sorted(path for path in bundle.rglob("*") if path.is_file()):
        relative = file.relative_to(bundle).as_posix()
        files.append({"path": relative, "sha256": sha256(file), "size": file.stat().st_size})
    manifest = {
        "schema": "jianying-cli-release/v1",
        "version": release_version,
        "platform": platform,
        "capabilityManifest": "provenance/CAPABILITIES.json",
        "files": files,
    }
    (bundle / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    checksum_lines = [
        f"{sha256(file)}  {file.relative_to(bundle).as_posix()}"
        for file in sorted(path for path in bundle.rglob("*") if path.is_file())
    ]
    (bundle / "SHA256SUMS").write_text("\n".join(checksum_lines) + "\n", encoding="utf-8")


def archive_bundle(
    bundle: Path,
    output_dir: Path,
    platform: str,
    *,
    epoch: int,
) -> Path:
    """按固定路径顺序、身份和时间戳生成可恢复重跑的确定性归档。"""
    paths = [
        bundle,
        *sorted(
            bundle.rglob("*"),
            key=lambda path: path.relative_to(bundle).as_posix(),
        ),
    ]
    for path in paths:
        if not path.is_file() and not path.is_dir():
            raise SystemExit(f"release bundle contains unsupported file type: {path}")
    if platform.startswith("win32"):
        archive = output_dir / f"{bundle.name}.zip"
        # ZIP DOS 时间戳不支持 1980 年之前；正式 Git 提交时间远高于该下界。
        zip_epoch = max(epoch, 315_532_800)
        timestamp = datetime.fromtimestamp(zip_epoch, timezone.utc)
        date_time = (
            timestamp.year, timestamp.month, timestamp.day,
            timestamp.hour, timestamp.minute, timestamp.second,
        )
        with zipfile.ZipFile(
            archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9
        ) as target:
            for file in (path for path in paths if path.is_file()):
                relative = f"{bundle.name}/{file.relative_to(bundle).as_posix()}"
                info = zipfile.ZipInfo(relative, date_time=date_time)
                info.create_system = 3
                mode = 0o755 if file.stat().st_mode & 0o111 else 0o644
                info.external_attr = (0o100000 | mode) << 16
                info.compress_type = zipfile.ZIP_DEFLATED
                target.writestr(info, file.read_bytes(), compresslevel=9)
        return archive
    archive = output_dir / f"{bundle.name}.tar.gz"
    with archive.open("wb") as raw:
        with gzip.GzipFile(
            filename="", mode="wb", fileobj=raw, compresslevel=9, mtime=epoch
        ) as compressed:
            with tarfile.open(fileobj=compressed, mode="w", format=tarfile.PAX_FORMAT) as target:
                for path in paths:
                    arcname = (
                        bundle.name
                        if path == bundle
                        else f"{bundle.name}/{path.relative_to(bundle).as_posix()}"
                    )
                    info = target.gettarinfo(str(path), arcname=arcname)
                    info.mtime = epoch
                    info.uid = 0
                    info.gid = 0
                    info.uname = ""
                    info.gname = ""
                    info.pax_headers = {}
                    if info.isdir():
                        info.mode = 0o755
                        target.addfile(info)
                    else:
                        info.mode = 0o755 if path.stat().st_mode & 0o111 else 0o644
                        with path.open("rb") as source:
                            target.addfile(info, source)
    return archive


def write_release_entry(
    output_dir: Path,
    archive: Path,
    bundle: Path,
    platform: str,
    release_version: str,
    executable_name: str,
) -> Path:
    """写出供三平台 release index 聚合器消费的机器可读摘要。"""
    capabilities = json.loads(
        (bundle / "provenance" / "CAPABILITIES.json").read_text(encoding="utf-8")
    )
    entry = {
        "schema": "jianying-cli-release-entry/v1",
        "version": release_version,
        "platform": platform,
        "archiveName": archive.name,
        "archiveSha256": sha256(archive),
        "binarySha256": sha256(bundle / executable_name),
        "capabilityManifestSha256": sha256(
            bundle / "provenance" / "CAPABILITIES.json"
        ),
        "sbomSha256": sha256(bundle / "SBOM.spdx.json"),
        "capabilitySchema": capabilities["schema"],
        "contentState": capabilities["contract_state"],
        "releaseRef": capabilities["release_ref"],
        "sourceCommit": capabilities["source_commit"],
    }
    path = output_dir / f"jianying-cli-{release_version}-{platform}.release-entry.json"
    path.write_text(json.dumps(entry, indent=2) + "\n", encoding="utf-8")
    return path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--platform", choices=sorted(PLATFORMS), required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--sbom", type=Path, required=True)
    parser.add_argument("--release-ref")
    args = parser.parse_args()
    binary = args.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"binary not found: {binary}")
    sbom = args.sbom.resolve()
    if not sbom.is_file():
        raise SystemExit(f"SBOM not found: {sbom}")
    release_version = version()
    identity = release_identity(release_version, args.release_ref)
    verify_sbom(sbom, release_version)
    embedded = verify_binary(binary)
    verify_embedded_contract(embedded)
    verify_embedded_identity(embedded, identity)
    args.output_dir.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="jianying-release-") as temporary:
        bundle = Path(temporary) / f"jianying-cli-{release_version}-{args.platform}"
        bundle.mkdir()
        executable_name = "jianying.exe" if args.platform.startswith("win32") else "jianying"
        shutil.copy2(binary, bundle / executable_name)
        copy_contract_files(bundle, sbom, identity, release_version, embedded)
        write_manifest(bundle, args.platform, release_version)
        archive = archive_bundle(
            bundle, args.output_dir, args.platform, epoch=release_epoch()
        )
        entry_path = write_release_entry(
            args.output_dir,
            archive,
            bundle,
            args.platform,
            release_version,
            executable_name,
        )
    checksum = sha256(archive)
    checksum_path = archive.with_suffix(archive.suffix + ".sha256")
    checksum_path.write_text(f"{checksum}  {archive.name}\n", encoding="utf-8")
    print(json.dumps({
        "archive": str(archive),
        "sha256": checksum,
        "platform": args.platform,
        "releaseEntry": str(entry_path),
    }))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

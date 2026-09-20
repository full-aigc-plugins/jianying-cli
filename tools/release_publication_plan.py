#!/usr/bin/env python3
"""为可重试的 GitHub Draft Release 生成 fail-closed 发布计划。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
from pathlib import Path
from typing import Callable


def query_release_metadata(
    repository: str,
    release_ref: str,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> dict | None:
    """通过 REST 查询 Release；只有明确 HTTP 404 才表示尚不存在。"""
    if re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is None:
        raise ValueError("repository must be an owner/name pair")
    if re.fullmatch(r"v\d+\.\d+\.\d+", release_ref) is None:
        raise ValueError("release ref must be a v<semver> tag")
    result = runner(
        ["gh", "api", f"repos/{repository}/releases/tags/{release_ref}"],
        capture_output=True,
        text=True,
        check=False,
    )
    try:
        payload = json.loads(result.stdout) if result.stdout.strip() else None
    except json.JSONDecodeError as error:
        raise ValueError("GitHub release query returned invalid JSON") from error
    if result.returncode != 0:
        if (
            isinstance(payload, dict)
            and str(payload.get("status")) == "404"
            and payload.get("message") == "Not Found"
        ):
            # GitHub 的按 tag 端点不会返回 Draft；继续扫描鉴权用户可见的
            # Release 列表，避免创建 Draft 后被误判为仍不存在。
            listed = runner(
                [
                    "gh", "api", "--paginate", "--slurp",
                    f"repos/{repository}/releases?per_page=100",
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            if listed.returncode != 0:
                detail = listed.stderr.strip() or listed.stdout.strip() or "unknown error"
                raise ValueError(f"cannot list GitHub releases: {detail}")
            try:
                pages = json.loads(listed.stdout)
            except json.JSONDecodeError as error:
                raise ValueError("GitHub release list returned invalid JSON") from error
            if not isinstance(pages, list) or any(
                not isinstance(page, list) for page in pages
            ):
                raise ValueError("GitHub release list returned malformed pages")
            matches = [
                item
                for page in pages
                for item in page
                if isinstance(item, dict) and item.get("tag_name") == release_ref
            ]
            if not matches:
                return None
            if len(matches) != 1:
                raise ValueError("GitHub release list returned duplicate tags")
            payload = matches[0]
        else:
            detail = result.stderr.strip() or result.stdout.strip() or "unknown error"
            raise ValueError(f"cannot query GitHub release: {detail}")
    if not isinstance(payload, dict):
        raise ValueError("GitHub release query returned no object")
    assets = payload.get("assets")
    if not isinstance(assets, list):
        raise ValueError("GitHub release query omitted assets")
    return {
        "tagName": payload.get("tag_name"),
        "isDraft": payload.get("draft"),
        "isPrerelease": payload.get("prerelease"),
        "isImmutable": payload.get("immutable"),
        "assets": [{
            "name": asset.get("name"),
            "size": asset.get("size"),
            "state": asset.get("state"),
            "digest": asset.get("digest"),
        } for asset in assets if isinstance(asset, dict)],
    }


def sha256(path: Path) -> str:
    """计算待发布资产的 SHA-256。"""
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def describe_expected(path: Path) -> dict[str, object]:
    """返回与 GitHub Release asset 元数据可比较的本地描述。"""
    if not path.is_file():
        raise ValueError(f"expected asset is not a file: {path}")
    return {
        "name": path.name,
        "size": path.stat().st_size,
        "state": "uploaded",
        "digest": f"sha256:{sha256(path)}",
    }


def verify_remote_tag_output(
    output: str,
    release_ref: str,
    expected_commit: str,
) -> str:
    """解析 lightweight/annotated tag，并绑定最终 peeled commit。"""
    if re.fullmatch(r"v\d+\.\d+\.\d+", release_ref) is None:
        raise ValueError("release ref must be a v<semver> tag")
    if re.fullmatch(r"[0-9a-f]{40}", expected_commit) is None:
        raise ValueError("expected commit must be a full lowercase Git SHA")
    direct_ref = f"refs/tags/{release_ref}"
    peeled_ref = f"{direct_ref}^{{}}"
    observed: dict[str, list[str]] = {direct_ref: [], peeled_ref: []}
    for line in output.splitlines():
        fields = line.split("\t")
        if len(fields) != 2 or fields[1] not in observed:
            raise ValueError("remote tag query returned unexpected output")
        if re.fullmatch(r"[0-9a-f]{40}", fields[0]) is None:
            raise ValueError("remote tag query returned an invalid Git object")
        observed[fields[1]].append(fields[0])
    if len(observed[direct_ref]) != 1 or len(observed[peeled_ref]) > 1:
        raise ValueError("remote tag query must resolve exactly one tag")
    resolved = (
        observed[peeled_ref][0]
        if observed[peeled_ref]
        else observed[direct_ref][0]
    )
    if resolved != expected_commit:
        raise ValueError("remote tag commit differs from the release checkout")
    return resolved


def verify_remote_tag(remote: str, release_ref: str, expected_commit: str) -> str:
    """从远端重新解析 tag，防止 Draft 公开前 tag 被移动。"""
    result = subprocess.run(
        [
            "git", "ls-remote", "--tags", remote,
            f"refs/tags/{release_ref}", f"refs/tags/{release_ref}^{{}}",
        ],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise ValueError(f"cannot resolve remote release tag: {result.stderr.strip()}")
    return verify_remote_tag_output(result.stdout, release_ref, expected_commit)


def plan_publication(
    metadata: dict | None,
    expected_paths: list[Path],
    release_ref: str,
) -> dict[str, object]:
    """校验现有 Release，并决定创建、续传、发布或只读复核。"""
    if not expected_paths:
        raise ValueError("expected asset set must not be empty")
    expected: dict[str, tuple[Path, dict[str, object]]] = {}
    for path in expected_paths:
        description = describe_expected(path)
        name = str(description["name"])
        if name in expected:
            raise ValueError(f"duplicate expected asset name: {name}")
        expected[name] = (path, description)

    if metadata is None:
        return {
            "schema": "jianying-cli-release-publication-plan/v1",
            "action": "create_draft",
            "releaseRef": release_ref,
            "missing": [str(path) for path in expected_paths],
        }
    if metadata.get("tagName") != release_ref:
        raise ValueError("tag differs from expected release ref")
    if metadata.get("isPrerelease") is not False:
        raise ValueError("prerelease cannot be used for a stable CLI release")
    assets = metadata.get("assets")
    if not isinstance(assets, list):
        raise ValueError("release assets must be an array")

    observed: dict[str, dict] = {}
    for asset in assets:
        if not isinstance(asset, dict) or not isinstance(asset.get("name"), str):
            raise ValueError("release asset metadata is malformed")
        name = asset["name"]
        if name in observed:
            raise ValueError(f"duplicate release asset name: {name}")
        if name not in expected:
            raise ValueError(f"unexpected asset in release draft: {name}")
        observed[name] = asset

    for name, asset in observed.items():
        description = expected[name][1]
        if asset.get("state") != "uploaded":
            raise ValueError(f"asset is not fully uploaded: {name}")
        if asset.get("size") != description["size"]:
            raise ValueError(f"asset size differs: {name}")
        if asset.get("digest") != description["digest"]:
            raise ValueError(f"asset digest differs: {name}")

    missing = [
        str(path)
        for name, (path, _) in expected.items()
        if name not in observed
    ]
    is_draft = metadata.get("isDraft")
    is_immutable = metadata.get("isImmutable")
    if is_draft is True:
        if is_immutable is not False:
            raise ValueError("draft release must not already be immutable")
        action = "resume_draft" if missing else "publish_draft"
    elif is_draft is False:
        if is_immutable is not True:
            raise ValueError("mutable published release is forbidden")
        if missing:
            raise ValueError("published immutable release asset set is incomplete")
        action = "verify_existing"
    else:
        raise ValueError("release draft state is missing")
    return {
        "schema": "jianying-cli-release-publication-plan/v1",
        "action": action,
        "releaseRef": release_ref,
        "missing": missing,
    }


def read_paths0(path: Path) -> list[Path]:
    """读取由 find -print0 生成的资产路径列表。"""
    raw = path.read_bytes()
    return [Path(item.decode()) for item in raw.split(b"\0") if item]


def main() -> int:
    parser = argparse.ArgumentParser()
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--metadata", type=Path)
    source.add_argument("--repository")
    parser.add_argument("--expected-paths0", type=Path, required=True)
    parser.add_argument("--release-ref", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--missing-paths0", type=Path, required=True)
    parser.add_argument("--remote")
    parser.add_argument("--expected-commit")
    arguments = parser.parse_args()
    if bool(arguments.remote) != bool(arguments.expected_commit):
        raise ValueError("--remote and --expected-commit must be supplied together")
    if arguments.remote:
        verify_remote_tag(
            arguments.remote,
            arguments.release_ref,
            arguments.expected_commit,
        )
    metadata = None
    if arguments.repository:
        metadata = query_release_metadata(arguments.repository, arguments.release_ref)
    elif arguments.metadata.is_file():
        metadata = json.loads(arguments.metadata.read_text(encoding="utf-8"))
    plan = plan_publication(
        metadata,
        read_paths0(arguments.expected_paths0),
        arguments.release_ref,
    )
    arguments.output.write_text(
        json.dumps(plan, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    missing = [Path(item) for item in plan["missing"]]
    arguments.missing_paths0.write_bytes(
        b"".join(str(path).encode() + b"\0" for path in missing)
    )
    print(json.dumps({"action": plan["action"], "missing": len(missing)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

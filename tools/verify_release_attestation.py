#!/usr/bin/env python3
"""验证 GitHub 不可变 Release attestation 的提交与完整资产集合。"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path


def sha256(path: Path) -> str:
    """计算文件 SHA-256。"""
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(
    evidence: dict,
    repository: str,
    release_ref: str,
    expected_tag_object: str,
    expected_paths: list[Path],
) -> None:
    """验证 release statement 精确绑定仓库、tag 对象与资产摘要。"""
    if re.fullmatch(r"[0-9a-f]{40}", expected_tag_object) is None:
        raise ValueError("expected tag object must be a full lowercase Git SHA")
    statement = evidence.get("verificationResult", {}).get("statement")
    if not isinstance(statement, dict):
        raise ValueError("release attestation statement is missing")
    predicate = statement.get("predicate")
    if not isinstance(predicate, dict):
        raise ValueError("release attestation predicate is missing")
    if predicate.get("repository") != repository or predicate.get("tag") != release_ref:
        raise ValueError("release attestation repository or tag differs")
    subjects = statement.get("subject")
    if not isinstance(subjects, list):
        raise ValueError("release attestation subjects are missing")
    package_uri = f"pkg:github/{repository}@{release_ref}"
    packages = [item for item in subjects if isinstance(item, dict) and item.get("uri") == package_uri]
    if len(packages) != 1 or packages[0].get("digest", {}).get("sha1") != expected_tag_object:
        raise ValueError("release attestation tag object differs")
    expected = {path.name: sha256(path) for path in expected_paths}
    if len(expected) != len(expected_paths) or not expected:
        raise ValueError("expected attestation asset set is empty or duplicated")
    observed: dict[str, str] = {}
    for item in subjects:
        if not isinstance(item, dict) or "name" not in item:
            continue
        name = item.get("name")
        digest = item.get("digest", {}).get("sha256")
        if not isinstance(name, str) or not isinstance(digest, str) or name in observed:
            raise ValueError("release attestation asset subject is malformed")
        observed[name] = digest
    if observed != expected:
        raise ValueError("release attestation asset set or digest differs")


def read_paths0(path: Path) -> list[Path]:
    """读取 NUL 分隔路径。"""
    return [Path(item.decode()) for item in path.read_bytes().split(b"\0") if item]


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--release-ref", required=True)
    parser.add_argument("--expected-tag-object", required=True)
    parser.add_argument("--expected-paths0", type=Path, required=True)
    arguments = parser.parse_args()
    verify(
        json.loads(arguments.input.read_text(encoding="utf-8")),
        arguments.repository,
        arguments.release_ref,
        arguments.expected_tag_object,
        read_paths0(arguments.expected_paths0),
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

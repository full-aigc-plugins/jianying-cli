#!/usr/bin/env python3
"""只读检查 jianying-cli 正式不可变发布的 GitHub 前置条件。"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Callable


ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "full-aigc-plugins/jianying-cli"


class RemoteQueryError(RuntimeError):
    """远端只读查询失败。"""


def release_tag_ruleset_fixture() -> list[dict]:
    """返回测试与文档共用的最小安全 tag ruleset 形状。"""
    return [{
        "target": "tag",
        "enforcement": "active",
        "bypass_actors": [],
        "conditions": {
            "ref_name": {"include": ["refs/tags/v*"], "exclude": []},
        },
        "rules": [{"type": "deletion"}, {"type": "non_fast_forward"}],
    }]


def release_tags_are_protected(evidence: object) -> bool:
    """确认 v* tag 由无 bypass 的 active ruleset 禁止删除与移动。"""
    if not isinstance(evidence, list):
        return False
    for ruleset in evidence:
        if not isinstance(ruleset, dict):
            continue
        conditions = ruleset.get("conditions")
        ref_name = conditions.get("ref_name") if isinstance(conditions, dict) else None
        include = ref_name.get("include") if isinstance(ref_name, dict) else None
        exclude = ref_name.get("exclude") if isinstance(ref_name, dict) else None
        rules = ruleset.get("rules")
        rule_types = {
            rule.get("type") for rule in rules or [] if isinstance(rule, dict)
        }
        if (
            ruleset.get("target") == "tag"
            and ruleset.get("enforcement") == "active"
            and ruleset.get("bypass_actors") == []
            and isinstance(include, list)
            and any(pattern in {"~ALL", "refs/tags/v*"} for pattern in include)
            and isinstance(exclude, list)
            and not exclude
            and {"deletion", "non_fast_forward"}.issubset(rule_types)
        ):
            return True
    return False


def load_release_tag_rulesets(
    query: Callable[[list[str]], object],
    repository: str,
) -> list[dict]:
    """列举适用 ruleset，并沿 GitHub 正典 self link 读取完整详情。"""
    summaries = query([
        "api", f"repos/{repository}/rulesets?includes_parents=true",
    ])
    if not isinstance(summaries, list):
        raise RemoteQueryError("GitHub ruleset list is not an array")
    details: list[dict] = []
    trusted_url = re.compile(
        r"https://api\.github\.com/(?:repos/[^/]+/[^/]+|orgs/[^/]+|"
        r"enterprises/[^/]+)/rulesets/\d+"
    )
    for summary in summaries:
        if not isinstance(summary, dict):
            raise RemoteQueryError("GitHub ruleset summary is malformed")
        if all(key in summary for key in ("conditions", "rules", "bypass_actors")):
            details.append(summary)
            continue
        links = summary.get("_links")
        self_link = links.get("self") if isinstance(links, dict) else None
        href = self_link.get("href") if isinstance(self_link, dict) else None
        if not isinstance(href, str) or trusted_url.fullmatch(href) is None:
            raise RemoteQueryError("GitHub ruleset summary omitted a trusted detail URL")
        detail = query(["api", href])
        if not isinstance(detail, dict):
            raise RemoteQueryError("GitHub ruleset detail is malformed")
        details.append(detail)
    return details


def gh_json(arguments: list[str]) -> object:
    """执行 gh 只读查询并解析 JSON。"""
    result = subprocess.run(
        ["gh", *arguments],
        capture_output=True,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        raise RemoteQueryError(result.stderr.strip() or result.stdout.strip())
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as error:
        raise RemoteQueryError(f"gh returned invalid JSON: {error}") from error


def check(
    identifier: str,
    query: Callable[[], object],
    predicate: Callable[[object], bool],
) -> dict:
    """执行单个只读检查并返回稳定状态。"""
    try:
        evidence = query()
        return {
            "id": identifier,
            "status": "ready" if predicate(evidence) else "blocked",
            "evidence": evidence,
        }
    except RemoteQueryError as error:
        return {
            "id": identifier,
            "status": "blocked",
            "evidence": {"error": str(error)},
        }


def build_report(
    root: Path = ROOT,
    query: Callable[[list[str]], object] = gh_json,
) -> dict:
    """构造发布前置条件与正式 Release 完成状态报告。"""
    cargo = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    version = cargo["package"]["version"]
    release_ref = f"v{version}"
    checks = [
        check(
            "repository.immutable_releases",
            lambda: query(["api", f"repos/{REPOSITORY}/immutable-releases"]),
            lambda evidence: evidence.get("enabled") is True,
        ),
        check(
            "repository.release_tag_protection",
            lambda: load_release_tag_rulesets(query, REPOSITORY),
            release_tags_are_protected,
        ),
        check(
            "release.expected",
            lambda: query([
                "release",
                "view",
                release_ref,
                "--repo",
                REPOSITORY,
                "--json",
                "tagName,isDraft,isPrerelease,isImmutable,publishedAt",
            ]),
            lambda evidence: (
                evidence.get("tagName") == release_ref
                and evidence.get("isDraft") is False
                and evidence.get("isPrerelease") is False
                and evidence.get("isImmutable") is True
            ),
        ),
    ]
    by_id = {item["id"]: item for item in checks}
    prerequisite_ids = {
        "repository.immutable_releases",
        "repository.release_tag_protection",
    }
    prerequisites_ready = all(
        by_id[identifier]["status"] == "ready"
        for identifier in prerequisite_ids
    )
    release_complete = (
        prerequisites_ready and by_id["release.expected"]["status"] == "ready"
    )
    return {
        "schema": "jianying-cli-remote-release-preflight/v1",
        "prerequisitesReady": prerequisites_ready,
        "releaseComplete": release_complete,
        "expected": {
            "repository": REPOSITORY,
            "version": version,
            "ref": release_ref,
        },
        "checks": checks,
        "blockedPrerequisites": [
            item["id"]
            for item in checks
            if item["id"] in prerequisite_ids and item["status"] == "blocked"
        ],
        "blocked": [item["id"] for item in checks if item["status"] == "blocked"],
    }


def main() -> int:
    """输出报告；只有构建前置条件全部满足时返回成功。"""
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()
    report = build_report()
    rendered = json.dumps(report, ensure_ascii=False, indent=2) + "\n"
    if arguments.output:
        arguments.output.write_text(rendered, encoding="utf-8")
    else:
        print(rendered, end="")
    return 0 if report["prerequisitesReady"] else 1


if __name__ == "__main__":
    sys.exit(main())

"""三平台 release index 聚合器的离线契约测试。"""

import hashlib
import importlib.util
import json
import os
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest
from unittest.mock import patch
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "build_release_index", ROOT / "tools" / "build_release_index.py"
)
INDEX = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INDEX)
PACKAGE_SPEC = importlib.util.spec_from_file_location(
    "package_release", ROOT / "tools" / "package_release.py"
)
PACKAGE = importlib.util.module_from_spec(PACKAGE_SPEC)
PACKAGE_SPEC.loader.exec_module(PACKAGE)
PREFLIGHT_SPEC = importlib.util.spec_from_file_location(
    "release_preflight", ROOT / "tools" / "release_preflight.py"
)
PREFLIGHT = importlib.util.module_from_spec(PREFLIGHT_SPEC)
PREFLIGHT_SPEC.loader.exec_module(PREFLIGHT)
PUBLICATION_SPEC = importlib.util.spec_from_file_location(
    "release_publication_plan", ROOT / "tools" / "release_publication_plan.py"
)
PUBLICATION = importlib.util.module_from_spec(PUBLICATION_SPEC)
PUBLICATION_SPEC.loader.exec_module(PUBLICATION)
ATTESTATION_SPEC = importlib.util.spec_from_file_location(
    "verify_release_attestation", ROOT / "tools" / "verify_release_attestation.py"
)
ATTESTATION = importlib.util.module_from_spec(ATTESTATION_SPEC)
ATTESTATION_SPEC.loader.exec_module(ATTESTATION)
UTILITY_SPEC = importlib.util.spec_from_file_location(
    "capcut_utility_differential", ROOT / "tools" / "capcut_utility_differential.py"
)
UTILITY = importlib.util.module_from_spec(UTILITY_SPEC)
UTILITY_SPEC.loader.exec_module(UTILITY)


class ReleaseIndexTests(unittest.TestCase):
    def test_capcut_utility_runner_survives_upstream_mid_codepoint_truncation(self) -> None:
        truncated = subprocess.CompletedProcess([], 0, b'[{"name":"\xe4\xb8', b"")
        with patch.object(UTILITY.subprocess, "run", return_value=truncated) as run:
            output = UTILITY.invoke(["node", "dist/index.js", "enums"])

        self.assertEqual(output, '[{"name":"\ufffd')
        self.assertTrue(run.call_args.kwargs["capture_output"])
        self.assertNotIn("text", run.call_args.kwargs)

    def test_release_packager_decodes_binary_output_as_utf8(self) -> None:
        capability_entries = [
            {"id": capability, "status": "supported"}
            for capability in PACKAGE.REQUIRED_RELEASE_CAPABILITIES
        ]
        capability_entries.append({
            "id": "runtime.profile.windows",
            "platform": "windows",
            "status": "external_dependency",
            "availability": "unsupported",
            "label": "剪映 Windows 运行时",
        })
        envelope = {"data": {"capabilities": capability_entries}}
        results = [
            subprocess.CompletedProcess([], 0, "jianying 1.6.12\n", ""),
            subprocess.CompletedProcess([], 0, json.dumps(envelope, ensure_ascii=False), ""),
        ]
        with patch.object(PACKAGE, "version", return_value="1.6.12"), patch.object(
            PACKAGE.subprocess, "run", side_effect=results
        ) as run:
            PACKAGE.verify_binary(Path("jianying.exe"))

        self.assertEqual(run.call_count, 2)
        for invocation in run.call_args_list:
            self.assertEqual(invocation.kwargs.get("encoding"), "utf-8")
            self.assertNotIn("text", invocation.kwargs)

    def test_release_attestation_binds_commit_and_exact_assets(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            asset = Path(temporary) / "release.zip"
            asset.write_bytes(b"release")
            commit = "a" * 40
            evidence = {"verificationResult": {"statement": {
                "predicate": {
                    "repository": "full-aigc-plugins/jianying-cli",
                    "tag": "v1.6.5",
                },
                "subject": [
                    {"uri": "pkg:github/full-aigc-plugins/jianying-cli@v1.6.5",
                     "digest": {"sha1": commit}},
                    {"name": asset.name,
                     "digest": {"sha256": hashlib.sha256(b"release").hexdigest()}},
                ],
            }}}
            ATTESTATION.verify(
                evidence, "full-aigc-plugins/jianying-cli", "v1.6.5",
                commit, [asset],
            )
            with self.assertRaisesRegex(ValueError, "tag object differs"):
                ATTESTATION.verify(
                    evidence, "full-aigc-plugins/jianying-cli", "v1.6.5",
                    "b" * 40, [asset],
                )

    def test_release_query_treats_only_explicit_http_404_as_absent(self) -> None:
        def missing(arguments, **_kwargs):
            if "--slurp" in arguments:
                return subprocess.CompletedProcess([], 0, "[]\n", "")
            return subprocess.CompletedProcess(
                [], 1, '{"message":"Not Found","status":"404"}\n',
                "gh: Not Found (HTTP 404)\n",
            )

        self.assertIsNone(PUBLICATION.query_release_metadata(
            "full-aigc-plugins/jianying-cli", "v1.6.5", missing,
        ))

        def forbidden(*_args, **_kwargs):
            return subprocess.CompletedProcess(
                [], 1, '{"message":"Forbidden","status":"403"}\n',
                "gh: Forbidden (HTTP 403)\n",
            )

        with self.assertRaisesRegex(ValueError, "cannot query GitHub release"):
            PUBLICATION.query_release_metadata(
                "full-aigc-plugins/jianying-cli", "v1.6.5", forbidden,
            )

    def test_release_query_recovers_tagged_draft_from_release_list(self) -> None:
        def draft(arguments, **_kwargs):
            if "--slurp" in arguments:
                return subprocess.CompletedProcess([], 0, '''[[{
                  "tag_name":"v1.6.5","draft":true,"prerelease":false,
                  "immutable":false,"assets":[]
                }]]''', "")
            return subprocess.CompletedProcess(
                [], 1, '{"message":"Not Found","status":"404"}\n', "",
            )

        metadata = PUBLICATION.query_release_metadata(
            "full-aigc-plugins/jianying-cli", "v1.6.5", draft,
        )
        self.assertTrue(metadata["isDraft"])
        self.assertEqual(metadata["assets"], [])

    def test_release_publication_binds_lightweight_and_annotated_remote_tags(self) -> None:
        release_ref = "v1.6.5"
        commit = "a" * 40
        tag_object = "b" * 40
        lightweight = f"{commit}\trefs/tags/{release_ref}\n"
        annotated = (
            f"{tag_object}\trefs/tags/{release_ref}\n"
            f"{commit}\trefs/tags/{release_ref}^{{}}\n"
        )
        self.assertEqual(
            PUBLICATION.verify_remote_tag_output(lightweight, release_ref, commit),
            commit,
        )
        self.assertEqual(
            PUBLICATION.verify_remote_tag_output(annotated, release_ref, commit),
            commit,
        )
        with self.assertRaisesRegex(ValueError, "remote tag commit differs"):
            PUBLICATION.verify_remote_tag_output(annotated, release_ref, "c" * 40)

    def test_release_publication_plan_recovers_only_clean_drafts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            first = root / "first.tar.gz"
            second = root / "second.json"
            first.write_bytes(b"first")
            second.write_bytes(b"second")
            expected = [first, second]

            created = PUBLICATION.plan_publication(None, expected, "v1.6.5")
            self.assertEqual(created["action"], "create_draft")
            self.assertEqual(created["missing"], [str(first), str(second)])

            partial = PUBLICATION.plan_publication(
                {
                    "tagName": "v1.6.5",
                    "isDraft": True,
                    "isPrerelease": False,
                    "isImmutable": False,
                    "assets": [PUBLICATION.describe_expected(first)],
                },
                expected,
                "v1.6.5",
            )
            self.assertEqual(partial["action"], "resume_draft")
            self.assertEqual(partial["missing"], [str(second)])

            complete_assets = [
                PUBLICATION.describe_expected(first),
                PUBLICATION.describe_expected(second),
            ]
            complete = PUBLICATION.plan_publication(
                {
                    "tagName": "v1.6.5",
                    "isDraft": True,
                    "isPrerelease": False,
                    "isImmutable": False,
                    "assets": complete_assets,
                },
                expected,
                "v1.6.5",
            )
            self.assertEqual(complete["action"], "publish_draft")
            self.assertEqual(complete["missing"], [])

            immutable = PUBLICATION.plan_publication(
                {
                    "tagName": "v1.6.5",
                    "isDraft": False,
                    "isPrerelease": False,
                    "isImmutable": True,
                    "assets": complete_assets,
                },
                expected,
                "v1.6.5",
            )
            self.assertEqual(immutable["action"], "verify_existing")
            self.assertEqual(immutable["missing"], [])

    def test_release_publication_plan_rejects_polluted_or_drifted_release(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            asset = root / "artifact.zip"
            asset.write_bytes(b"expected")
            expected_asset = PUBLICATION.describe_expected(asset)
            base = {
                "tagName": "v1.6.5",
                "isDraft": True,
                "isPrerelease": False,
                "isImmutable": False,
                "assets": [expected_asset],
            }

            cases = {
                "unexpected asset": {**base, "assets": [
                    expected_asset,
                    {**expected_asset, "name": "unexpected.bin"},
                ]},
                "digest differs": {**base, "assets": [
                    {**expected_asset, "digest": "sha256:" + "0" * 64},
                ]},
                "mutable published": {**base, "isDraft": False},
                "prerelease": {**base, "isPrerelease": True},
                "tag differs": {**base, "tagName": "v1.5.0"},
            }
            for message, metadata in cases.items():
                with self.subTest(message=message):
                    with self.assertRaisesRegex(ValueError, message):
                        PUBLICATION.plan_publication(
                            metadata, [asset], "v1.6.5"
                        )

    def test_github_actions_and_rust_toolchain_are_immutable(self) -> None:
        action_reference = re.compile(r"^\s*-\s+uses:\s+([^\s#]+)", re.MULTILINE)
        immutable_action = re.compile(r"^[^/\s]+/[^@\s]+@[0-9a-f]{40}$")
        workflows = sorted((ROOT / ".github" / "workflows").glob("*.yml"))
        self.assertTrue(workflows)
        for workflow in workflows:
            references = action_reference.findall(workflow.read_text(encoding="utf-8"))
            self.assertTrue(references, f"{workflow.name} contains no action references")
            for reference in references:
                self.assertRegex(
                    reference,
                    immutable_action,
                    f"{workflow.name} uses a mutable action reference: {reference}",
                )

        toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
        self.assertRegex(
            toolchain,
            re.compile(r'^channel = "\d+\.\d+\.\d+"$', re.MULTILINE),
        )

        release_workflow = (
            ROOT / ".github" / "workflows" / "release-artifacts.yml"
        ).read_text(encoding="utf-8")
        self.assertIn("concurrency:\n  group: ${{ github.workflow }}-${{ github.ref }}", release_workflow)
        self.assertIn("cancel-in-progress: false", release_workflow)
        self.assertNotIn("--clobber", release_workflow)
        self.assertIn('gh release create "$GITHUB_REF_NAME" --verify-tag --generate-notes --draft', release_workflow)
        self.assertIn('gh release edit "$GITHUB_REF_NAME" --draft=false', release_workflow)
        self.assertIn("immutable-releases", release_workflow)
        self.assertGreaterEqual(
            release_workflow.count('--repository "$GITHUB_REPOSITORY"'), 3,
        )
        self.assertIn("gh release verify-asset", release_workflow)
        self.assertIn("existing immutable release matches this build", release_workflow)
        self.assertIn("published release asset set or digest differs", release_workflow)
        self.assertIn("asset attestation not visible yet", release_workflow)
        self.assertIn("for attempt in $(seq 1 30)", release_workflow)
        self.assertIn('gh release download "$GITHUB_REF_NAME"', release_workflow)
        self.assertIn("tools/release_publication_plan.py", release_workflow)
        self.assertGreaterEqual(release_workflow.count("--remote origin"), 2)
        self.assertGreaterEqual(release_workflow.count('--expected-commit "$(git rev-parse HEAD)"'), 2)
        self.assertIn("resuming a byte-identical partial draft release", release_workflow)
        self.assertNotIn('gh release view "$GITHUB_REF_NAME"', release_workflow)
        self.assertIn(
            'test "$(jq -r .action "$release_plan")" = "verify_existing"',
            release_workflow,
        )
        self.assertIn('if [ "$publication_action" != "verify_existing" ]; then', release_workflow)
        self.assertNotIn("release already exists; refusing to replace immutable assets", release_workflow)
        self.assertIn("event_type=cli-released", release_workflow)
        self.assertIn(
            "repos/full-aigc-plugins/jianying-edit-plugin/dispatches",
            release_workflow,
        )
        self.assertIn("client_payload[index_sha256]", release_workflow)
        self.assertIn("source-parity-gate:", release_workflow)
        self.assertIn("needs: source-parity-gate", release_workflow)
        self.assertIn("tools/parity_run.py", release_workflow)
        self.assertIn("tools/capcut_project_differential.py", release_workflow)
        self.assertIn("tools/capcut_timeline_differential.py", release_workflow)
        self.assertIn("tools/capcut_media_analysis_differential.py", release_workflow)
        self.assertIn("tools/capcut_media_mutation_differential.py", release_workflow)
        self.assertIn("tools/capcut_material_discovery_differential.py", release_workflow)
        self.assertIn("tools/capcut_interchange_differential.py", release_workflow)
        self.assertIn("tools/capcut_utility_differential.py", release_workflow)
        self.assertIn("tools/check_provenance.py", release_workflow)
        self.assertGreaterEqual(release_workflow.count("actions/setup-python@"), 3)
        self.assertIn("python-version: \"3.12\"", release_workflow)
        self.assertIn("actions/setup-node@", release_workflow)
        self.assertIn("node-version: \"24\"", release_workflow)
        self.assertIn("Validate release tag and immutable repository setting", release_workflow)
        self.assertIn("tag ${GITHUB_REF_NAME} does not match Cargo version", release_workflow)
        self.assertIn("tools/release_preflight.py", release_workflow)
        self.assertGreaterEqual(
            release_workflow.count(
                "secrets.RELEASE_RULESET_READ_TOKEN || github.token"
            ),
            3,
        )
        self.assertIn("--output release-preflight.json", release_workflow)
        self.assertIn("name: jianying-cli-remote-preflight", release_workflow)
        self.assertIn("path: release-preflight.json", release_workflow)
        self.assertNotIn("pattern: jianying-cli-*", release_workflow)
        for platform in ("darwin-arm64", "darwin-x64", "win32-x64"):
            self.assertIn(f"name: jianying-cli-{platform}", release_workflow)
            self.assertIn(f"path: dist/{platform}", release_workflow)
        self.assertIn('if [[ "$GITHUB_REF" == refs/tags/v* ]]', release_workflow)
        self.assertIn('exit "$preflight_status"', release_workflow)
        self.assertLess(
            release_workflow.index("tools/release_preflight.py"),
            release_workflow.index("cargo fmt --all --check"),
        )

    def test_release_preflight_distinguishes_build_prerequisites_from_completion(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text(
                '[package]\nname = "jianying-cli"\nversion = "1.6.5"\n',
                encoding="utf-8",
            )

            def query(arguments: list[str]) -> dict:
                if arguments[0] == "api" and "immutable-releases" in arguments[1]:
                    return {"enabled": True}
                if arguments[0] == "api" and "rulesets" in arguments[1]:
                    return PREFLIGHT.release_tag_ruleset_fixture()
                raise PREFLIGHT.RemoteQueryError("release not found")

            report = PREFLIGHT.build_report(root, query)
            self.assertTrue(report["prerequisitesReady"])
            self.assertFalse(report["releaseComplete"])
            self.assertEqual(report["blockedPrerequisites"], [])
            self.assertEqual(report["blocked"], ["release.expected"])
            self.assertEqual(report["expected"]["ref"], "v1.6.5")

    def test_release_preflight_blocks_disabled_immutable_releases(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text(
                '[package]\nname = "jianying-cli"\nversion = "1.6.5"\n',
                encoding="utf-8",
            )

            def query(arguments: list[str]) -> dict:
                if arguments[0] == "api" and "immutable-releases" in arguments[1]:
                    return {"enabled": False}
                if arguments[0] == "api" and "rulesets" in arguments[1]:
                    return PREFLIGHT.release_tag_ruleset_fixture()
                return {
                    "tagName": "v1.6.5",
                    "isDraft": False,
                    "isPrerelease": False,
                    "isImmutable": True,
                    "publishedAt": "2026-09-20T00:00:00Z",
                }

            report = PREFLIGHT.build_report(root, query)
            self.assertFalse(report["prerequisitesReady"])
            self.assertFalse(report["releaseComplete"])
            self.assertEqual(
                report["blockedPrerequisites"],
                ["repository.immutable_releases"],
            )

    def test_release_preflight_requires_active_non_bypassable_tag_protection(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.toml").write_text(
                '[package]\nname = "jianying-cli"\nversion = "1.6.5"\n',
                encoding="utf-8",
            )

            def query(arguments: list[str]) -> dict:
                if "immutable-releases" in " ".join(arguments):
                    return {"enabled": True}
                if "rulesets" in " ".join(arguments):
                    return []
                raise PREFLIGHT.RemoteQueryError("release not found")

            report = PREFLIGHT.build_report(root, query)
            self.assertFalse(report["prerequisitesReady"])
            self.assertEqual(
                report["blockedPrerequisites"],
                ["repository.release_tag_protection"],
            )

    def test_release_preflight_loads_ruleset_details_from_canonical_self_link(self) -> None:
        self_link = (
            "https://api.github.com/orgs/full-aigc-plugins/rulesets/42"
        )
        calls: list[list[str]] = []

        def query(arguments: list[str]) -> object:
            calls.append(arguments)
            if arguments[1] == self_link:
                return PREFLIGHT.release_tag_ruleset_fixture()[0]
            return [{"id": 42, "_links": {"self": {"href": self_link}}}]

        details = PREFLIGHT.load_release_tag_rulesets(query, PREFLIGHT.REPOSITORY)
        self.assertTrue(PREFLIGHT.release_tags_are_protected(details))
        self.assertEqual(calls[-1], ["api", self_link])

    def test_release_preflight_rejects_summary_without_trusted_detail_link(self) -> None:
        with self.assertRaisesRegex(PREFLIGHT.RemoteQueryError, "detail URL"):
            PREFLIGHT.load_release_tag_rulesets(
                lambda _arguments: [{"id": 42}], PREFLIGHT.REPOSITORY
            )

    def test_packager_requires_job_and_compile_entrypoints(self) -> None:
        statuses = {
            capability: "supported"
            for capability in PACKAGE.REQUIRED_RELEASE_CAPABILITIES
        }
        PACKAGE.verify_required_capabilities(statuses)
        for capability in PACKAGE.REQUIRED_RELEASE_CAPABILITIES:
            incomplete = dict(statuses)
            incomplete[capability] = "partial"
            with self.subTest(capability=capability):
                with self.assertRaisesRegex(SystemExit, capability.replace(".", r"\.")):
                    PACKAGE.verify_required_capabilities(incomplete)

    def test_packager_rejects_unvalidated_windows_runtime_profile(self) -> None:
        embedded = json.loads(
            (ROOT / "provenance" / "CAPABILITIES.json").read_text(encoding="utf-8")
        )
        capabilities = embedded["capabilities"]
        PACKAGE.verify_windows_runtime_profile(capabilities)
        profile = next(
            item for item in capabilities if item["id"] == "runtime.profile.windows"
        )
        for field, value in (
            ("status", "supported"),
            ("availability", "supported"),
            ("platform", "darwin"),
        ):
            changed = [dict(item) for item in capabilities]
            next(
                item for item in changed if item["id"] == "runtime.profile.windows"
            )[field] = value
            with self.subTest(field=field):
                with self.assertRaisesRegex(SystemExit, "Windows runtime profile"):
                    PACKAGE.verify_windows_runtime_profile(changed)

        missing = [
            item for item in capabilities if item["id"] != "runtime.profile.windows"
        ]
        with self.assertRaisesRegex(SystemExit, "Windows runtime profile"):
            PACKAGE.verify_windows_runtime_profile(missing)

    def fixture(self, root: Path, platforms: tuple[str, ...]) -> None:
        for platform in platforms:
            directory = root / platform
            directory.mkdir()
            extension = ".zip" if platform.startswith("win32") else ".tar.gz"
            archive = directory / f"jianying-cli-1.6.5-{platform}{extension}"
            archive.write_bytes(f"archive:{platform}".encode())
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            archive.with_suffix(archive.suffix + ".sha256").write_text(
                f"{digest}  {archive.name}\n", encoding="utf-8"
            )
            entry = {
                "schema": "jianying-cli-release-entry/v1",
                "version": "1.6.5",
                "platform": platform,
                "archiveName": archive.name,
                "archiveSha256": digest,
                "binarySha256": "1" * 64,
                "capabilityManifestSha256": "2" * 64,
                "sbomSha256": "3" * 64,
                "capabilitySchema": "jianying-capabilities/v1",
                "contentState": "released",
                "releaseRef": "v1.6.5",
                "sourceCommit": "a" * 40,
            }
            (directory / f"{platform}.release-entry.json").write_text(
                json.dumps(entry), encoding="utf-8"
            )

    def test_complete_platform_set_builds_plugin_compatible_index(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            index = INDEX.build_index(root, "full-aigc-plugins/jianying-cli", "v1.6.5")
            self.assertEqual(index["schema"], "jianying-cli-release-index/v1")
            self.assertEqual(index["releaseRef"], "v1.6.5")
            self.assertEqual(index["repository"], "full-aigc-plugins/jianying-cli")
            self.assertEqual(set(index["artifacts"]), INDEX.PLATFORMS)
            self.assertEqual(index["sourceCommit"], "a" * 40)
            self.assertTrue(index["artifacts"]["win32-x64"]["url"].startswith("https://"))

    def test_index_rejects_noncanonical_repository_or_archive_name(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            with self.assertRaisesRegex(ValueError, "canonical repository"):
                INDEX.build_index(root, "mirror/jianying-cli", "v1.6.5")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            entry_path = next(root.rglob("darwin-arm64/*.release-entry.json"))
            entry = json.loads(entry_path.read_text())
            entry["archiveName"] = "renamed-darwin-arm64.tar.gz"
            entry_path.write_text(json.dumps(entry))
            with self.assertRaisesRegex(ValueError, "exact archive name"):
                INDEX.build_index(
                    root, "full-aigc-plugins/jianying-cli", "v1.6.5"
                )

    def test_missing_platform_and_tampered_archive_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64",))
            with self.assertRaisesRegex(ValueError, "exactly one entry"):
                INDEX.build_index(root, "full-aigc-plugins/jianying-cli", "v1.6.5")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            next(root.rglob("*.zip")).write_bytes(b"tampered")
            with self.assertRaisesRegex(ValueError, "checksum differs"):
                INDEX.build_index(root, "full-aigc-plugins/jianying-cli", "v1.6.5")

    def test_missing_or_tampered_archive_checksum_sidecar_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            sidecar = next(root.rglob("*.sha256"))
            sidecar.unlink()
            with self.assertRaisesRegex(ValueError, "checksum sidecar"):
                INDEX.build_index(
                    root, "full-aigc-plugins/jianying-cli", "v1.6.5"
                )

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            sidecar = next(root.rglob("*.sha256"))
            sidecar.write_text(f"{'0' * 64}  wrong-name.zip\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "checksum sidecar"):
                INDEX.build_index(
                    root, "full-aigc-plugins/jianying-cli", "v1.6.5"
                )

    def test_unreleased_platform_entry_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.fixture(root, ("darwin-arm64", "darwin-x64", "win32-x64"))
            entry_path = next(root.rglob("*.release-entry.json"))
            entry = json.loads(entry_path.read_text())
            entry["contentState"] = "working_tree_unreleased"
            entry["releaseRef"] = None
            entry_path.write_text(json.dumps(entry))
            with self.assertRaisesRegex(ValueError, "not bound to the release tag"):
                INDEX.build_index(root, "full-aigc-plugins/jianying-cli", "v1.6.5")

    def test_packager_rejects_stale_binary_contract_and_split_identity(self) -> None:
        embedded = json.loads(
            (ROOT / "provenance" / "CAPABILITIES.json").read_text(encoding="utf-8")
        )
        embedded["cli_version"] = PACKAGE.version()
        PACKAGE.verify_embedded_contract(embedded)

        stale = dict(embedded)
        stale["baseline_commit"] = "0" * 40
        with self.assertRaisesRegex(SystemExit, "does not match provenance"):
            PACKAGE.verify_embedded_contract(stale)

        identity = {
            "contract_state": "released",
            "release_ref": "v1.6.5",
            "source_commit": "a" * 40,
        }
        split = dict(embedded)
        split.update({
            "contract_state": "working_tree_unreleased",
            "release_ref": None,
            "source_commit": None,
        })
        with self.assertRaisesRegex(SystemExit, "identity does not match"):
            PACKAGE.verify_embedded_identity(split, identity)

        split.update(identity)
        PACKAGE.verify_embedded_identity(split, identity)

    def test_packager_rejects_placeholder_or_unlicensed_sbom(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sbom = Path(temporary) / "SBOM.spdx.json"
            sbom.write_text("{}", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "SPDX-2.3"):
                PACKAGE.verify_sbom(sbom, "1.6.5")

            sbom.write_text(json.dumps({
                "spdxVersion": "SPDX-2.3",
                "dataLicense": "CC0-1.0",
                "packages": [{
                    "name": "jianying-cli",
                    "versionInfo": "1.6.5",
                    "licenseDeclared": "NOASSERTION",
                }],
            }), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "lack declared licenses"):
                PACKAGE.verify_sbom(sbom, "1.6.5")

            document = json.loads(sbom.read_text(encoding="utf-8"))
            document["packages"][0]["licenseDeclared"] = "Apache-2.0"
            document["packages"][0]["SPDXID"] = "SPDXRef-Package-jianying-cli-1.6.5"
            sbom.write_text(json.dumps(document), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "Cargo.lock package coverage differs"):
                PACKAGE.verify_sbom(sbom, "1.6.5")

    def test_packager_requires_complete_locked_sbom_identity_and_checksums(self) -> None:
        locked = tomllib.loads((ROOT / "Cargo.lock").read_text(encoding="utf-8"))["package"]
        current_version = tomllib.loads(
            (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        )["package"]["version"]
        packages = []
        for item in locked:
            package = {
                "SPDXID": f"SPDXRef-{item['name']}-{item['version']}",
                "name": item["name"],
                "versionInfo": item["version"],
                "downloadLocation": item.get("source") or "NOASSERTION",
                "licenseDeclared": "Apache-2.0",
            }
            if checksum := item.get("checksum"):
                package["checksums"] = [{
                    "algorithm": "SHA256",
                    "checksumValue": checksum,
                }]
            packages.append(package)
        document = {
            "spdxVersion": "SPDX-2.3",
            "dataLicense": "CC0-1.0",
            "packages": packages,
        }

        with tempfile.TemporaryDirectory() as temporary:
            sbom = Path(temporary) / "SBOM.spdx.json"
            sbom.write_text(json.dumps(document), encoding="utf-8")
            PACKAGE.verify_sbom(sbom, current_version)

            truncated = dict(document)
            truncated["packages"] = packages[:-1]
            sbom.write_text(json.dumps(truncated), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "Cargo.lock package coverage differs"):
                PACKAGE.verify_sbom(sbom, current_version)

            tampered = json.loads(json.dumps(document))
            checksummed = next(
                package for package in tampered["packages"] if package.get("checksums")
            )
            checksummed["checksums"][0]["checksumValue"] = "0" * 64
            sbom.write_text(json.dumps(tampered), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "checksum differs from Cargo.lock"):
                PACKAGE.verify_sbom(sbom, current_version)

            wrong_source = json.loads(json.dumps(document))
            sourced = next(
                package for package in wrong_source["packages"]
                if package["downloadLocation"] != "NOASSERTION"
            )
            sourced["downloadLocation"] = "https://example.invalid/source"
            sbom.write_text(json.dumps(wrong_source), encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "source differs from Cargo.lock"):
                PACKAGE.verify_sbom(sbom, current_version)

    def test_generated_sbom_passes_full_cargo_lock_verification(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            sbom = Path(temporary) / "SBOM.spdx.json"
            second = Path(temporary) / "SBOM-second.spdx.json"
            environment = os.environ.copy()
            environment["SOURCE_DATE_EPOCH"] = "1700000000"
            subprocess.run(
                [sys.executable, str(ROOT / "tools/generate_sbom.py"), "--output", str(sbom)],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            subprocess.run(
                [sys.executable, str(ROOT / "tools/generate_sbom.py"), "--output", str(second)],
                cwd=ROOT,
                check=True,
                capture_output=True,
                text=True,
                env=environment,
            )
            PACKAGE.verify_sbom(sbom, PACKAGE.version())
            document = json.loads(sbom.read_text(encoding="utf-8"))
            self.assertEqual(document["creationInfo"]["created"], "2023-11-14T22:13:20Z")
            self.assertEqual(sbom.read_bytes(), second.read_bytes())
            locked = tomllib.loads(
                (ROOT / "Cargo.lock").read_text(encoding="utf-8")
            )["package"]
            self.assertEqual(len(document["packages"]), len(locked))
            checksummed = sum(1 for package in locked if package.get("checksum"))
            observed_checksums = sum(
                1 for package in document["packages"] if package.get("checksums")
            )
            self.assertEqual(observed_checksums, checksummed)

    def test_platform_archives_are_reproducible_across_file_mtime_changes(self) -> None:
        for platform in ("darwin-arm64", "win32-x64"):
            with self.subTest(platform=platform), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                bundle = root / f"jianying-cli-1.6.5-{platform}"
                (bundle / "nested").mkdir(parents=True)
                binary = bundle / ("jianying.exe" if platform == "win32-x64" else "jianying")
                binary.write_bytes(b"binary fixture")
                binary.chmod(0o755)
                (bundle / "nested/data.json").write_text('{"ok":true}\n', encoding="utf-8")

                first_dir = root / "first"
                second_dir = root / "second"
                first_dir.mkdir()
                second_dir.mkdir()
                first = PACKAGE.archive_bundle(
                    bundle, first_dir, platform, epoch=1_700_000_000
                )
                os.utime(binary, (1_800_000_000, 1_800_000_000))
                os.utime(bundle / "nested/data.json", (1_900_000_000, 1_900_000_000))
                second = PACKAGE.archive_bundle(
                    bundle, second_dir, platform, epoch=1_700_000_000
                )
                self.assertEqual(first.read_bytes(), second.read_bytes())


if __name__ == "__main__":
    unittest.main()

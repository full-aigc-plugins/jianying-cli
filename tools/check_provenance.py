#!/usr/bin/env python3
"""Validate fixed-source provenance and parity baselines without network access."""

from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path


EXPECTED_CAPCUT_COMMANDS = [
    "info", "version", "lint", "tracks", "segments", "texts", "set-text", "shift",
    "shift-all", "speed", "volume", "trim", "opacity", "export-srt", "export-ass",
    "export-timeline", "import-timeline", "materials", "segment", "material", "add-audio",
    "add-video", "add-text", "tts", "crop", "cut", "duplicate", "remove", "keyframe",
    "transition", "mask", "bg-blur", "text-style", "text-anim", "image-anim", "add-sticker",
    "mix-mode", "audio-fade", "add-cover", "add-filter", "bubble-text", "add-effect",
    "save-template", "apply-template", "make-preset", "templates", "batch", "import-srt",
    "import-ass", "text-ranges", "caption", "translate", "migrate", "add-sfx", "chroma",
    "matting", "prune", "register", "rename", "relink", "replace-media", "timeline",
    "projects", "diff", "concat", "config", "describe", "completions", "enums", "catalogue",
    "harvest-enums", "doctor", "diagnose", "fixture", "sync-timelines", "restore", "serve",
    "decrypt", "export", "init", "quickstart", "compile", "render", "detect-scenes",
    "detect-silence", "detect-retakes",
]
ALLOWED_STATUSES = {"supported", "partial", "external_dependency", "not_applicable"}
REQUIRED_SOURCES = {"pyjianyingdraft", "capcut-cli", "jianying-headless-capability-target"}
REQUIRED_PROTOCOL_OBJECTS = {
    "timeline", "track", "segment", "video", "audio", "text", "sticker", "filter",
    "effect", "transition", "mask", "animation", "keyframe", "subtitle", "text_style",
    "speed", "volume", "crop", "transform", "compositing",
}
COMPARISON_MODES = {"exact", "normalized_equivalent", "approved_difference"}
PROJECT_DIFFERENTIAL_COMMANDS = {
    "info", "version", "diff", "describe", "migrate", "concat", "cut", "init", "quickstart", "prune", "add-cover"
}
TIMELINE_DIFFERENTIAL_COMMANDS = {
    "tracks", "segments", "segment", "shift", "shift-all", "speed", "volume", "trim",
    "timeline", "add-track", "add-segment", "set", "split", "duplicate", "remove", "matting", "chroma", "mask", "bg-blur", "audio-fade", "opacity", "composite",
    "add-filter",
    "bubble-text",
    "add-effect",
    "crop",
    "keyframe",
    "transition",
    "text-anim",
    "image-anim",
    "add-sticker",
    "text-ranges",
}
MEDIA_ANALYSIS_DIFFERENTIAL_COMMANDS = {"detect-scenes", "detect-silence", "detect-retakes"}
MEDIA_MUTATION_DIFFERENTIAL_COMMANDS = {
    "add-video", "add-audio", "replace-media", "relink", "add-sfx", "tts"
}
MATERIAL_DISCOVERY_DIFFERENTIAL_COMMANDS = {"materials", "material"}
INTERCHANGE_DIFFERENTIAL_COMMANDS = {"export-timeline", "import-timeline"}
NATIVE_RUNTIME_CASES = {
    "runtime-profile-probe",
    "isolated-draft-copy",
    "owned-process-control",
    "asr-provider-ledger",
    "native-export-task",
}
EVIDENCE_LEVELS = [
    "unit", "differential", "structural", "app-open", "cold-reopen",
    "playback", "native-export",
]


class GateError(RuntimeError):
    """Raised when a provenance or parity invariant is violated."""


def load_json(path: Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"cannot read {path}: {error}") from error


def tree_digest(root: Path, pattern: str) -> tuple[int, str]:
    files = sorted(path for path in root.glob(pattern) if path.is_file())
    digest = hashlib.sha256()
    for path in files:
        relative = path.relative_to(root).as_posix()
        digest.update(relative.encode())
        digest.update(b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).hexdigest().encode())
        digest.update(b"\n")
    return len(files), digest.hexdigest()


def validate_fixture_origins(root: Path, manifest: dict, sources: dict) -> None:
    actual = {
        path.relative_to(root).as_posix()
        for path in root.glob("tests/parity/scenarios/*.json")
        if path.is_file()
    }
    declared: set[str] = set()
    for fixture_set in manifest.get("fixture_sets", []):
        origin = fixture_set.get("origin")
        reference = fixture_set.get("reference_source")
        if not origin or reference not in sources:
            raise GateError(f"fixture set has unknown origin/reference: {fixture_set.get('id')}")
        if reference == "jianying-headless-capability-target":
            raise GateError("restricted headless source cannot provide fixtures")
        for path in fixture_set.get("files", []):
            if path in declared:
                raise GateError(f"fixture declared more than once: {path}")
            declared.add(path)
    missing = actual - declared
    stale = declared - actual
    if missing or stale:
        raise GateError(f"fixture provenance mismatch; missing={sorted(missing)}, stale={sorted(stale)}")


def validate(root: Path, source_manifest: dict, parity: dict, v1: dict) -> None:
    if source_manifest.get("schema") != "jianying-source-manifest/v1":
        raise GateError("unsupported SOURCE_MANIFEST schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if set(sources) != REQUIRED_SOURCES:
        raise GateError(f"source ids must be exactly {sorted(REQUIRED_SOURCES)}")
    for source_id, source in sources.items():
        if len(source.get("commit", "")) != 40 or len(source.get("tree", "")) != 40:
            raise GateError(f"{source_id} is not pinned to full commit and tree hashes")
        if not source.get("declared_license") or not source.get("license_sha256"):
            raise GateError(f"{source_id} lacks fixed license evidence")
    restricted = sources["jianying-headless-capability-target"]
    if restricted.get("decision") != "blocked_as_implementation_source":
        raise GateError("non-commercial headless source must remain blocked as an implementation source")

    for artifact in source_manifest.get("artifact_sets", []):
        count, digest = tree_digest(root, artifact["pattern"])
        if count != artifact["file_count"] or digest != artifact["tree_sha256"]:
            raise GateError(
                f"artifact set {artifact['id']} drifted: count={count}, sha256={digest}"
            )
        if artifact.get("source") not in sources:
            raise GateError(f"artifact set {artifact['id']} has unknown source")

    validate_fixture_origins(root, source_manifest, sources)

    if parity.get("schema") != "jianying-parity-matrix/v1":
        raise GateError("unsupported parity matrix schema")
    commands = parity.get("commands", [])
    names = [entry.get("upstream_command") for entry in commands]
    if names != EXPECTED_CAPCUT_COMMANDS:
        raise GateError("capcut-cli command matrix is missing, duplicated, or out of fixed baseline order")
    for entry in commands:
        if entry.get("status") not in ALLOWED_STATUSES:
            raise GateError(f"invalid status for {entry.get('upstream_command')}")
        if not entry.get("rust_target") or not entry.get("test") or not entry.get("evidence"):
            raise GateError(f"incomplete command mapping for {entry.get('upstream_command')}")
    for section in ("functions", "data_structures"):
        rows = parity.get(section, [])
        if not rows:
            raise GateError(f"parity section {section} is empty")
        for row in rows:
            if row.get("source") not in sources or not row.get("rust_target"):
                raise GateError(f"invalid {section} row {row.get('id')}")

    if v1.get("schema") != "jianying-v1-baseline/v1":
        raise GateError("unsupported v1 baseline schema")
    if v1.get("plan_schema") != "jianying-cli-plan/v1":
        raise GateError("v1 baseline changed plan schema")
    count, digest = tree_digest(root, "tests/parity/scenarios/*.json")
    if count != v1.get("parity_fixture_count") or digest != v1.get("parity_fixture_tree_sha256"):
        raise GateError("v1 parity fixture baseline drifted")
    if not (root / "provenance" / "MIGRATION_SCOPE.md").is_file():
        raise GateError("migration scope decision record is missing")


def validate_protocol_differentials(root: Path, protocol: dict, run: dict) -> None:
    if protocol.get("schema") != "jianying-protocol-differentials/v1":
        raise GateError("unsupported protocol differential schema")
    if run.get("schema") != "jianying-python-rust-parity-run/v1":
        raise GateError("unsupported parity run schema")
    if protocol.get("python_source_commit") != run.get("python_source", {}).get("commit"):
        raise GateError("protocol differential source commit differs from parity run")
    summary = run.get("summary", {})
    if summary != {"passed": 55, "failed": 0, "total": 55}:
        raise GateError(f"parity run is not a clean 55-scenario pass: {summary}")
    scenario_files = {
        path.name for path in root.glob("tests/parity/scenarios/*.json") if path.is_file()
    }
    run_rows = run.get("scenarios", [])
    run_by_scenario = {row.get("scenario"): row for row in run_rows}
    if set(run_by_scenario) != scenario_files or len(run_rows) != len(run_by_scenario):
        raise GateError("parity run does not cover every scenario exactly once")
    for scenario, row in run_by_scenario.items():
        if row.get("status") != "passed" or row.get("issues") != []:
            raise GateError(f"parity scenario is not clean: {scenario}")
        if row.get("comparison") not in COMPARISON_MODES:
            raise GateError(f"invalid scenario comparison mode: {scenario}")

    objects = protocol.get("objects", [])
    by_name = {row.get("object"): row for row in objects}
    if set(by_name) != REQUIRED_PROTOCOL_OBJECTS or len(objects) != len(by_name):
        raise GateError("protocol differential object coverage is incomplete or duplicated")
    for name, row in by_name.items():
        mode = row.get("mode")
        if mode not in COMPARISON_MODES:
            raise GateError(f"invalid protocol comparison mode: {name}")
        evidence = row.get("scenarios", [])
        if not evidence:
            raise GateError(f"protocol object lacks differential scenarios: {name}")
        for scenario in evidence:
            run_row = run_by_scenario.get(scenario)
            if run_row is None or run_row.get("comparison") != mode:
                raise GateError(f"protocol object {name} has incompatible evidence {scenario}")
        if mode == "approved_difference" and not row.get("rationale"):
            raise GateError(f"approved protocol difference lacks rationale: {name}")


def validate_command_evidence(root: Path, source_manifest: dict, parity: dict, evidence: dict) -> None:
    if evidence.get("schema") != "jianying-capcut-command-evidence/v1":
        raise GateError("unsupported capcut command evidence schema")
    capcut_commit = next(
        source["commit"]
        for source in source_manifest["sources"]
        if source["id"] == "capcut-cli"
    )
    if evidence.get("baseline_commit") != capcut_commit:
        raise GateError("capcut command evidence baseline commit drifted")
    matrix_rows = parity.get("commands", [])
    evidence_rows = evidence.get("commands", [])
    if [row.get("command") for row in evidence_rows] != [
        row.get("upstream_command") for row in matrix_rows
    ]:
        raise GateError("capcut command evidence must cover all 86 commands in baseline order")
    incomplete = []
    for matrix_row, evidence_row in zip(matrix_rows, evidence_rows):
        command = matrix_row["upstream_command"]
        status = matrix_row["status"]
        if evidence_row.get("status") != status:
            raise GateError(f"command evidence status drifted for {command}")
        expected_claim = {
            "supported": "verified",
            "not_applicable": "verified",
            "partial": "gap-recorded",
            "external_dependency": "external-gate",
        }[status]
        if evidence_row.get("claim") != expected_claim:
            raise GateError(f"command evidence claim is invalid for {command}")
        for field in ("test_ref", "evidence_ref"):
            reference = evidence_row.get(field)
            if not isinstance(reference, str) or not reference:
                raise GateError(f"command evidence {field} missing for {command}")
            path = Path(reference)
            if path.is_absolute() or ".." in path.parts or not (root / path).is_file():
                raise GateError(f"command evidence {field} is not a repository file for {command}")
        if status in {"partial", "external_dependency"}:
            incomplete.append(command)
        elif evidence_row.get("test_ref") == "tests/provenance_guard.rs":
            raise GateError(f"supported command {command} cannot use only the gap guard as its test")
    overall_claim = evidence.get("overall_claim")
    if overall_claim not in {"complete", "incomplete"}:
        raise GateError("capcut command evidence overall_claim must be complete or incomplete")
    if overall_claim == "complete" and incomplete:
        raise GateError(
            "capcut parity cannot be claimed complete while partial/external commands remain: "
            + ", ".join(incomplete)
        )


def validate_migration_rollback_evidence(root: Path, evidence: dict) -> None:
    if evidence.get("schema") != "jianying-migration-rollback-evidence/v1":
        raise GateError("unsupported migration rollback evidence schema")
    if evidence.get("status") != "passed":
        raise GateError("migration rollback rehearsal is not passed")
    parser = evidence.get("v1_parser", {})
    if parser.get("status") != "retained" or parser.get("fixture_count") != 55:
        raise GateError("v1 parser retention evidence is incomplete")
    rollback = evidence.get("draft_rollback", {})
    if rollback.get("status") != "passed" or len(rollback.get("observed_steps", [])) < 5:
        raise GateError("draft rollback rehearsal evidence is incomplete")
    references = list(parser.get("implementation_refs", [])) + [
        parser.get("test_ref"),
        rollback.get("test_ref"),
    ]
    for reference in references:
        if not isinstance(reference, str) or not reference or not (root / reference).is_file():
            raise GateError(f"migration rollback evidence reference is missing: {reference}")


def validate_capcut_project_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-project-differential/v1":
        raise GateError("unsupported capcut project differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    expected_commit = sources["capcut-cli"]["commit"]
    if report.get("upstream", {}).get("commit") != expected_commit:
        raise GateError("capcut project differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != PROJECT_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut project differential command coverage is incomplete or duplicated")
    if report.get("passed") != 11 or report.get("failed") != 0:
        raise GateError("capcut project differential is not a clean 11-case pass")
    for command, case in by_command.items():
        status = case.get("status")
        if status not in {"exact", "exact-timeline", "semantic", "approved-difference", "semantic-command-map"}:
            raise GateError(f"invalid capcut project differential status: {command}")
        if status == "approved-difference" and not case.get("reason"):
            raise GateError(f"approved capcut project difference lacks rationale: {command}")


def validate_capcut_timeline_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-timeline-differential/v1":
        raise GateError("unsupported capcut timeline differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut timeline differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != TIMELINE_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut timeline differential command coverage is incomplete or duplicated")
    if report.get("passed") != 32 or report.get("failed") != 0:
        raise GateError("capcut timeline differential is not a clean 32-case pass")
    for command, case in by_command.items():
        if case.get("status") not in {"exact", "semantic", "rust-extension"}:
            raise GateError(f"invalid capcut timeline differential status: {command}")
        if case.get("status") != "exact" and not case.get("evidence"):
            raise GateError(f"non-exact timeline differential lacks evidence: {command}")


def validate_capcut_media_analysis_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-media-analysis-differential/v1":
        raise GateError("unsupported capcut media analysis differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut media analysis differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != MEDIA_ANALYSIS_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut media analysis differential coverage is incomplete or duplicated")
    if report.get("passed") != 3 or report.get("failed") != 0:
        raise GateError("capcut media analysis differential is not a clean 3-case pass")
    if any(case.get("status") != "exact" for case in cases):
        raise GateError("capcut media analysis differential must be exact")


def validate_capcut_utility_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-utility-differential/v1":
        raise GateError("unsupported capcut utility differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut utility differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    completion_cases = [case for case in cases if case.get("command") == "completions"]
    actual = {(case.get("command"), case.get("shell")) for case in completion_cases}
    expected = {
        ("completions", "bash"),
        ("completions", "zsh"),
        ("completions", "fish"),
    }
    if actual != expected or len(completion_cases) != len(actual):
        raise GateError("capcut utility differential shell coverage is incomplete")
    enum_cases = [case for case in cases if case.get("command") == "enums"]
    categories = {
        "transitions", "masks", "image_intros", "image_outros", "image_combos",
        "text_intros", "text_outros", "text_loop_anims", "scene_effects",
        "character_effects", "audio_effects", "fonts", "filters", "bubbles",
    }
    expected_enums = {
        (namespace, category)
        for namespace in {"capcut", "jianying"}
        for category in categories
    }
    actual_enums = {
        (case.get("namespace"), case.get("category")) for case in enum_cases
    }
    if actual_enums != expected_enums or len(enum_cases) != len(actual_enums):
        raise GateError("capcut utility differential enum coverage is incomplete")
    allowed_enum_statuses = {"exact", "approved-upstream-stdout-truncation"}
    if any(
        case.get("status") not in allowed_enum_statuses or not case.get("evidence")
        for case in enum_cases
    ):
        raise GateError("capcut utility enum differential lacks valid evidence")
    approved = {
        (case.get("namespace"), case.get("category"))
        for case in enum_cases
        if case.get("status") == "approved-upstream-stdout-truncation"
    }
    if approved != {
        ("jianying", "transitions"),
        ("jianying", "scene_effects"),
        ("jianying", "filters"),
    }:
        raise GateError("capcut utility enum approved differences drifted")
    harvest_cases = [case for case in cases if case.get("command") == "harvest-enums"]
    if {(case.get("mode"), case.get("status")) for case in harvest_cases} != {
        ("scan", "semantic-command-map"),
        ("add", "semantic-command-map"),
        ("sync", "semantic-command-map"),
    } or any(not case.get("evidence") for case in harvest_cases):
        raise GateError("capcut utility harvest-enums coverage is incomplete")
    diagnose_cases = [case for case in cases if case.get("command") == "diagnose"]
    if {(case.get("mode"), case.get("status")) for case in diagnose_cases} != {
        ("canonical", "semantic-command-map"),
        ("divergence-bundle", "semantic-command-map"),
    } or any(not case.get("evidence") for case in diagnose_cases):
        raise GateError("capcut utility diagnose coverage is incomplete")
    fixture_cases = [case for case in cases if case.get("command") == "fixture"]
    if {(case.get("mode"), case.get("status")) for case in fixture_cases} != {
        ("sanitize-check", "semantic-command-map"),
        ("verify-only-failure", "semantic-command-map"),
    } or any(not case.get("evidence") for case in fixture_cases):
        raise GateError("capcut utility fixture coverage is incomplete")
    compile_cases = [case for case in cases if case.get("command") == "compile"]
    if {(case.get("mode"), case.get("status")) for case in compile_cases} != {
        ("preflight", "semantic-command-map"),
        ("nine-operations-build", "semantic-command-map"),
        ("jsonl-batch", "semantic-command-map"),
        ("batch-fail-fast", "semantic-command-map"),
    } or any(not case.get("evidence") for case in compile_cases):
        raise GateError("capcut utility compile coverage is incomplete")
    if len(cases) != 42 or report.get("passed") != 42 or report.get("failed") != 0:
        raise GateError("capcut utility differential is not a clean 42-case pass")
    if any(
        case.get("status") != "semantic-command-map" or not case.get("evidence")
        for case in completion_cases
    ):
        raise GateError("capcut utility differential lacks mapped semantic evidence")


def validate_capcut_media_mutation_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-media-mutation-differential/v1":
        raise GateError("unsupported capcut media mutation differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut media mutation differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != MEDIA_MUTATION_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut media mutation differential coverage is incomplete or duplicated")
    if report.get("passed") != 6 or report.get("failed") != 0:
        raise GateError("capcut media mutation differential is not a clean 6-case pass")
    for command, case in by_command.items():
        if case.get("status") not in {"semantic", "exact-fields"}:
            raise GateError(f"invalid capcut media mutation differential status: {command}")
        if case.get("status") == "semantic" and not case.get("reason"):
            raise GateError(f"semantic media mutation differential lacks rationale: {command}")


def validate_capcut_material_discovery_differentials(
    source_manifest: dict, report: dict
) -> None:
    if report.get("schema") != "jianying-capcut-material-discovery-differential/v1":
        raise GateError("unsupported capcut material discovery differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut material discovery differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != MATERIAL_DISCOVERY_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut material discovery differential coverage is incomplete or duplicated")
    if report.get("passed") != 2 or report.get("failed") != 0:
        raise GateError("capcut material discovery differential is not a clean 2-case pass")
    for command, case in by_command.items():
        if case.get("status") != "exact" or not case.get("variants"):
            raise GateError(f"invalid capcut material discovery differential: {command}")


def validate_capcut_interchange_differentials(source_manifest: dict, report: dict) -> None:
    if report.get("schema") != "jianying-capcut-interchange-differential/v1":
        raise GateError("unsupported capcut interchange differential schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    if report.get("upstream", {}).get("commit") != sources["capcut-cli"]["commit"]:
        raise GateError("capcut interchange differential is not pinned to SOURCE_MANIFEST")
    cases = report.get("cases", [])
    by_command = {case.get("command"): case for case in cases}
    if set(by_command) != INTERCHANGE_DIFFERENTIAL_COMMANDS or len(cases) != len(by_command):
        raise GateError("capcut interchange differential coverage is incomplete or duplicated")
    if report.get("passed") != 2 or report.get("failed") != 0:
        raise GateError("capcut interchange differential is not a clean 2-case pass")
    export_case = by_command["export-timeline"]
    if export_case.get("status") != "exact" or set(export_case.get("variants", [])) != {
        "stdout-document", "file-and-summary", "caption-markers"
    }:
        raise GateError("capcut export interchange differential variants are incomplete")
    import_case = by_command["import-timeline"]
    if import_case.get("status") != "normalized-equivalent" or not import_case.get("approved"):
        raise GateError("capcut import interchange differential lacks approved differences")


def validate_native_runtime_independence(
    root: Path, source_manifest: dict, evidence: dict
) -> None:
    if evidence.get("schema") != "jianying-native-runtime-independent-evidence/v1":
        raise GateError("unsupported native runtime evidence schema")
    sources = {entry.get("id"): entry for entry in source_manifest.get("sources", [])}
    restricted_source = sources["jianying-headless-capability-target"]
    restricted = evidence.get("restricted_source", {})
    if restricted.get("id") != "jianying-headless-capability-target":
        raise GateError("native runtime evidence does not identify the restricted source")
    if restricted.get("commit") != restricted_source.get("commit"):
        raise GateError("native runtime review is not pinned to the restricted source commit")
    if restricted.get("decision") != "blocked_as_implementation_source":
        raise GateError("native runtime evidence did not preserve the restricted-source block")
    if restricted.get("review_result") != (
        "no_source_tests_blueprints_resources_fixtures_or_constants_used"
    ):
        raise GateError("native runtime independence review is incomplete")
    policy = evidence.get("policy", {})
    if policy != {
        "material_origin": "project_generated_at_test_runtime",
        "network": "disabled",
        "real_editor": "not_used",
        "claim_limit": "synthetic_contract_evidence_only",
    }:
        raise GateError("native runtime synthetic evidence policy drifted")
    cases = evidence.get("cases", [])
    by_id = {case.get("id"): case for case in cases}
    if set(by_id) != NATIVE_RUNTIME_CASES or len(cases) != len(by_id):
        raise GateError("native runtime independent evidence coverage is incomplete")
    for case_id, case in by_id.items():
        if case.get("material_origin") != "project_generated_at_test_runtime":
            raise GateError(f"native runtime case has a non-project origin: {case_id}")
        if case.get("evidence_level") not in {"unit", "structural"}:
            raise GateError(f"native runtime case has an invalid evidence level: {case_id}")
        test_path = root / str(case.get("test_path", ""))
        if not test_path.is_file() or root not in test_path.resolve().parents:
            raise GateError(f"native runtime test evidence is missing or outside root: {case_id}")
        if not case.get("covers"):
            raise GateError(f"native runtime case has no declared coverage: {case_id}")
    if evidence.get("ci_command") != "cargo test --workspace --all-targets":
        raise GateError("native runtime evidence is not bound to the full workspace test command")
    forbidden_tokens = {
        "github.com/partme-ai/jianying-headless",
        restricted_source["commit"],
    }
    for relative_root in evidence.get("implementation_roots", []):
        implementation_root = (root / relative_root).resolve()
        if root not in implementation_root.parents or not implementation_root.is_dir():
            raise GateError(f"invalid native runtime implementation root: {relative_root}")
        for path in implementation_root.glob("*.rs"):
            text = path.read_text(encoding="utf-8")
            if any(token in text for token in forbidden_tokens):
                raise GateError(f"restricted source marker found in implementation: {path}")


def validate_evidence_model(root: Path, model: dict) -> None:
    if model.get("schema") != "jianying-evidence-model/v1":
        raise GateError("unsupported evidence model schema")
    if model.get("record_schema") != "schemas/evidence-record-v1.schema.json":
        raise GateError("evidence model does not reference the fixed record schema")
    levels = model.get("levels", [])
    if [level.get("id") for level in levels] != EVIDENCE_LEVELS:
        raise GateError("evidence levels are missing, duplicated, or out of order")
    if [level.get("rank") for level in levels] != list(range(1, 8)):
        raise GateError("evidence ranks must be contiguous from 1 through 7")
    for level in levels:
        if not level.get("proves") or not level.get("required_artifacts"):
            raise GateError(f"evidence level is incomplete: {level.get('id')}")
    rules = model.get("rules", {})
    if rules.get("no_implicit_promotion") is not True:
        raise GateError("evidence model permits implicit promotion")
    if rules.get("highest_claim_equals_observed_level") is not True:
        raise GateError("evidence claim may exceed observation")
    if rules.get("synthetic_process_max_level") != "structural":
        raise GateError("synthetic evidence exceeds structural level")
    if rules.get("proxy_render_max_level") != "structural":
        raise GateError("proxy render can be mislabeled as native evidence")
    if rules.get("native_export_requires_explicit_approval") is not True:
        raise GateError("native export evidence lacks approval gate")
    external = rules.get("external_levels_require_named_runtime")
    if external != EVIDENCE_LEVELS[3:]:
        raise GateError("external evidence levels do not require a named runtime")
    schema_path = root / model["record_schema"]
    record_schema = load_json(schema_path)
    enum = record_schema.get("properties", {}).get("level", {}).get("enum")
    if enum != EVIDENCE_LEVELS:
        raise GateError("evidence record schema level enum drifted from the model")


def negative_self_test(root: Path, source_manifest: dict, parity: dict, v1: dict) -> None:
    unknown = copy.deepcopy(source_manifest)
    unknown["fixture_sets"][0]["reference_source"] = "unknown-source"
    try:
        validate(root, unknown, parity, v1)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted an unknown fixture source")

    restricted = copy.deepcopy(source_manifest)
    restricted["fixture_sets"][0]["reference_source"] = "jianying-headless-capability-target"
    try:
        validate(root, restricted, parity, v1)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted a restricted fixture source")


def negative_protocol_self_test(root: Path, protocol: dict, run: dict) -> None:
    failed_run = copy.deepcopy(run)
    failed_run["summary"] = {"passed": 54, "failed": 1, "total": 55}
    failed_run["scenarios"][0]["status"] = "failed"
    failed_run["scenarios"][0]["issues"] = ["synthetic drift"]
    try:
        validate_protocol_differentials(root, protocol, failed_run)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted an incomplete parity run")

    missing_object = copy.deepcopy(protocol)
    missing_object["objects"] = missing_object["objects"][1:]
    try:
        validate_protocol_differentials(root, missing_object, run)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted an incomplete protocol matrix")


def negative_native_runtime_self_test(
    root: Path, source_manifest: dict, evidence: dict
) -> None:
    contaminated = copy.deepcopy(evidence)
    contaminated["cases"][0]["material_origin"] = "restricted_upstream_fixture"
    try:
        validate_native_runtime_independence(root, source_manifest, contaminated)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted restricted native runtime evidence")


def negative_evidence_model_self_test(root: Path, model: dict) -> None:
    promoted = copy.deepcopy(model)
    promoted["rules"]["proxy_render_max_level"] = "native-export"
    try:
        validate_evidence_model(root, promoted)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted proxy-to-native evidence promotion")


def negative_command_completion_self_test(
    root: Path, source_manifest: dict, parity: dict, evidence: dict
) -> None:
    promoted = copy.deepcopy(evidence)
    promoted["overall_claim"] = "complete"
    try:
        validate_command_evidence(root, source_manifest, parity, promoted)
    except GateError:
        pass
    else:
        raise GateError("negative self-test accepted an incomplete 86-command completion claim")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--self-test-negative-cases", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    source_manifest = load_json(root / "provenance" / "SOURCE_MANIFEST.json")
    parity = load_json(root / "provenance" / "PARITY_MATRIX.json")
    v1 = load_json(root / "provenance" / "V1_BASELINE.json")
    protocol = load_json(root / "provenance" / "PROTOCOL_DIFFERENTIALS.json")
    parity_run = load_json(root / "provenance" / "PARITY_RUN.json")
    capcut_project = load_json(root / "provenance" / "CAPCUT_PROJECT_DIFFERENTIALS.json")
    capcut_timeline = load_json(root / "provenance" / "CAPCUT_TIMELINE_DIFFERENTIALS.json")
    capcut_utility = load_json(root / "provenance" / "CAPCUT_UTILITY_DIFFERENTIALS.json")
    capcut_media_analysis = load_json(root / "provenance" / "CAPCUT_MEDIA_ANALYSIS_DIFFERENTIALS.json")
    capcut_media_mutation = load_json(root / "provenance" / "CAPCUT_MEDIA_MUTATION_DIFFERENTIALS.json")
    capcut_material_discovery = load_json(
        root / "provenance" / "CAPCUT_MATERIAL_DISCOVERY_DIFFERENTIALS.json"
    )
    capcut_interchange = load_json(root / "provenance" / "CAPCUT_INTERCHANGE_DIFFERENTIALS.json")
    native_runtime = load_json(root / "provenance" / "NATIVE_RUNTIME_INDEPENDENT_EVIDENCE.json")
    evidence_model = load_json(root / "provenance" / "EVIDENCE_MODEL.json")
    command_evidence = load_json(root / "provenance" / "CAPCUT_COMMAND_EVIDENCE.json")
    migration_rollback = load_json(root / "provenance" / "MIGRATION_ROLLBACK_EVIDENCE.json")
    validate(root, source_manifest, parity, v1)
    validate_protocol_differentials(root, protocol, parity_run)
    validate_capcut_project_differentials(source_manifest, capcut_project)
    validate_capcut_timeline_differentials(source_manifest, capcut_timeline)
    validate_capcut_utility_differentials(source_manifest, capcut_utility)
    validate_capcut_media_analysis_differentials(source_manifest, capcut_media_analysis)
    validate_capcut_media_mutation_differentials(source_manifest, capcut_media_mutation)
    validate_capcut_material_discovery_differentials(
        source_manifest, capcut_material_discovery
    )
    validate_capcut_interchange_differentials(source_manifest, capcut_interchange)
    validate_native_runtime_independence(root, source_manifest, native_runtime)
    validate_evidence_model(root, evidence_model)
    validate_command_evidence(root, source_manifest, parity, command_evidence)
    validate_migration_rollback_evidence(root, migration_rollback)
    if args.self_test_negative_cases:
        negative_self_test(root, source_manifest, parity, v1)
        negative_protocol_self_test(root, protocol, parity_run)
        negative_native_runtime_self_test(root, source_manifest, native_runtime)
        negative_evidence_model_self_test(root, evidence_model)
        negative_command_completion_self_test(root, source_manifest, parity, command_evidence)
    print(json.dumps({"ok": True, "sources": 3, "capcut_commands": 86, "fixtures": 55,
                      "capcut_project_differentials": 11, "capcut_timeline_differentials": 32,
                      "capcut_utility_differentials": 42,
                      "capcut_media_analysis_differentials": 3,
                      "capcut_media_mutation_differentials": 6,
                      "capcut_material_discovery_differentials": 2,
                      "capcut_interchange_differentials": 2,
                      "capcut_command_evidence": 86,
                      "migration_rollback": "passed",
                      "native_runtime_cases": 5}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

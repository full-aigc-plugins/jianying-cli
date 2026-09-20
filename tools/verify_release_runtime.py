#!/usr/bin/env python3
"""Verify a freshly built platform binary before packaging it for release."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib


REQUIRED_SUPPORTED_CAPABILITIES = {
    "capabilities",
    "schema.job_v2",
    "schema.compile_v1_compat",
    "job.run",
    "project.compile",
    "media.asr_whisper_cpp",
    "runtime.discovery",
}

CLOUD_TTS_PROVIDERS = {
    "xiaomi-mimo",
    "volcengine",
    "aliyun-bailian",
    "baidu",
    "tencent-cloud",
    "minimax",
    "zhipu-glm",
}

TTS_EXECUTION_OPTIONS = {
    "--approval-id",
    "--approval-root",
    "--tts-state-root",
}

ASR_EXECUTION_OPTIONS = {
    "--out",
    "--executable",
    "--model",
    "--model-id",
    "--format",
    "--language",
    "--translate",
    "--segment-timestamps",
    "--task-id",
    "--state-root",
    "--plan",
    "--retry",
}

RUNTIME_DISCOVERY_OPTIONS = {"--search-root", "--home", "--local-app-data"}


def run(binary: Path, *arguments: str, env: dict[str, str] | None = None) -> str:
    """Run one release-binary probe and return stdout or fail with diagnostics."""
    completed = subprocess.run(
        [str(binary), *arguments],
        check=False,
        capture_output=True,
        encoding="utf-8",
        env=env,
    )
    if completed.returncode != 0:
        raise SystemExit(
            f"release runtime probe failed ({' '.join(arguments)}): "
            f"exit={completed.returncode}\nstdout={completed.stdout}\nstderr={completed.stderr}"
        )
    return completed.stdout


def parse_envelope(raw: str, operation: str) -> dict:
    """Parse one stable JSON success envelope."""
    try:
        envelope = json.loads(raw)
    except json.JSONDecodeError as error:
        raise SystemExit(f"{operation} did not emit one JSON document: {error}") from error
    if envelope.get("ok") is not True or not isinstance(envelope.get("data"), dict):
        raise SystemExit(f"{operation} did not emit a success envelope")
    return envelope["data"]


def validate_capabilities(
    data: dict,
    expected_version: str,
    expected_state: str | None,
    expected_ref: str | None,
    expected_commit: str | None,
) -> None:
    """Validate release identity and mandatory supported capabilities."""
    if data.get("schema") != "jianying-capabilities/v1":
        raise SystemExit("release binary reports an incompatible capability schema")
    if data.get("cli_version") != expected_version:
        raise SystemExit("release binary version differs from Cargo.toml")
    for field, expected in (
        ("contract_state", expected_state),
        ("release_ref", expected_ref),
        ("source_commit", expected_commit),
    ):
        if expected is not None and data.get(field) != expected:
            raise SystemExit(f"release binary {field} differs from the build identity")

    capabilities = {
        item.get("id"): item
        for item in data.get("capabilities", [])
        if isinstance(item, dict)
    }
    statuses = {
        capability_id: item.get("status")
        for capability_id, item in capabilities.items()
    }
    missing = sorted(
        capability
        for capability in REQUIRED_SUPPORTED_CAPABILITIES
        if statuses.get(capability) != "supported"
    )
    if missing:
        raise SystemExit("release binary lacks supported capabilities: " + ", ".join(missing))
    if statuses.get("media.tts_provider") not in {"partial", "supported"}:
        raise SystemExit("release binary does not expose the TTS provider capability")

    windows_profile = capabilities.get("runtime.profile.windows")
    if (
        not isinstance(windows_profile, dict)
        or windows_profile.get("platform") != "windows"
        or windows_profile.get("status") != "external_dependency"
        or windows_profile.get("availability") != "unsupported"
    ):
        raise SystemExit(
            "Windows runtime profile must remain unsupported until a validated Windows canary exists"
        )


def validate_tts_help(help_text: str) -> None:
    """Validate the packaged command surface for cloud TTS execution."""
    missing_providers = sorted(provider for provider in CLOUD_TTS_PROVIDERS if provider not in help_text)
    missing_options = sorted(option for option in TTS_EXECUTION_OPTIONS if option not in help_text)
    if missing_providers or missing_options:
        details = []
        if missing_providers:
            details.append("providers=" + ",".join(missing_providers))
        if missing_options:
            details.append("options=" + ",".join(missing_options))
        raise SystemExit("release binary has an incomplete TTS command surface: " + "; ".join(details))


def validate_asr_help(help_text: str) -> None:
    """Validate the packaged command surface for local whisper.cpp execution."""
    missing_options = sorted(option for option in ASR_EXECUTION_OPTIONS if option not in help_text)
    if missing_options:
        raise SystemExit(
            "release binary has an incomplete ASR command surface: options="
            + ",".join(missing_options)
        )


def validate_runtime_discovery_help(help_text: str) -> None:
    """Validate the packaged read-only runtime discovery command surface."""
    missing_options = sorted(
        option for option in RUNTIME_DISCOVERY_OPTIONS if option not in help_text
    )
    if missing_options:
        raise SystemExit(
            "release binary has an incomplete runtime discovery surface: options="
            + ",".join(missing_options)
        )


def validate_cloud_plan(data: dict, raw: str, plaintext: str, draft: Path) -> None:
    """Validate that cloud planning is secret-free, offline, and non-mutating."""
    if data.get("state") != "approval_required" or data.get("provider") != "minimax":
        raise SystemExit("release binary did not create the expected cloud approval plan")
    if data.get("execution_mode") != "production":
        raise SystemExit("release binary cloud plan does not target the production adapter")
    if not str(data.get("endpoint", "")).startswith("https://"):
        raise SystemExit("release binary cloud plan does not use HTTPS")
    binding = data.get("approval_binding")
    if not isinstance(binding, dict) or binding.get("command") != "tts.cloud.submit":
        raise SystemExit("release binary cloud plan lacks an exact approval binding")
    if plaintext in raw:
        raise SystemExit("release binary leaked TTS plaintext in its approval plan")
    if any(draft.iterdir()):
        raise SystemExit("release binary mutated the draft while producing a cloud plan")


def cargo_version(repository: Path) -> str:
    """Read the canonical CLI package version from Cargo.toml."""
    with (repository / "Cargo.toml").open("rb") as source:
        document = tomllib.load(source)
    return document["package"]["version"]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", required=True, type=Path)
    arguments = parser.parse_args()

    binary = arguments.binary.resolve()
    if not binary.is_file():
        raise SystemExit(f"release binary not found: {binary}")
    repository = Path(__file__).resolve().parents[1]
    version = cargo_version(repository)

    version_output = run(binary, "--version").strip()
    if version_output != f"jianying {version}":
        raise SystemExit(f"unexpected release binary version output: {version_output}")

    capabilities = parse_envelope(run(binary, "capabilities", "--json"), "capabilities")
    expected_ref = os.environ.get("JIANYING_BUILD_RELEASE_REF") or None
    validate_capabilities(
        capabilities,
        version,
        os.environ.get("JIANYING_BUILD_CONTRACT_STATE"),
        expected_ref,
        os.environ.get("JIANYING_BUILD_SOURCE_COMMIT"),
    )
    validate_tts_help(run(binary, "media", "tts", "--help"))
    validate_asr_help(run(binary, "media", "transcribe", "--help"))
    validate_runtime_discovery_help(run(binary, "runtime", "discover", "--help"))

    plaintext = "release runtime smoke plaintext"
    with tempfile.TemporaryDirectory(prefix="jianying-release-runtime-") as temporary:
        draft = Path(temporary) / "draft"
        draft.mkdir()
        environment = os.environ.copy()
        environment.pop("JIANYING_RELEASE_SMOKE_UNUSED_SECRET", None)
        environment.pop("JIANYING_ALLOW_TTS_ENDPOINT_OVERRIDE", None)
        raw_plan = run(
            binary,
            "media",
            "tts",
            str(draft),
            "--provider",
            "minimax",
            "--text",
            plaintext,
            "--format",
            "mp3",
            "--model",
            "speech-2.8-hd",
            "--voice",
            "male-qn-qingse",
            "--credential-env",
            "JIANYING_RELEASE_SMOKE_UNUSED_SECRET",
            "--task-id",
            "release-runtime-smoke",
            "--max-cost-microunits",
            "50000",
            "--plan",
            "--json",
            env=environment,
        )
        validate_cloud_plan(parse_envelope(raw_plan, "media tts --plan"), raw_plan, plaintext, draft)

    print(
        f"verified jianying {version}: identity, required capabilities, "
        "cloud TTS, local ASR and runtime discovery command surfaces, and offline approval plan"
    )


if __name__ == "__main__":
    main()

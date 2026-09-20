import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location(
    "verify_release_runtime", ROOT / "tools" / "verify_release_runtime.py"
)
VERIFY = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(VERIFY)


class VerifyReleaseRuntimeTests(unittest.TestCase):
    def test_capabilities_require_identity_and_supported_contract(self):
        capabilities = [
            {"id": identifier, "status": "supported"}
            for identifier in VERIFY.REQUIRED_SUPPORTED_CAPABILITIES
        ]
        capabilities.append({"id": "media.tts_provider", "status": "partial"})
        capabilities.append(
            {
                "id": "runtime.profile.windows",
                "status": "external_dependency",
                "platform": "windows",
                "availability": "unsupported",
            }
        )
        VERIFY.validate_capabilities(
            {
                "schema": "jianying-capabilities/v1",
                "cli_version": "1.6.1",
                "contract_state": "released",
                "release_ref": "v1.6.1",
                "source_commit": "a" * 40,
                "capabilities": capabilities,
            },
            "1.6.1",
            "released",
            "v1.6.1",
            "a" * 40,
        )
        capabilities[0]["status"] = "partial"
        with self.assertRaisesRegex(SystemExit, "lacks supported capabilities"):
            VERIFY.validate_capabilities(
                {
                    "schema": "jianying-capabilities/v1",
                    "cli_version": "1.6.1",
                    "capabilities": capabilities,
                },
                "1.6.1",
                None,
                None,
                None,
            )

    def test_capabilities_fail_closed_for_unvalidated_windows_runtime_profile(self):
        base_capabilities = [
            {"id": identifier, "status": "supported"}
            for identifier in VERIFY.REQUIRED_SUPPORTED_CAPABILITIES
        ]
        base_capabilities.append({"id": "media.tts_provider", "status": "partial"})
        invalid_profiles = {
            "missing": None,
            "supported availability": {
                "id": "runtime.profile.windows",
                "status": "external_dependency",
                "platform": "windows",
                "availability": "supported",
            },
            "supported status": {
                "id": "runtime.profile.windows",
                "status": "supported",
                "platform": "windows",
                "availability": "unsupported",
            },
            "wrong platform": {
                "id": "runtime.profile.windows",
                "status": "external_dependency",
                "platform": "darwin",
                "availability": "unsupported",
            },
        }
        for name, profile in invalid_profiles.items():
            with self.subTest(name=name):
                capabilities = list(base_capabilities)
                if profile is not None:
                    capabilities.append(profile)
                with self.assertRaisesRegex(
                    SystemExit, "Windows runtime profile must remain unsupported"
                ):
                    VERIFY.validate_capabilities(
                        {
                            "schema": "jianying-capabilities/v1",
                            "cli_version": "1.6.1",
                            "capabilities": capabilities,
                        },
                        "1.6.1",
                        None,
                        None,
                        None,
                    )

    def test_tts_help_requires_all_cloud_providers_and_execution_options(self):
        complete = " ".join(VERIFY.CLOUD_TTS_PROVIDERS | VERIFY.TTS_EXECUTION_OPTIONS)
        VERIFY.validate_tts_help(complete)
        with self.assertRaisesRegex(SystemExit, "incomplete TTS command surface"):
            VERIFY.validate_tts_help(complete.replace("--approval-id", ""))

    def test_asr_help_requires_whisper_identity_plan_and_retry_options(self):
        complete = " ".join(VERIFY.ASR_EXECUTION_OPTIONS)
        VERIFY.validate_asr_help(complete)
        with self.assertRaisesRegex(SystemExit, "incomplete ASR command surface"):
            VERIFY.validate_asr_help(complete.replace("--model-id", ""))

    def test_runtime_discovery_help_requires_explicit_search_overrides(self):
        complete = " ".join(VERIFY.RUNTIME_DISCOVERY_OPTIONS)
        VERIFY.validate_runtime_discovery_help(complete)
        with self.assertRaisesRegex(SystemExit, "incomplete runtime discovery surface"):
            VERIFY.validate_runtime_discovery_help(complete.replace("--search-root", ""))

    def test_cloud_plan_is_secret_free_and_non_mutating(self):
        with tempfile.TemporaryDirectory() as temporary:
            draft = Path(temporary)
            data = {
                "state": "approval_required",
                "provider": "minimax",
                "execution_mode": "production",
                "endpoint": "https://api.minimax.cn/v1/t2a_v2",
                "approval_binding": {"command": "tts.cloud.submit"},
            }
            VERIFY.validate_cloud_plan(data, "hashed output", "private text", draft)
            with self.assertRaisesRegex(SystemExit, "leaked TTS plaintext"):
                VERIFY.validate_cloud_plan(data, "private text", "private text", draft)


if __name__ == "__main__":
    unittest.main()

import os
import pathlib
import shutil
import signal
import subprocess
import tempfile
import time
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
COLLECTOR = ROOT / "spikes/precision/infra-hyperstack/run-demographic-benchmark.sh"
HELPER = ROOT / "scripts/cuda-final-state-teardown.sh"


class TeardownFixture:
    def __init__(self, root: pathlib.Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.bin = root / "bin"
        self.bin.mkdir()
        self.key = root / "key"
        self.key.write_text("unused\n")
        self.artifact = root / "artifact"
        self.calls = root / "calls.log"
        self.terraform = self.bin / "terraform"
        self.reconcile = self.bin / "reconcile"
        self.terraform.write_text(
            "#!/usr/bin/env bash\n"
            "echo terraform \"$@\" >> \"$FOCUSED_CALLS\"\n"
            "if [[ \"$1\" == destroy ]]; then exit \"${STUB_DESTROY_STATUS:-0}\"; fi\n"
            "if [[ \"$1 $2\" == 'state list' ]]; then printf '%b' \"${STUB_STATE_LIST:-}\"; exit \"${STUB_STATE_STATUS:-0}\"; fi\n"
            "exit 0\n"
        )
        self.reconcile.write_text(
            "#!/usr/bin/env bash\n"
            "echo reconcile \"$@\" >> \"$FOCUSED_CALLS\"\n"
            "if [[ \" $* \" == *' --delete '* ]]; then\n"
            "  echo 'Verified: no sembla-precision* VMs remain in the account.'\n"
            "  exit \"${STUB_DELETE_STATUS:-0}\"\n"
            "fi\n"
            "printf 'name_prefix: sembla-precision\\n\\ntracked in Terraform state: 0\\nother VMs in the account (never touched): 0\\n\\nNo orphans. Every sembla-precision* VM in the account is tracked in state.\\n'\n"
            "exit \"${STUB_REPORT_STATUS:-0}\"\n"
        )
        self.terraform.chmod(0o755)
        self.reconcile.chmod(0o755)

    def environment(self, **overrides):
        environment = os.environ.copy()
        for name in list(environment):
            if name.startswith("BENCH_") or name.startswith("SEMBLA_SWEEP_EXPERIMENT_"):
                environment.pop(name, None)
        timeout = shutil.which("timeout")
        if not timeout:
            raise unittest.SkipTest("GNU timeout is required by the collector")
        environment.update(
            {
                "BENCH_CUDA_FINAL_STATE_DECISION": "1",
                "SEMBLA_FOCUSED_TEARDOWN_TEST_MODE": "1",
                "SSH_HOST_KEY_FINGERPRINT": "SHA256:AAAA",
                "SSH_PRIVATE_KEY_PATH": str(self.key),
                "ARTIFACT_DIR": str(self.artifact),
                "HYPERSTACK_API_KEY": "stub",
                "FOCUSED_TERRAFORM_BIN": str(self.terraform),
                "FOCUSED_RECONCILE_BIN": str(self.reconcile),
                "FOCUSED_TIMEOUT_BIN": timeout,
                "FOCUSED_TEARDOWN_TIMEOUT_SECONDS": "5",
                "FOCUSED_TEST_TIMEOUT_SECONDS": "5",
                "FOCUSED_CALLS": str(self.calls),
                "STUB_DESTROY_STATUS": "0",
                "STUB_DELETE_STATUS": "0",
                "STUB_REPORT_STATUS": "0",
                "STUB_STATE_STATUS": "0",
                "STUB_STATE_LIST": "",
                **overrides,
            }
        )
        return environment

    def run(self, **overrides):
        return subprocess.run(
            ["bash", str(COLLECTOR)],
            cwd=COLLECTOR.parent,
            env=self.environment(**overrides),
            text=True,
            capture_output=True,
            timeout=30,
        )


class FocusedTeardownTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.fixture = TeardownFixture(pathlib.Path(self.temporary.name))

    def tearDown(self):
        self.temporary.cleanup()

    def statuses(self):
        return (
            (self.fixture.artifact / "benchmark-status.txt").read_text().strip(),
            (self.fixture.artifact / "teardown-status.txt").read_text().strip(),
        )

    def test_success_records_separate_statuses_and_zero_resources(self):
        result = self.fixture.run(FOCUSED_TEST_BENCHMARK_COMMAND="true")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.statuses(), ("0", "0"))
        self.assertTrue((self.fixture.artifact / "zero-resource-result.txt").is_file())
        calls = self.fixture.calls.read_text().splitlines()
        self.assertIn("terraform destroy", calls[0])
        self.assertTrue(any(line == "reconcile" for line in calls))
        self.assertFalse(any("--delete" in line for line in calls))

    def test_benchmark_failure_wins_over_successful_teardown(self):
        result = self.fixture.run(FOCUSED_TEST_BENCHMARK_COMMAND="exit 7")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertEqual(self.statuses(), ("7", "0"))

    def test_timeout_preserves_124_and_still_tears_down(self):
        result = self.fixture.run(
            FOCUSED_TEST_TIMEOUT_SECONDS="0.1",
            FOCUSED_TEST_BENCHMARK_COMMAND="sleep 5",
        )
        self.assertEqual(result.returncode, 124, result.stderr)
        self.assertEqual(self.statuses(), ("124", "0"))
        self.assertIn("terraform destroy", self.fixture.calls.read_text())

    def _signal_case(self, signal_name, expected):
        result = self.fixture.run(FOCUSED_TEST_SIGNAL=signal_name)
        self.assertEqual(result.returncode, expected, result.stderr)
        self.assertEqual(self.statuses(), (str(expected), "0"))
        self.assertIn("terraform destroy", self.fixture.calls.read_text())

    def test_term_and_int_are_mapped_and_teardown_once(self):
        self._signal_case("TERM", 143)
        self.fixture = TeardownFixture(pathlib.Path(self.temporary.name) / "int")
        self._signal_case("INT", 130)

    def test_destroy_delete_and_report_failures_never_succeed(self):
        scenarios = [
            {"STUB_DESTROY_STATUS": "3"},
            {"STUB_DESTROY_STATUS": "3", "STUB_DELETE_STATUS": "4"},
            {"STUB_REPORT_STATUS": "5"},
            {"STUB_STATE_LIST": "hyperstack_core_virtual_machine.gpu[0]\\n"},
        ]
        for index, environment in enumerate(scenarios):
            with self.subTest(environment=environment):
                if index:
                    self.fixture = TeardownFixture(pathlib.Path(self.temporary.name) / str(index))
                result = self.fixture.run(**environment)
                self.assertNotEqual(result.returncode, 0)
                benchmark, teardown = self.statuses()
                self.assertEqual(benchmark, "0")
                self.assertNotEqual(teardown, "0")

    def test_nonzero_benchmark_status_has_precedence_over_teardown_failure(self):
        result = self.fixture.run(
            FOCUSED_TEST_BENCHMARK_COMMAND="exit 7",
            STUB_REPORT_STATUS="5",
        )
        self.assertEqual(result.returncode, 7)
        self.assertEqual(self.statuses()[0], "7")
        self.assertNotEqual(self.statuses()[1], "0")

    def test_helper_retries_interrupted_or_failed_attempts(self):
        artifact = self.fixture.artifact
        artifact.mkdir(parents=True)
        (artifact / ".cuda-final-state-teardown-started").write_text("interrupted\n")
        script = f"""
set -Eeuo pipefail
source {HELPER!s}
export FOCUSED_TERRAFORM_BIN={self.fixture.terraform!s}
export FOCUSED_RECONCILE_BIN={self.fixture.reconcile!s}
export FOCUSED_TIMEOUT_BIN={shutil.which('timeout')!s}
export FOCUSED_TEARDOWN_TIMEOUT_SECONDS=5
export FOCUSED_CALLS={self.fixture.calls!s}
export STUB_DESTROY_STATUS=0 STUB_DELETE_STATUS=0 STUB_REPORT_STATUS=5 STUB_STATE_STATUS=0 STUB_STATE_LIST=
if cuda_final_state_teardown {artifact!s} {COLLECTOR.parent!s} terraform.tfvars; then
  first=0
else
  first=$?
fi
[[ "$first" != 0 ]]
export STUB_REPORT_STATUS=0
cuda_final_state_teardown {artifact!s} {COLLECTOR.parent!s} terraform.tfvars
"""
        result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((artifact / "teardown-attempt-count.txt").read_text().strip(), "2")
        calls = self.fixture.calls.read_text().splitlines()
        self.assertEqual(sum(line.startswith("terraform destroy") for line in calls), 2)
        self.assertTrue((artifact / ".cuda-final-state-teardown-complete").is_file())

    def test_helper_is_idempotent(self):
        artifact = self.fixture.artifact
        script = f"""
set -Eeuo pipefail
source {HELPER!s}
export FOCUSED_TERRAFORM_BIN={self.fixture.terraform!s}
export FOCUSED_RECONCILE_BIN={self.fixture.reconcile!s}
export FOCUSED_TIMEOUT_BIN={shutil.which('timeout')!s}
export FOCUSED_TEARDOWN_TIMEOUT_SECONDS=5
export FOCUSED_CALLS={self.fixture.calls!s}
export STUB_DESTROY_STATUS=0 STUB_DELETE_STATUS=0 STUB_REPORT_STATUS=0 STUB_STATE_STATUS=0 STUB_STATE_LIST=
cuda_final_state_teardown {artifact!s} {COLLECTOR.parent!s} terraform.tfvars
cuda_final_state_teardown {artifact!s} {COLLECTOR.parent!s} terraform.tfvars
"""
        result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = self.fixture.calls.read_text().splitlines()
        self.assertEqual(sum(line.startswith("terraform destroy") for line in calls), 1)
        self.assertEqual(sum(line == "reconcile" for line in calls), 1)


if __name__ == "__main__":
    unittest.main()

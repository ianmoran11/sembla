import hashlib
import os
import subprocess
import tarfile
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "spikes" / "precision" / "infra-hyperstack" / "run-demographic-benchmark.sh"
INCOMPATIBLE = (
    "BENCH_PROFILE",
    "BENCH_CORPUS",
    "BENCH_SWEEP",
    "BENCH_CONCURRENCY_SPIKE",
    "BENCH_CONCURRENCY_SPIKE_ONLY",
    "BENCH_CONCURRENCY_SUPPORTED",
    "BENCH_CONCURRENCY_LOCKSTEP",
    "BENCH_CONCURRENCY_FREE_STREAMS",
    "BENCH_CONCURRENCY_FUSED",
    "BENCH_CONCURRENCY_CRN",
)
FOCUSED_INCOMPATIBLE = (
    "BENCH_CUDA_READBACK_DIAGNOSTIC",
    "BENCH_PROFILE",
    "BENCH_CORPUS",
    "BENCH_SWEEP",
    "BENCH_SWEEP_NUMA",
    "BENCH_CONCURRENCY_SPIKE",
    "BENCH_CONCURRENCY_SPIKE_ONLY",
    "BENCH_CONCURRENCY_SUPPORTED",
    "BENCH_CONCURRENCY_LOCKSTEP",
    "BENCH_CONCURRENCY_FREE_STREAMS",
    "BENCH_CONCURRENCY_FUSED",
    "BENCH_CONCURRENCY_CRN",
)


class CollectorFlagValidationTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        root = Path(self.temp.name)
        self.key = root / "key"
        self.key.write_text("not-used-by-preflight\n")
        self.artifact = root / "artifact"

    def tearDown(self):
        self.temp.cleanup()

    def run_script(self, **overrides):
        environment = os.environ.copy()
        for name in (
            "BENCH_CUDA_READBACK_DIAGNOSTIC",
            "BENCH_CUDA_FINAL_STATE_DECISION",
            "BENCH_SWEEP_BASELINE_COMMIT",
            "KEEP_VM",
            "SEMBLA_FOCUSED_TEARDOWN_TEST_MODE",
            "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256",
            "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256_VERIFY",
            *INCOMPATIBLE,
            *FOCUSED_INCOMPATIBLE,
        ):
            environment.pop(name, None)
        environment.update(
            {
                "SSH_HOST_KEY_FINGERPRINT": "SHA256:AAAA",
                "SSH_PRIVATE_KEY_PATH": str(self.key),
                "ARTIFACT_DIR": str(self.artifact),
                **overrides,
            }
        )
        return subprocess.run(
            ["bash", str(SCRIPT)],
            cwd=SCRIPT.parent,
            env=environment,
            text=True,
            capture_output=True,
            timeout=30,
        )

    def test_rejects_malformed_focused_value(self):
        result = self.run_script(BENCH_CUDA_FINAL_STATE_DECISION="yes")
        self.assertEqual(result.returncode, 2)
        self.assertIn("must be 0 or 1", result.stderr)
        self.assertFalse(self.artifact.exists())

    def test_rejects_malformed_diagnostic_value(self):
        result = self.run_script(BENCH_CUDA_READBACK_DIAGNOSTIC="yes")
        self.assertEqual(result.returncode, 2)
        self.assertIn("must be 0 or 1", result.stderr)
        self.assertFalse(self.artifact.exists())

    def test_rejects_every_incompatible_stage(self):
        for incompatible in INCOMPATIBLE:
            with self.subTest(incompatible=incompatible):
                result = self.run_script(
                    BENCH_CUDA_READBACK_DIAGNOSTIC="1", **{incompatible: "1"}
                )
                self.assertEqual(result.returncode, 2)
                self.assertIn(f"mutually exclusive with {incompatible}", result.stderr)
                self.assertFalse(self.artifact.exists())

    def test_focused_stage_rejects_every_conflict_keep_vm_and_retired_selectors_before_artifacts(self):
        for incompatible in FOCUSED_INCOMPATIBLE:
            with self.subTest(incompatible=incompatible):
                result = self.run_script(
                    BENCH_CUDA_FINAL_STATE_DECISION="1", **{incompatible: "1"}
                )
                self.assertEqual(result.returncode, 2)
                self.assertIn("mutually exclusive", result.stderr)
                self.assertIn(incompatible, result.stderr)
                self.assertIn("BENCH_CUDA_FINAL_STATE_DECISION", result.stderr)
                self.assertFalse(self.artifact.exists())
        for environment, expected in [
            ({"KEEP_VM": "1"}, "rejects KEEP_VM=1"),
            ({"BENCH_SWEEP_BASELINE_COMMIT": "deadbeef"}, "BENCH_SWEEP_BASELINE_COMMIT"),
            ({"SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256": "1"}, "retired selector"),
            ({"SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256_VERIFY": "1"}, "retired selector"),
        ]:
            result = self.run_script(BENCH_CUDA_FINAL_STATE_DECISION="1", **environment)
            self.assertEqual(result.returncode, 2)
            self.assertIn(expected, result.stderr)
            self.assertFalse(self.artifact.exists())

    def test_focused_preflight_failures_and_outer_timeout_have_cleanup_paths(self):
        source = SCRIPT.read_text()
        self.assertIn("focused_paid_resources_in_state", source)
        self.assertIn('if ! state="$(cd "$MODULE_DIR"', source)
        self.assertIn('[[ -n "${HYPERSTACK_API_KEY:-}" ]]', source)
        self.assertIn("sembla-final-state-preflight-cleanup", source)
        self.assertIn("cuda_final_state_teardown", source)
        self.assertIn("kill -TERM -- \"-$pid\"", source)
        self.assertIn("demographic-bench-partial.tar.gz", source)
        runbook = (SCRIPT.parent / "RUNBOOK.md").read_text()
        self.assertIn("set -o pipefail", runbook)

    def test_focused_stage_is_baked_into_payload_and_cannot_reach_broad_suites(self):
        source = SCRIPT.read_text()
        self.assertIn("export BENCH_CUDA_FINAL_STATE_DECISION=%q", source)
        self.assertIn('BENCH_CUDA_FINAL_STATE_DECISION:-0}\" == \"1\"', source)
        self.assertIn("scripts/run-cuda-final-state-decision.py", source)
        self.assertIn("exactly 27 benchmark", source)
        self.assertIn('&& "${BENCH_CUDA_FINAL_STATE_DECISION:-0}" != "1"', source)
        self.assertNotIn("SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256=1", source)

    def test_rejects_sweep_baseline_commit(self):
        result = self.run_script(
            BENCH_CUDA_READBACK_DIAGNOSTIC="1",
            BENCH_SWEEP_BASELINE_COMMIT="deadbeef",
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("mutually exclusive with BENCH_SWEEP_BASELINE_COMMIT", result.stderr)
        self.assertFalse(self.artifact.exists())

    def test_pins_compatible_ncu_and_temporarily_opens_driver_counters(self):
        source = SCRIPT.read_text()
        self.assertIn("NCU_PACKAGE='nsight-compute-2025.2.1'", source)
        self.assertIn("NCU_DEBIAN_VERSION='2025.2.1.3-1'", source)
        self.assertIn("NCU_BIN='/opt/nvidia/nsight-compute/2025.2.1/ncu'", source)
        self.assertIn("dpkg-query -W -f='${Package} ${Version}\\n'", source)
        self.assertIn("reload_nvidia_counter_mode 0", source)
        self.assertIn("restore_nvidia_counter_restriction", source)
        self.assertIn(
            'sudo -n modprobe nvidia "NVreg_RestrictProfilingToAdminUsers=$admin_only"',
            source,
        )
        self.assertIn("RmProfilingAdminOnly: 0", source)
        self.assertIn("RmProfilingAdminOnly: 1", source)
        self.assertIn("--kill-after=5s 30s", source)
        self.assertIn("trap 'package_partial_on_error 143' TERM", source)
        self.assertIn("trap 'package_partial_on_error 130' INT", source)
        self.assertIn("trap 'diagnostic_exit_handler $?' EXIT", source)
        self.assertNotIn("setcap cap_sys_admin", source)
        self.assertEqual(
            source.count(
                'timeout --signal=TERM --kill-after=30s 240s "$NCU_BIN"'
            ),
            2,
        )
        self.assertEqual(
            source.count(
                'timeout --signal=TERM --kill-after=10s 60s "$NCU_BIN"'
            ),
            2,
        )
        self.assertNotIn('sudo -n "$NCU_BIN" --devices', source)
        self.assertNotIn("timeout 240s ncu ", source)

    def test_preserves_raw_profiler_reports_and_partial_failures(self):
        source = SCRIPT.read_text()
        self.assertNotIn('rm -f "$arm/trace.nsys-rep"', source)
        self.assertNotIn('-sol.ncu-rep" \\\n      "$DIAGNOSTIC_DIR', source)
        self.assertNotIn('-detail.ncu-rep" \\\n      "$DIAGNOSTIC_DIR', source)
        self.assertIn("package_partial_on_error", source)
        self.assertIn("DIAGNOSTIC_FAILURE_HANDLED=0", source)
        self.assertIn("trap 'package_partial_on_error $?' ERR", source)
        self.assertIn("diagnostic_fail", source)
        self.assertIn("SHA256SUMS.partial", source)
        self.assertIn("Checksummed partial diagnostic evidence", source)

    def test_explicit_failure_packages_checksummed_partial_without_work_state(self):
        source = SCRIPT.read_text()
        start = source.index("reload_nvidia_counter_mode() {")
        end = source.index(
            '\nif [[ "${BENCH_CUDA_READBACK_DIAGNOSTIC:-0}" == "1" ' + "\\" + "\n",
            start,
        )
        functions = source[start:end]
        home = Path(self.temp.name) / "home"
        output = home / "demographic-bench"
        diagnostic = output / "cuda-readback-diagnostic"
        diagnostic.mkdir(parents=True)
        (diagnostic / "raw.ncu-rep").write_bytes(b"raw-report")
        (output / "gpu-provenance.txt").write_text("gpu\n")
        work = output / "work"
        work.mkdir()
        (work / "large.state").write_bytes(b"excluded")
        script = f"""set -Eeuo pipefail
export HOME={home!s}
OUT_ROOT={output!s}
PARTIAL_ARCHIVE="$HOME/demographic-bench-partial.tar.gz"
{functions}
trap package_partial_on_error ERR
diagnostic_fail "forced explicit validation failure"
"""
        result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
        self.assertEqual(result.returncode, 9, result.stderr)
        archive = home / "demographic-bench-partial.tar.gz"
        self.assertTrue(archive.is_file())
        extracted = Path(self.temp.name) / "extracted"
        extracted.mkdir()
        with tarfile.open(archive, "r:gz") as bundle:
            bundle.extractall(extracted)
        root = extracted / "demographic-bench-partial"
        self.assertTrue((root / "cuda-readback-diagnostic" / "raw.ncu-rep").is_file())
        self.assertFalse((root / "work").exists())
        for line in (root / "SHA256SUMS.partial").read_text().splitlines():
            expected, relative = line.split(maxsplit=1)
            path = root / relative.removeprefix("./")
            self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), expected)

    def test_internal_reload_failure_propagates_and_keeps_active_flag_for_retry(self):
        source = SCRIPT.read_text()
        start = source.index("reload_nvidia_counter_mode() {")
        end = source.index("\npackage_partial_on_error() {", start)
        functions = source[start:end]
        script = f"""set -Eeuo pipefail
{functions}
timeout() {{ return 77; }}
NCU_COUNTER_MODE_ACTIVE=1
set +e
restore_nvidia_counter_restriction
rc=$?
set -e
printf 'rc=%s active=%s\\n' "$rc" "$NCU_COUNTER_MODE_ACTIVE"
[[ "$rc" == 77 && "$NCU_COUNTER_MODE_ACTIVE" == 1 ]]
"""
        result = subprocess.run(["bash", "-c", script], text=True, capture_output=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("rc=77 active=1", result.stdout)

    def test_documents_both_profiler_compatibility_preflights(self):
        source = SCRIPT.read_text()
        self.assertIn("cuTensorMapEncodeIm2colWide", source)
        self.assertIn("passwordless sudo is required", source)
        self.assertIn("file capabilities do not", source)
        self.assertIn("Version 2025.2.1.", source)


if __name__ == "__main__":
    unittest.main()

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
            "BENCH_SWEEP_BASELINE_COMMIT",
            *INCOMPATIBLE,
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

    def test_rejects_sweep_baseline_commit(self):
        result = self.run_script(
            BENCH_CUDA_READBACK_DIAGNOSTIC="1",
            BENCH_SWEEP_BASELINE_COMMIT="deadbeef",
        )
        self.assertEqual(result.returncode, 2)
        self.assertIn("mutually exclusive with BENCH_SWEEP_BASELINE_COMMIT", result.stderr)
        self.assertFalse(self.artifact.exists())

    def test_pins_compatible_ncu_and_limits_counter_privilege(self):
        source = SCRIPT.read_text()
        self.assertIn("NCU_PACKAGE='nsight-compute-2025.2.1'", source)
        self.assertIn("NCU_DEBIAN_VERSION='2025.2.1.3-1'", source)
        self.assertIn("NCU_BIN='/opt/nvidia/nsight-compute/2025.2.1/ncu'", source)
        self.assertIn("dpkg-query -W -f='${Package} ${Version}\\n'", source)
        self.assertIn("sudo -n setcap cap_sys_admin+ep \"$NCU_BIN\"", source)
        self.assertIn("getcap \"$NCU_BIN\"", source)
        self.assertIn("remove_ncu_capability", source)
        self.assertIn("sudo -n setcap -r \"$NCU_BIN\"", source)
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
        self.assertIn(
            'if [[ "${BENCH_CUDA_READBACK_DIAGNOSTIC:-0}" == "1" ]]; then\n  trap package_partial_on_error ERR',
            source,
        )
        self.assertIn("diagnostic_fail", source)
        self.assertIn("SHA256SUMS.partial", source)
        self.assertIn("Checksummed partial diagnostic evidence", source)

    def test_explicit_failure_packages_checksummed_partial_without_work_state(self):
        source = SCRIPT.read_text()
        start = source.index("remove_ncu_capability() {")
        end = source.index(
            '\nif [[ "${BENCH_CUDA_READBACK_DIAGNOSTIC:-0}" == "1" ]]', start
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

    def test_documents_both_profiler_compatibility_preflights(self):
        source = SCRIPT.read_text()
        self.assertIn("cuTensorMapEncodeIm2colWide", source)
        self.assertIn("passwordless sudo is required", source)
        self.assertIn("Version 2025.2.1.", source)


if __name__ == "__main__":
    unittest.main()

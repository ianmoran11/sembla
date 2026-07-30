import os
import subprocess
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


if __name__ == "__main__":
    unittest.main()

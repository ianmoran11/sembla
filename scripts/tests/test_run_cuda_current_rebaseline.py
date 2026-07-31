import copy
import importlib.util
import json
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("current_protocol", ROOT / "scripts/run-cuda-current-rebaseline.py")
MODULE = importlib.util.module_from_spec(SPEC); SPEC.loader.exec_module(MODULE)

class Fixture:
    def __init__(self, root):
        self.root = pathlib.Path(root); self.root.mkdir(parents=True, exist_ok=True); self.binary = self.root / "sembla"; self.model = self.root / "model"; self.state = self.root / "state"; self.evidence = self.root / "evidence"
        self.binary.write_bytes(b"bin"); self.model.write_bytes(b"model"); self.state.write_bytes(b"state")
        self.provenance = {"repository_commit": "a" * 40, "repository_status": "", "binary": str(self.binary), "binary_sha256": MODULE.sha256_file(self.binary), "model": str(self.model), "model_sha256": MODULE.sha256_file(self.model), "state": str(self.state), "state_sha256": MODULE.sha256_file(self.state)}
        self.manifest = MODULE.build_manifest(self.binary, self.model, self.state, self.evidence, self.provenance)
    def final(self, record):
        materialized = record["id"] == 1; downloaded = {"state": 2 * 1024 * 1024, "inputs": 8, "input_counts": 8}; downloaded["total"] = sum(downloaded.values())
        return {"schema": MODULE.FINAL_STATE_SCHEMA, "mode": "materialized" if materialized else "packed-pageable", "one_time_allocation_ms": 0.0, "pageable_dtoh_host_api_ms": 10.0, "pinned_dtoh_enqueue_api_ms": None, "wait_to_pinned_host_readable_ms": None, "pinned_to_cacheable_staging_copy_ms": None, "host_state_reconstruction_ms": 1.0 if materialized else None, "cpu_sha256_ms": 2.0, "attributed_phase_sum_ms": 12.0, "unattributed_timer_overhead_ms": .1, "final_state_seam_total_ms": 12.1, "timer_tolerance_ms": .01, "final_state_seam_total_excludes_one_time_allocation": True, "phases_reconcile": True, "allocation_plus_seam_reconciles_with_draw_wall": True, "downloaded_bytes": downloaded, "buffer_accounting": {"buffer_set_count": 0, "underlying_pinned_allocation_count": 0, "pinned_bytes": 0, "cacheable_staging_bytes": 0}}
    def timing(self, record):
        aggregate = {"requested_lane_count": record["workers"], "retained_lane_count": record["workers"], "requested_buffer_set_count": 0, "buffer_set_count": 0, "requested_underlying_pinned_allocation_count": 0, "underlying_pinned_allocation_count": 0, "effective_pinned_bytes": 0, "effective_cacheable_staging_bytes": 0, "requested_pinned_bytes": 0, "requested_cacheable_staging_bytes": 0}
        result = {"schema": MODULE.TIMING_SCHEMAS[record["workers"]], "repository_commit": "a" * 40, "binary_sha256": self.provenance["binary_sha256"], "draws": record["draws"], "ticks_per_draw": 24, "setup_wall_time_ms": 10.0 + (record.get("repetition") or 0), "whole_sweep_wall_time_ms": 500.0, "draw_timings": [{"k": k, "wall_time_ms": 100.0, "final_state": self.final(record)} for k in range(record["draws"])], "final_state_buffer_accounting": aggregate}
        if record["workers"] == 4: result.update(execution_window_wall_time_ms=400.0, publication_wall_time_ms=2.0)
        return result
    def make(self, record, changed=False, malformed=False):
        output = pathlib.Path(record["output_dir"]); output.mkdir(parents=True, exist_ok=True)
        (output / "draw_0.csv").write_bytes(b"same\n" if not changed else b"different\n")
        (output / "run-manifest.json").write_text(json.dumps({"executions": [{"final_state_sha256": "f" * 64} for _ in range(record["draws"])]}, sort_keys=True))
        timing = self.timing(record)
        if malformed: timing["draw_timings"][0]["final_state"]["cpu_sha256_ms"] = None
        path = pathlib.Path(record["timing_json"]); path.parent.mkdir(parents=True, exist_ok=True); path.write_text(json.dumps(timing))
    def executor(self, calls, fail=None, changed=None, malformed=None):
        def execute(record):
            calls.append(record["id"]); self.make(record, record["id"] == changed, record["id"] == malformed)
            arm = pathlib.Path(record["arm_dir"]); arm.mkdir(parents=True, exist_ok=True)
            stdout, stderr = arm / "stdout.txt", arm / "stderr.txt"
            stdout.write_text(""); stderr.write_text("")
            result = {**record, "return_code": 124 if record["id"] == fail else 0, "timed_out": record["id"] == fail, "resource_sampling_complete": True, "resource_samples": [{"rss_bytes": 10, "vram_bytes": 20, "rss_query_succeeded": True, "vram_query_succeeded": True}], "peak_rss_bytes": 10, "peak_vram_bytes": 20, "stdout": str(stdout), "stderr": str(stderr)}
            MODULE.atomic_json(arm / "record.json", result); return result
        return execute

class BytecodeHygieneTests(unittest.TestCase):
    def test_collector_and_analyzer_help_do_not_create_support_bytecode(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td)
            scripts = root / "scripts"
            scripts.mkdir()
            for name in (
                "run-cuda-current-rebaseline.py",
                "analyze-cuda-current-rebaseline.py",
                "run-cuda-final-state-decision.py",
                "analyze-cuda-final-state-decision.py",
            ):
                shutil.copy2(ROOT / "scripts" / name, scripts / name)
            environment = os.environ.copy()
            environment.pop("PYTHONDONTWRITEBYTECODE", None)
            environment.pop("PYTHONPYCACHEPREFIX", None)
            for script in ("run-cuda-current-rebaseline.py", "analyze-cuda-current-rebaseline.py"):
                result = subprocess.run(
                    [sys.executable, str(scripts / script), "--help"],
                    cwd=root,
                    env=environment,
                    text=True,
                    capture_output=True,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertFalse(any(root.rglob("__pycache__")))
            self.assertFalse(any(root.rglob("*.pyc")))


class ManifestTests(unittest.TestCase):
    def test_exact_six_executions_eighteen_draws_and_selector_scope(self):
        with tempfile.TemporaryDirectory() as td:
            records = Fixture(td).manifest["executions"]
            self.assertEqual(len(records), 6); self.assertEqual(sum(r["draws"] for r in records), 18)
            self.assertEqual([r["class"] for r in records], ["control-preflight", "current-preflight", "timed", "timed", "timed", "profile"])
            self.assertEqual(records[0]["environment"], {MODULE.SELECTOR: "materialized"})
            self.assertTrue(all(r["environment"] == {} for r in records[1:])); self.assertTrue(all(r["environment_unset"] == list(MODULE.FORBIDDEN_INHERITED) for r in records))
            self.assertEqual(sum(r["profiled"] for r in records), 1); self.assertEqual(sum(r["included_in_performance"] for r in records), 3)
    def test_mutated_count_order_shape_noise_identity_and_argv_fail_closed(self):
        with tempfile.TemporaryDirectory() as td:
            fixture = Fixture(td); mutations = []
            for field, value in (("workers", 2), ("draws", 20), ("noise", "crn"), ("environment", {MODULE.SELECTOR: "packed-pageable"}), ("provenance", {})):
                item = copy.deepcopy(fixture.manifest); item["executions"][2][field] = value; mutations.append(item)
            item = copy.deepcopy(fixture.manifest); item["executions"].pop(); mutations.append(item)
            item = copy.deepcopy(fixture.manifest); item["executions"][2]["benchmark_argv"].append("--bad"); mutations.append(item)
            item = copy.deepcopy(fixture.manifest); item["executions"][5]["argv"][8] = "/wrong/nsight/output"; mutations.append(item)
            for manifest in mutations:
                with self.assertRaises(MODULE.ProtocolError): MODULE.validate_manifest(manifest)
    def test_inherited_current_and_retired_selectors_are_rejected_before_evidence(self):
        with tempfile.TemporaryDirectory() as td:
            root = pathlib.Path(td); binary = root / "bin"; model = root / "model"; state = root / "state"; binary.write_bytes(b"x"); model.write_bytes(b"x"); state.write_bytes(b"x")
            for variable in MODULE.FORBIDDEN_INHERITED:
                evidence = root / variable
                env = os.environ.copy(); env[variable] = "1"
                result = subprocess.run([sys.executable, str(ROOT / "scripts/run-cuda-current-rebaseline.py"), "--binary", str(binary), "--model", str(model), "--state", str(state), "--evidence", str(evidence)], env=env, text=True, capture_output=True)
                self.assertEqual(result.returncode, 1); self.assertIn(variable, result.stderr); self.assertFalse(evidence.exists())

class BarrierTests(unittest.TestCase):
    def test_preflight_parity_schema_timeout_and_negative_control_stop_before_timed(self):
        cases = ((2, None, None), (None, 2, None), (None, None, 2))
        for fail, changed, malformed in cases:
            with self.subTest(fail=fail, changed=changed, malformed=malformed), tempfile.TemporaryDirectory() as td:
                fixture = Fixture(td); calls = []
                with self.assertRaises(MODULE.ProtocolError): MODULE.execute_protocol(fixture.manifest, fixture.evidence, fixture.executor(calls, fail, changed, malformed), lambda r: {})
                self.assertEqual(calls, [1, 2]); status = json.loads((fixture.evidence / "protocol-status.json").read_text()); self.assertFalse(status["timed_started"]); self.assertTrue((fixture.evidence / "SHA256SUMS.partial").is_file())
    def test_accepted_negative_control_stops_before_timed(self):
        with tempfile.TemporaryDirectory() as td:
            fixture = Fixture(td); calls = []
            with mock.patch.object(MODULE, "negative_control", return_value={"accepted": True, "rejected": False}):
                with self.assertRaises(MODULE.ProtocolError): MODULE.execute_protocol(fixture.manifest, fixture.evidence, fixture.executor(calls), lambda r: {})
            self.assertEqual(calls, [1, 2])
    def test_complete_protocol_profiles_only_last_and_compares_four_draw_current_arms(self):
        with tempfile.TemporaryDirectory() as td:
            fixture = Fixture(td); calls = []; profiles = []
            def exporter(record): profiles.append(record["id"]); return {n: "x" for n in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum")}
            status = MODULE.execute_protocol(fixture.manifest, fixture.evidence, fixture.executor(calls), exporter)
            self.assertTrue(status["completed"]); self.assertEqual(calls, list(range(1, 7))); self.assertEqual(profiles, [6])
            self.assertEqual(len(json.loads((fixture.evidence / "comparisons.json").read_text())), 4)
    def test_nonzero_pinned_accounting_and_current_reconstruction_are_rejected(self):
        with tempfile.TemporaryDirectory() as td:
            fixture = Fixture(td); record = fixture.manifest["executions"][2]; fixture.make(record); path = pathlib.Path(record["timing_json"]); value = json.loads(path.read_text()); value["draw_timings"][0]["final_state"]["buffer_accounting"]["pinned_bytes"] = 1; path.write_text(json.dumps(value))
            with self.assertRaises(MODULE.ProtocolError): MODULE.validate_timing(path, record)
    def test_timeout_record_uses_124_and_preserves_record(self):
        with tempfile.TemporaryDirectory() as td:
            fixture = Fixture(td); record = copy.deepcopy(fixture.manifest["executions"][0]); record["argv"] = [sys.executable, "-c", "import time; time.sleep(5)"]
            result = MODULE.run_command(record, timeout_seconds=.03); self.assertEqual(result["return_code"], 124); self.assertTrue(result["timed_out"]); self.assertTrue((pathlib.Path(record["arm_dir"]) / "record.json").is_file())

if __name__ == "__main__": unittest.main()

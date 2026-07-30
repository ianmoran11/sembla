import copy
import importlib.util
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

ROOT = pathlib.Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "final_state_protocol", ROOT / "scripts/run-cuda-final-state-decision.py"
)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class ProtocolFixture:
    def __init__(self, root: pathlib.Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)
        self.binary = root / "sembla"
        self.model = root / "model.json"
        self.state = root / "state.bin"
        self.evidence = root / "evidence"
        self.binary.write_bytes(b"binary")
        self.model.write_text("{}\n")
        self.state.write_bytes(b"state")
        self.provenance = {
            "repository_commit": "a" * 40,
            "repository_status": "",
            "binary": str(self.binary),
            "binary_sha256": MODULE.sha256_file(self.binary),
            "model": str(self.model),
            "model_sha256": MODULE.sha256_file(self.model),
            "state": str(self.state),
            "state_sha256": MODULE.sha256_file(self.state),
        }
        self.manifest = MODULE.build_manifest(
            self.binary, self.model, self.state, self.evidence, self.provenance
        )

    def final(self, mode):
        pageable = mode in ("A", "B")
        pinned = mode == "C"
        reconstructed = mode == "A"
        downloaded = {"state": 2 * 1024 * 1024, "inputs": 8, "input_counts": 8}
        downloaded["total"] = sum(downloaded.values())
        return {
            "schema": MODULE.FINAL_STATE_SCHEMA,
            "mode": MODULE.MODES[mode],
            "one_time_allocation_ms": 1.0 if pinned else 0.0,
            "pageable_dtoh_host_api_ms": 10.0 if pageable else None,
            "pinned_dtoh_enqueue_api_ms": 1.0 if pinned else None,
            "wait_to_pinned_host_readable_ms": 2.0 if pinned else None,
            "pinned_to_cacheable_staging_copy_ms": 1.0 if pinned else None,
            "host_state_reconstruction_ms": 1.0 if reconstructed else None,
            "cpu_sha256_ms": 1.0,
            "attributed_phase_sum_ms": 12.0 if reconstructed else (11.0 if mode == "B" else 5.0),
            "unattributed_timer_overhead_ms": 0.1,
            "final_state_seam_total_ms": 12.1 if reconstructed else (11.1 if mode == "B" else 5.1),
            "final_state_seam_total_excludes_one_time_allocation": True,
            "timer_tolerance_ms": 0.001,
            "phases_reconcile": True,
            "allocation_plus_seam_reconciles_with_draw_wall": True,
            "downloaded_bytes": downloaded,
            "buffer_accounting": {
                "buffer_set_count": 1 if pinned else 0,
                "underlying_pinned_allocation_count": 3 if pinned else 0,
                "pinned_bytes": downloaded["total"] if pinned else 0,
                "cacheable_staging_bytes": downloaded["total"] if pinned else 0,
            },
        }

    def timing(self, record):
        draws = [
            {"k": k, "wall_time_ms": 100.0, "final_state": self.final(record["mode"])}
            for k in range(record["draws"])
        ]
        pinned = record["mode"] == "C"
        lanes = record["workers"] if pinned else 0
        total = self.final(record["mode"])["downloaded_bytes"]["total"] * lanes
        document = {
            "schema": MODULE.TIMING_SCHEMAS[record["workers"]],
            "repository_commit": record["provenance"]["repository_commit"],
            "binary_sha256": record["provenance"]["binary_sha256"],
            "draws": record["draws"],
            "ticks_per_draw": MODULE.TICKS,
            "setup_wall_time_ms": 10.0,
            "whole_sweep_wall_time_ms": 400.0,
            "draw_timings": draws,
            "final_state_buffer_accounting": {
                "requested_lane_count": record["workers"],
                "retained_lane_count": record["workers"],
                "requested_pinned_bytes": total,
                "effective_pinned_bytes": total,
                "requested_cacheable_staging_bytes": total,
                "effective_cacheable_staging_bytes": total,
                "requested_buffer_set_count": lanes,
                "buffer_set_count": lanes,
                "requested_underlying_pinned_allocation_count": 3 * lanes,
                "underlying_pinned_allocation_count": 3 * lanes,
            },
        }
        if record["workers"] == 4:
            document.update(
                execution_window_wall_time_ms=390.0,
                publication_wall_time_ms=1.0,
            )
        return document

    def make_output(self, record, changed=False):
        output = pathlib.Path(record["output_dir"])
        output.mkdir(parents=True, exist_ok=True)
        (output / "manifest.csv").write_text("same\n" if not changed else "changed\n")
        (output / "run-manifest.json").write_text(
            json.dumps(
                {
                    "executions": [
                        {"final_state_sha256": "f" * 64}
                        for _ in range(record["draws"])
                    ]
                },
                sort_keys=True,
            )
            + "\n"
        )
        timing = pathlib.Path(record["timing_json"])
        timing.parent.mkdir(parents=True, exist_ok=True)
        timing.write_text(json.dumps(self.timing(record)) + "\n")

    def executor(self, calls, fail_id=None, changed_id=None, invalid_timing_id=None):
        def execute(record):
            calls.append(record["id"])
            self.make_output(record, record["id"] == changed_id)
            if record["id"] == invalid_timing_id:
                timing = json.loads(pathlib.Path(record["timing_json"]).read_text())
                del timing["draw_timings"][0]["final_state"]["cpu_sha256_ms"]
                pathlib.Path(record["timing_json"]).write_text(json.dumps(timing))
            result = {
                **record,
                "return_code": 124 if record["id"] == fail_id else 0,
                "timed_out": record["id"] == fail_id,
                "resource_sampling_complete": True,
                "resource_sampling_error": None,
                "resource_samples": [
                    {
                        "rss_bytes": 123,
                        "vram_bytes": 456,
                        "rss_query_succeeded": True,
                        "vram_query_succeeded": True,
                    }
                ],
                "peak_rss_bytes": 123,
                "peak_vram_bytes": 456,
            }
            arm = pathlib.Path(record["arm_dir"])
            arm.mkdir(parents=True, exist_ok=True)
            MODULE.atomic_json(arm / "record.json", result)
            return result
        return execute


class ManifestTests(unittest.TestCase):
    def test_exact_27_record_schedule_and_performance_scope(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            records = fixture.manifest["executions"]
            self.assertEqual(len(records), 27)
            self.assertEqual("".join(r["mode"] for r in records[:3]), "ABC")
            self.assertEqual(
                ["".join(r["mode"] for r in records[3 + i * 3 : 6 + i * 3]) for i in range(6)],
                ["ABC", "CBA", "ABC", "CBA", "ABC", "CBA"],
            )
            self.assertEqual(sum(r["included_in_performance"] for r in records), 18)
            self.assertEqual(sum(r["profiled"] for r in records), 3)
            self.assertTrue(all(r["draws"] != 20 and r["workers"] != 2 for r in records))

    def test_mutated_count_order_shape_noise_selector_and_command_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            mutations = []
            missing = copy.deepcopy(fixture.manifest)
            missing["executions"].pop()
            mutations.append(missing)
            for field, value in [
                ("mode", "C"),
                ("workers", 2),
                ("draws", 20),
                ("noise", "crn"),
                ("environment", {MODULE.SELECTOR: "materialized", "EXTRA": "1"}),
            ]:
                changed = copy.deepcopy(fixture.manifest)
                changed["executions"][3][field] = value
                mutations.append(changed)
            argv = copy.deepcopy(fixture.manifest)
            argv["executions"][3]["benchmark_argv"].append("--bad")
            mutations.append(argv)
            for manifest in mutations:
                with self.assertRaises(MODULE.ProtocolError):
                    MODULE.validate_manifest(manifest)


class ValidationAndBarrierTests(unittest.TestCase):
    def test_all_mode_timing_topologies_and_zero_treatment_controls(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            for record in fixture.manifest["executions"][:3]:
                fixture.make_output(record)
                MODULE.validate_timing(pathlib.Path(record["timing_json"]), record)

    def test_timing_provenance_required_fields_and_exact_lane_buffers_fail_closed(self):
        mutations = (
            "commit",
            "allocation-flag",
            "missing-nullable-phase",
            "underallocated-lane",
        )
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = ProtocolFixture(pathlib.Path(temporary))
                    record = fixture.manifest["executions"][2]
                    fixture.make_output(record)
                    path = pathlib.Path(record["timing_json"])
                    timing = json.loads(path.read_text())
                    if mutation == "commit":
                        timing["repository_commit"] = "b" * 40
                    elif mutation == "allocation-flag":
                        del timing["draw_timings"][0]["final_state"][
                            "final_state_seam_total_excludes_one_time_allocation"
                        ]
                    elif mutation == "missing-nullable-phase":
                        del timing["draw_timings"][0]["final_state"][
                            "pinned_dtoh_enqueue_api_ms"
                        ]
                    else:
                        timing["final_state_buffer_accounting"]["buffer_set_count"] = 0
                    path.write_text(json.dumps(timing))
                    with self.assertRaises(MODULE.ProtocolError):
                        MODULE.validate_timing(path, record)

    def test_timeout_preserves_partial_and_never_starts_matrix(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            calls = []
            with self.assertRaisesRegex(MODULE.ProtocolError, "failed with 124"):
                MODULE.execute_protocol(
                    fixture.manifest,
                    fixture.evidence,
                    fixture.executor(calls, fail_id=3),
                    lambda record: {},
                )
            self.assertEqual(calls, [1, 2, 3])
            status = json.loads((fixture.evidence / "protocol-status.json").read_text())
            self.assertFalse(status["matrix_started"])
            self.assertEqual(status["execution_count"], 3)
            self.assertTrue((fixture.evidence / "SHA256SUMS.partial").is_file())
            self.assertFalse((fixture.evidence / "arms" / fixture.manifest["executions"][3]["name"]).exists())

    def test_parity_and_diagnostic_failure_stop_before_matrix(self):
        for changed_id, invalid_id in [(2, None), (None, 3)]:
            with self.subTest(changed_id=changed_id, invalid_id=invalid_id):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = ProtocolFixture(pathlib.Path(temporary))
                    calls = []
                    with self.assertRaises(MODULE.ProtocolError):
                        MODULE.execute_protocol(
                            fixture.manifest,
                            fixture.evidence,
                            fixture.executor(calls, changed_id=changed_id, invalid_timing_id=invalid_id),
                            lambda record: {},
                        )
                    self.assertEqual(calls, [1, 2, 3])

    def test_accepted_negative_control_stops_before_matrix(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            calls = []
            with mock.patch.object(
                MODULE,
                "negative_control",
                return_value={"accepted": True, "rejected": False},
            ):
                with self.assertRaises(MODULE.ProtocolError):
                    MODULE.execute_protocol(
                        fixture.manifest,
                        fixture.evidence,
                        fixture.executor(calls),
                        lambda record: {},
                    )
            self.assertEqual(calls, [1, 2, 3])

    def test_complete_fake_protocol_runs_exactly_27_and_profiles_last(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            calls, profiles = [], []
            def exporter(record):
                profiles.append(record["id"])
                return {name: str(pathlib.Path(record["arm_dir"]) / f"{name}.csv") for name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum")}
            status = MODULE.execute_protocol(
                fixture.manifest,
                fixture.evidence,
                fixture.executor(calls),
                exporter,
            )
            self.assertTrue(status["completed"])
            self.assertEqual(calls, list(range(1, 28)))
            self.assertEqual(profiles, [25, 26, 27])
            comparisons = json.loads((fixture.evidence / "comparisons.json").read_text())
            self.assertEqual(len(comparisons), 9)
            self.assertTrue(all(item["passed"] for item in comparisons))

    def test_resource_sample_is_structured_without_cuda(self):
        sample = MODULE.sample_resources(__import__("os").getpid())
        self.assertGreaterEqual(sample["rss_bytes"], 0)
        self.assertGreaterEqual(sample["vram_bytes"], 0)
        self.assertGreaterEqual(sample["process_count"], 1)

    def test_successful_child_with_failed_resource_sampling_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            record = copy.deepcopy(fixture.manifest["executions"][0])
            record["argv"] = [sys.executable, "-c", "import time; time.sleep(.05)"]
            original = MODULE.sample_resources
            MODULE.sample_resources = lambda _pid: {
                "rss_bytes": 0,
                "vram_bytes": 0,
                "rss_query_succeeded": False,
                "vram_query_succeeded": False,
            }
            try:
                result = MODULE.run_command(record, timeout_seconds=1)
            finally:
                MODULE.sample_resources = original
            self.assertEqual(result["return_code"], 125)
            self.assertFalse(result["resource_sampling_complete"])

    def test_timeout_record_preserves_partial_status_and_resource_peaks(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = ProtocolFixture(pathlib.Path(temporary))
            record = copy.deepcopy(fixture.manifest["executions"][0])
            record["argv"] = [sys.executable, "-c", "import time; time.sleep(5)"]
            result = MODULE.run_command(record, timeout_seconds=0.05)
            self.assertEqual(result["return_code"], 124)
            self.assertTrue(result["timed_out"])
            self.assertGreaterEqual(result["peak_rss_bytes"], 0)
            self.assertGreaterEqual(result["peak_vram_bytes"], 0)
            saved = json.loads((pathlib.Path(record["arm_dir"]) / "record.json").read_text())
            self.assertEqual(saved["return_code"], 124)
            self.assertIn("resource_samples", saved)


if __name__ == "__main__":
    unittest.main()

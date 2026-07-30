import importlib.util
import json
import math
import pathlib
import shutil
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


ANALYZER = load("final_state_analyzer", ROOT / "scripts/analyze-cuda-final-state-decision.py")
DRIVER_TEST = load("final_state_driver_test", ROOT / "scripts/tests/test_run_cuda_final_state_decision.py")


class EvidenceFixture:
    def __init__(self, root: pathlib.Path):
        self.fixture = DRIVER_TEST.ProtocolFixture(root)
        self.root = self.fixture.evidence
        self.calls = []

        def exporter(record):
            arm = pathlib.Path(record["arm_dir"])
            profile = arm / "profile"
            profile.mkdir(parents=True, exist_ok=True)
            pathlib.Path(record["nsys_report"]).write_bytes(b"raw nsys report")
            duration = 2.5 if record["mode"] != "C" else 2.25
            destination = "Pageable" if record["mode"] in ("A", "B") else "Pinned"
            trace = profile / "cuda_gpu_trace.csv"
            rows = [
                "Start (ms),Duration (ms),Name,Bytes,SrcMemKd,DstMemKd,Ctx,Strm"
            ]
            for index in range(4):
                rows.append(
                    f"{index * duration},{duration},[CUDA memcpy DtoH],2097152,Device,{destination},1,7"
                )
            rows.append("0,1,my_kernel,0,Device,Device,1,7")
            trace.write_text("\n".join(rows) + "\n")
            api = profile / "cuda_api_sum.csv"
            api.write_text(
                "Time (%),Total Time (ms),Num Calls,Name\n"
                "60,10,4,cuMemcpyDtoHAsync\n40,4,4,cuStreamSynchronize\n"
            )
            kernels = profile / "cuda_gpu_kern_sum.csv"
            kernels.write_text("Time (%),Total Time (ms),Instances,Name\n100,1,1,my_kernel\n")
            return {
                "cuda_gpu_trace": str(trace),
                "cuda_api_sum": str(api),
                "cuda_gpu_kern_sum": str(kernels),
            }

        DRIVER_TEST.MODULE.execute_protocol(
            self.fixture.manifest,
            self.root,
            self.fixture.executor(self.calls),
            exporter,
        )
        self.set_exact_boundary_timings()
        DRIVER_TEST.MODULE.write_checksums(self.root)

    def records(self):
        return self.fixture.manifest["executions"]

    def timing(self, record):
        path = pathlib.Path(record["timing_json"])
        return path, json.loads(path.read_text())

    def set_exact_boundary_timings(self):
        for record in self.records():
            path, timing = self.timing(record)
            if record["class"] == "timed":
                if record["workers"] == 4:
                    walls = {"A": 100.0, "B": 95.0, "C": 90.25}
                else:
                    walls = {"A": 100.0, "B": 102.0, "C": 104.04}
                timing["whole_sweep_wall_time_ms"] = walls[record["mode"]]
                if record["mode"] == "C":
                    for draw in timing["draw_timings"]:
                        draw["final_state"]["pinned_dtoh_enqueue_api_ms"] = 4.0
                        draw["final_state"]["wait_to_pinned_host_readable_ms"] = 5.0
            path.write_text(json.dumps(timing) + "\n")

    def set_timed_walls(self, workers, mode, values):
        records = [
            record
            for record in self.records()
            if record["class"] == "timed" and record["workers"] == workers and record["mode"] == mode
        ]
        for record, value in zip(sorted(records, key=lambda item: item["repetition"]), values):
            path, timing = self.timing(record)
            timing["whole_sweep_wall_time_ms"] = value
            path.write_text(json.dumps(timing) + "\n")
        DRIVER_TEST.MODULE.write_checksums(self.root)


class ThresholdTests(unittest.TestCase):
    def base(self):
        return {
            "B_A_workers4": 0.95,
            "B_A_workers1": 1.02,
            "C_B_workers4": 0.95,
            "C_B_workers1": 1.02,
        }

    def test_exact_boundaries_pass_and_next_float_fails(self):
        exact = ANALYZER.evaluate_thresholds(self.base(), 0.90, 0.90)
        self.assertTrue(all(gate["passed"] for gate in exact.values()))
        cases = [
            ("B_A_workers4", "B_workers4_wall", 0.95),
            ("B_A_workers1", "B_workers1_wall", 1.02),
            ("C_B_workers4", "C_workers4_wall", 0.95),
            ("C_B_workers1", "C_workers1_wall", 1.02),
        ]
        for input_name, gate_name, boundary in cases:
            ratios = self.base()
            ratios[input_name] = math.nextafter(boundary, math.inf)
            result = ANALYZER.evaluate_thresholds(ratios, 0.90, 0.90)
            self.assertFalse(result[gate_name]["passed"])
        host = ANALYZER.evaluate_thresholds(self.base(), math.nextafter(0.90, math.inf), 0.90)
        self.assertFalse(host["C_host_mechanism"]["passed"])
        nsight = ANALYZER.evaluate_thresholds(self.base(), 0.90, math.nextafter(0.90, math.inf))
        self.assertFalse(nsight["C_nsight_mechanism"]["passed"])

    def test_mechanism_or_gate(self):
        host_only = ANALYZER.evaluate_thresholds(self.base(), 0.9, 0.91)
        nsight_only = ANALYZER.evaluate_thresholds(self.base(), 0.91, 0.9)
        neither = ANALYZER.evaluate_thresholds(self.base(), 0.91, 0.91)
        self.assertTrue(host_only["C_mechanism_OR"]["passed"])
        self.assertTrue(nsight_only["C_mechanism_OR"]["passed"])
        self.assertFalse(neither["C_mechanism_OR"]["passed"])
        self.assertNotIn("staging", ANALYZER.THRESHOLDS["C_host_mechanism"]["numerator"])


class NsightTests(unittest.TestCase):
    def timing(self, sizes):
        return {
            "draw_timings": [
                {
                    "final_state": {
                        "downloaded_bytes": {
                            "state": size,
                            "inputs": 0,
                            "input_counts": 0,
                            "total": size,
                        }
                    }
                }
                for size in sizes
            ]
        }

    def write_trace(self, path, copy_rows, include_header=True, kernel_rows=()):
        rows = []
        if include_header:
            rows.append(
                "Start (ms),Duration (ms),Name,Bytes,SrcMemKd,DstMemKd,Ctx,Strm"
            )
        rows.extend(copy_rows)
        rows.extend(kernel_rows)
        path.write_text("\n".join(rows) + "\n")

    def test_union_overlap_repeated_sizes_rounding_and_tiny_exclusion(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "trace.csv"
            self.write_trace(
                path,
                [
                    "0,5,[CUDA memcpy DtoH],2097151,Device,Pinned,1,7",
                    "5,5,[CUDA memcpy DtoH],2097153,Device,Pinned,1,7",
                    "2,1,[CUDA memcpy DtoH],64,Device,Pinned,1,7",
                ],
                kernel_rows=["4,2,my_kernel,0,Device,Device,1,7"],
            )
            result = ANALYZER.analyze_nsys_trace(
                path, self.timing([2097152, 2097152]), "C"
            )
            self.assertEqual(result["large_copy_count"], 2)
            self.assertEqual(len(result["unmatched_tiny_dtoh"]), 1)
            self.assertAlmostEqual(result["copy_union_duration_ms"], 10.0)
            self.assertAlmostEqual(result["copy_kernel_overlap_ms"], 2.0)
            self.assertAlmostEqual(result["exposed_final_state_dtoh_ms"], 8.0)

    def test_no_overlap_and_pageable_destination(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "trace.csv"
            self.write_trace(
                path,
                ["0,3,[CUDA memcpy DtoH],2097152,Device,Pageable,1,0"],
                kernel_rows=["4,1,my_kernel,0,Device,Device,1,0"],
            )
            result = ANALYZER.analyze_nsys_trace(path, self.timing([2097152]), "B")
            self.assertEqual(result["copy_kernel_overlap_ms"], 0)
            self.assertEqual(result["exposed_final_state_dtoh_ms"], 3)

    def test_missing_columns_wrong_multiplicity_and_extra_large_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            missing = root / "missing.csv"
            missing.write_text("Start,Duration,Name,Bytes\n0,1,x,1\n")
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze_nsys_trace(missing, self.timing([2097152]), "B")
            wrong = root / "wrong.csv"
            self.write_trace(wrong, [])
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze_nsys_trace(wrong, self.timing([2097152]), "B")
            extra = root / "extra.csv"
            self.write_trace(
                extra,
                [
                    "0,1,[CUDA memcpy DtoH],2097152,Device,Pageable,1,0",
                    "2,1,[CUDA memcpy DtoH],3145728,Device,Pageable,1,0",
                ],
            )
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze_nsys_trace(extra, self.timing([2097152]), "B")


class CompletenessAndVerdictTests(unittest.TestCase):
    def test_complete_exact_boundary_fixture_emits_go_eligible_not_promotion(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            result = ANALYZER.analyze(fixture.root)
            self.assertTrue(result["complete"])
            self.assertTrue(result["eligibility"]["B_packed_pageable"])
            self.assertTrue(result["eligibility"]["C_packed_pinned"])
            self.assertFalse(result["eligibility"]["promotion_authorized"])
            self.assertEqual(result["eligibility"]["verdict"], "GO-ELIGIBLE-FOR-LATER-PRD")
            self.assertEqual(len(result["ratios"]["B_A_workers4"]["raw"]), 3)
            self.assertIn("absolute_command_metrics", result)
            self.assertIn("resources_by_workers", result)

    def test_evidence_can_be_reanalyzed_after_transfer_to_a_new_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fixture = EvidenceFixture(base / "original")
            moved = base / "moved"
            shutil.copytree(fixture.root, moved)
            shutil.rmtree(base / "original")
            result = ANALYZER.analyze(moved)
            self.assertTrue(result["complete"])

    def test_one_outlier_does_not_override_median_but_threshold_miss_is_no_go(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            fixture.set_timed_walls(4, "B", [90.0, 150.0, 90.0])
            result = ANALYZER.analyze(fixture.root)
            self.assertTrue(result["eligibility"]["B_packed_pageable"])
            fixture.set_timed_walls(4, "B", [96.0, 96.0, 96.0])
            result = ANALYZER.analyze(fixture.root)
            self.assertFalse(result["eligibility"]["B_packed_pageable"])

    def test_workers1_regression_above_two_percent_vetoes_only_treatment(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            fixture.set_timed_walls(1, "B", [103.0, 103.0, 103.0])
            result = ANALYZER.analyze(fixture.root)
            self.assertFalse(result["eligibility"]["B_packed_pageable"])
            self.assertTrue(result["eligibility"]["C_packed_pinned"])

    def test_missing_resource_samples_are_rejected_after_valid_checksums(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            record = fixture.fixture.manifest["executions"][4]
            path = pathlib.Path(record["arm_dir"]) / "record.json"
            payload = json.loads(path.read_text())
            payload["resource_samples"] = []
            payload["resource_sampling_complete"] = False
            path.write_text(json.dumps(payload) + "\n")
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            with self.assertRaisesRegex(ANALYZER.AnalysisError, "resource evidence"):
                ANALYZER.analyze(fixture.root)

    def test_resource_peak_over_h100_headroom_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            for record in fixture.fixture.manifest["executions"]:
                if record["class"] == "timed" and record["mode"] == "C":
                    path = pathlib.Path(record["arm_dir"]) / "record.json"
                    payload = json.loads(path.read_text())
                    payload["peak_rss_bytes"] = 200 * 1024**3
                    path.write_text(json.dumps(payload) + "\n")
                    break
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze(fixture.root)

    def test_incomplete_or_inconsistent_evidence_is_rejected(self):
        mutations = [
            "missing-record", "duplicate-arm", "extra-arm", "command", "commit",
            "parity", "negative", "timing", "unbounded", "nsight", "kernel",
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = EvidenceFixture(pathlib.Path(temporary))
                    if mutation == "missing-record":
                        pathlib.Path(fixture.records()[5]["arm_dir"]).joinpath("record.json").unlink()
                    elif mutation in ("duplicate-arm", "extra-arm"):
                        path = fixture.root / "execution-manifest.json"
                        manifest = json.loads(path.read_text())
                        if mutation == "duplicate-arm":
                            manifest["executions"][5] = manifest["executions"][4]
                        else:
                            manifest["executions"].append(manifest["executions"][-1])
                        path.write_text(json.dumps(manifest))
                    elif mutation == "command":
                        path = pathlib.Path(fixture.records()[5]["arm_dir"]) / "record.json"
                        record = json.loads(path.read_text())
                        record["argv"].append("--drift")
                        path.write_text(json.dumps(record))
                    elif mutation == "commit":
                        path = pathlib.Path(fixture.records()[5]["arm_dir"]) / "record.json"
                        record = json.loads(path.read_text())
                        record["provenance"]["repository_commit"] = "b" * 40
                        path.write_text(json.dumps(record))
                    elif mutation == "parity":
                        path = fixture.root / "comparisons.json"
                        comparisons = json.loads(path.read_text())
                        comparisons[0]["passed"] = False
                        path.write_text(json.dumps(comparisons))
                    elif mutation == "negative":
                        path = fixture.root / "negative-control.json"
                        value = json.loads(path.read_text())
                        value.update(accepted=True, rejected=False)
                        path.write_text(json.dumps(value))
                    elif mutation == "timing":
                        path = pathlib.Path(fixture.records()[5]["timing_json"])
                        path.unlink()
                    elif mutation == "unbounded":
                        record = next(r for r in fixture.records() if r["class"] == "timed" and r["mode"] == "C")
                        path, timing = fixture.timing(record)
                        timing["final_state_buffer_accounting"]["buffer_set_count"] = 99
                        path.write_text(json.dumps(timing))
                    else:
                        record = next(
                            r
                            for r in fixture.records()
                            if r["class"] == "profile" and r["mode"] == "B"
                        )
                        name = (
                            "cuda_gpu_kern_sum.csv"
                            if mutation == "kernel"
                            else "cuda_gpu_trace.csv"
                        )
                        path = pathlib.Path(record["arm_dir"]) / "profile" / name
                        path.write_text("malformed\n")
                    with self.assertRaises(ANALYZER.AnalysisError):
                        ANALYZER.analyze(fixture.root)

    def test_markdown_and_json_include_thresholds_phases_resources_and_risks(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            result = ANALYZER.analyze(fixture.root)
            text = ANALYZER.markdown(result)
            self.assertIn("Frozen thresholds", text)
            self.assertIn("Raw adjacent repetitions", text)
            self.assertIn("Residual risks", text)
            self.assertEqual(result["thresholds"]["C_mechanism_OR"]["operator"], "OR")
            self.assertIn("final_state_seam_total_ms", next(iter(result["absolute_command_metrics"].values())))


if __name__ == "__main__":
    unittest.main()

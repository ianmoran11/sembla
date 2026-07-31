import importlib.util
import json
import math
import pathlib
import re
from decimal import Decimal
import shutil
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
JSON_NUMBER = r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?"


def replace_json_number_literals(path, key, literals):
    pattern = re.compile(rf'(\"{re.escape(key)}\"\s*:\s*)({JSON_NUMBER})')
    text = path.read_text()
    matches = list(pattern.finditer(text))
    if len(matches) != len(literals):
        raise AssertionError(
            f"expected {len(literals)} {key} values in {path}, found {len(matches)}"
        )
    replacements = iter(literals)
    path.write_text(
        pattern.sub(lambda match: f"{match.group(1)}{next(replacements)}", text)
    )


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
                "cuda_gpu_trace": (pathlib.Path(record["arm_relative"]) / "profile" / trace.name).as_posix(),
                "cuda_api_sum": (pathlib.Path(record["arm_relative"]) / "profile" / api.name).as_posix(),
                "cuda_gpu_kern_sum": (pathlib.Path(record["arm_relative"]) / "profile" / kernels.name).as_posix(),
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

    def test_exact_decimal_operand_boundaries_do_not_need_an_epsilon(self):
        exact_wall = ANALYZER.exact_ratio(95.00190, 100.002, "wall")
        self.assertEqual(exact_wall, Decimal("0.95"))
        ratios = {
            "B_A_workers4": ANALYZER.ratio_decimal_text(exact_wall),
            "B_A_workers1": "1.02",
            "C_B_workers4": "0.95",
            "C_B_workers1": "1.02",
        }
        result = ANALYZER.evaluate_thresholds(ratios, "0.90", "0.90")
        self.assertTrue(all(gate["passed"] for gate in result.values()))
        ratios["B_A_workers4"] = "0.9500000000000000001"
        result = ANALYZER.evaluate_thresholds(ratios, "0.90", "0.90")
        self.assertFalse(result["B_workers4_wall"]["passed"])

    def test_values_above_threshold_beyond_decimal_context_precision_fail(self):
        above = ANALYZER.exact_ratio(
            "95000000000000000000000000001",
            "100000000000000000000000000000",
            "beyond-context wall",
        )
        above_host = ANALYZER.exact_ratio(
            "90000000000000000000000000001",
            "100000000000000000000000000000",
            "beyond-context host",
        )
        ratios = self.base()
        ratios["B_A_workers4"] = above
        result = ANALYZER.evaluate_thresholds(ratios, above_host, Decimal("0.90"))
        self.assertFalse(result["B_workers4_wall"]["passed"])
        self.assertFalse(result["C_host_mechanism"]["passed"])
        self.assertGreater(
            Decimal(result["B_workers4_wall"]["actual_decimal"]), Decimal("0.95")
        )
        self.assertGreater(
            Decimal(result["C_host_mechanism"]["actual_decimal"]), Decimal("0.90")
        )

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

    def test_context_alias_prefers_exact_ctx_over_greenctx(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "trace.csv"
            path.write_text(
                "Start (ms),Duration (ms),Name,Bytes,SrcMemKd,DstMemKd,Ctx,GreenCtx,Strm\n"
                "0,3,[CUDA memcpy DtoH],2097152,Device,Pageable,1,0,7\n"
            )
            result = ANALYZER.analyze_nsys_trace(path, self.timing([2097152]), "B")
            self.assertEqual(result["large_copy_count"], 1)

    def test_zero_mb_rounded_tiny_dtoh_is_not_rejected_or_matched(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "trace.csv"
            path.write_text(
                "Start (ms),Duration (ms),Name,Bytes (MB),SrcMemKd,DstMemKd,Ctx,Strm\n"
                "0,1,[CUDA memcpy DtoH],0.000,Device,Pageable,1,7\n"
                "1,3,[CUDA memcpy DtoH],2.000,Device,Pageable,1,7\n"
            )
            result = ANALYZER.analyze_nsys_trace(path, self.timing([2_000_000]), "B")
            self.assertEqual(result["large_copy_count"], 1)
            self.assertEqual(result["unmatched_tiny_dtoh"][0]["bytes"], 0)

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

    def test_large_d2d_and_htod_are_not_final_state_dtoh(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "trace.csv"
            self.write_trace(
                path,
                [
                    "0,1,[CUDA memcpy Device-to-Device],3145728,Device,Device,1,7",
                    "1,1,[CUDA memcpy Host-to-Device],4194304,Pageable,Device,1,7",
                    "2,3,[CUDA memcpy Device-to-Host],2097152,Device,Pageable,1,7",
                ],
            )
            result = ANALYZER.analyze_nsys_trace(path, self.timing([2097152]), "B")
            self.assertEqual(result["large_copy_count"], 1)
            self.assertEqual(result["large_copy_bytes"], 2097152)

    def test_mixed_destination_kinds_and_direction_disagreements_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            mixed = root / "mixed.csv"
            self.write_trace(
                mixed,
                [
                    "0,1,[CUDA memcpy DtoH],2097152,Device,Pinned,1,7",
                    "1,1,[CUDA memcpy DtoH],2097152,Device,Pageable,1,7",
                ],
            )
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze_nsys_trace(
                    mixed, self.timing([2097152, 2097152]), "C"
                )
            contradictions = (
                "0,1,[CUDA memcpy HtoD],2097152,Device,Pageable,1,7",
                "0,1,[CUDA memcpy Device-to-Device],2097152,Device,Pinned,1,7",
                "0,1,[CUDA memcpy DtoH],2097152,Pageable,Device,1,7",
            )
            for index, row in enumerate(contradictions):
                with self.subTest(row=row):
                    disagreement = root / f"disagreement-{index}.csv"
                    self.write_trace(disagreement, [row])
                    with self.assertRaisesRegex(
                        ANALYZER.AnalysisError, "direction|D2H activity"
                    ):
                        ANALYZER.analyze_nsys_trace(
                            disagreement, self.timing([2097152]), "B"
                        )

    def test_directionless_memcpy_cannot_satisfy_final_state_copy(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = pathlib.Path(temporary) / "directionless.csv"
            self.write_trace(
                path,
                ["0,1,[CUDA memcpy],2097152,Device,Pageable,1,7"],
            )
            with self.assertRaisesRegex(ANALYZER.AnalysisError, "missing final-state D2H"):
                ANALYZER.analyze_nsys_trace(path, self.timing([2097152]), "B")

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
    def set_raw_gate_operands(
        self,
        fixture,
        *,
        wall_workers4_ratio,
        wall_workers1_ratio,
        host_ratio,
    ):
        a4 = Decimal("2350.486887312202")
        b4 = a4 * wall_workers4_ratio
        c4 = b4 * wall_workers4_ratio
        a1 = Decimal("8540.789147668733")
        b1 = a1 * wall_workers1_ratio
        c1 = b1 * wall_workers1_ratio
        host_denominator = Decimal("8453.941617061801")
        host_numerator = host_denominator * host_ratio
        walls = {
            (4, "A"): a4,
            (4, "B"): b4,
            (4, "C"): c4,
            (1, "A"): a1,
            (1, "B"): b1,
            (1, "C"): c1,
        }
        for record in fixture.records():
            if record["class"] != "timed":
                continue
            path = pathlib.Path(record["timing_json"])
            replace_json_number_literals(
                path,
                "whole_sweep_wall_time_ms",
                [format(walls[(record["workers"], record["mode"])], "f")],
            )
            if record["workers"] == 4 and record["mode"] == "B":
                replace_json_number_literals(
                    path,
                    "pageable_dtoh_host_api_ms",
                    [format(host_denominator, "f"), "0", "0", "0"],
                )
            elif record["workers"] == 4 and record["mode"] == "C":
                replace_json_number_literals(
                    path,
                    "pinned_dtoh_enqueue_api_ms",
                    [format(host_numerator, "f"), "0", "0", "0"],
                )
                replace_json_number_literals(
                    path,
                    "wait_to_pinned_host_readable_ms",
                    ["0", "0", "0", "0"],
                )
        DRIVER_TEST.MODULE.write_checksums(fixture.root)

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

    def test_raw_json_exact_boundaries_pass_and_next_decimal_values_fail(self):
        cases = (
            (
                Decimal("0.95"),
                Decimal("1.02"),
                Decimal("0.90"),
                True,
            ),
            (
                Decimal("0.9500000000000000001"),
                Decimal("1.0200000000000000001"),
                Decimal("0.9000000000000000001"),
                False,
            ),
        )
        for workers4, workers1, host, expected in cases:
            with self.subTest(expected=expected):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = EvidenceFixture(pathlib.Path(temporary))
                    self.set_raw_gate_operands(
                        fixture,
                        wall_workers4_ratio=workers4,
                        wall_workers1_ratio=workers1,
                        host_ratio=host,
                    )
                    result = ANALYZER.analyze(fixture.root)
                    expected_ratios = {
                        "B_workers4_wall": workers4,
                        "C_workers4_wall": workers4,
                        "B_workers1_wall": workers1,
                        "C_workers1_wall": workers1,
                        "C_host_mechanism": host,
                    }
                    for gate, expected_ratio in expected_ratios.items():
                        threshold = result["thresholds"][gate]
                        self.assertEqual(threshold["passed"], expected)
                        actual = Decimal(threshold["actual_decimal"])
                        if expected:
                            self.assertEqual(actual, expected_ratio)
                        else:
                            self.assertGreater(actual, Decimal(str(threshold["value"])))
                    json.dumps(result)

    def test_raw_host_phase_sum_above_boundary_beyond_decimal_context_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            half = "0.450000000000000000000000000005"
            for record in fixture.records():
                if record["class"] != "timed" or record["workers"] != 4:
                    continue
                path = pathlib.Path(record["timing_json"])
                if record["mode"] == "B":
                    replace_json_number_literals(
                        path,
                        "pageable_dtoh_host_api_ms",
                        ["1", "0", "0", "0"],
                    )
                elif record["mode"] == "C":
                    replace_json_number_literals(
                        path,
                        "pinned_dtoh_enqueue_api_ms",
                        [half, "0", "0", "0"],
                    )
                    replace_json_number_literals(
                        path,
                        "wait_to_pinned_host_readable_ms",
                        [half, "0", "0", "0"],
                    )
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            gate = ANALYZER.analyze(fixture.root)["thresholds"]["C_host_mechanism"]
            self.assertEqual(
                gate["actual_decimal"], "0.90000000000000000000000000001"
            )
            self.assertFalse(gate["passed"])

    def test_exact_decimal_result_writes_parseable_json_and_markdown(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fixture = EvidenceFixture(base)
            self.set_raw_gate_operands(
                fixture,
                wall_workers4_ratio=Decimal("0.95"),
                wall_workers1_ratio=Decimal("1.02"),
                host_ratio=Decimal("0.90"),
            )
            timing_path = pathlib.Path(fixture.records()[0]["timing_json"])
            timing_text = timing_path.read_text()
            marker = '"final_state_buffer_accounting": {'
            self.assertIn(marker, timing_text)
            timing_path.write_text(
                timing_text.replace(
                    marker,
                    marker + '"future_fractional_metric": 0.1234567890123456789, ',
                    1,
                )
            )
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            json_output = base / "decision.json"
            markdown_output = base / "decision.md"
            status = ANALYZER.main(
                [
                    str(fixture.root),
                    "--json",
                    str(json_output),
                    "--markdown",
                    str(markdown_output),
                ]
            )
            self.assertEqual(status, 0)
            rendered = json.loads(json_output.read_text())
            self.assertTrue(rendered["thresholds"]["B_workers4_wall"]["passed"])
            self.assertIn("## Frozen thresholds", markdown_output.read_text())

    def test_evidence_can_be_reanalyzed_after_transfer_to_a_new_root(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = pathlib.Path(temporary)
            fixture = EvidenceFixture(base / "original")
            moved = base / "moved"
            shutil.copytree(fixture.root, moved)
            shutil.rmtree(base / "original")
            result = ANALYZER.analyze(moved)
            self.assertTrue(result["complete"])

    def test_host_mechanism_exact_decimal_boundary_survives_phase_aggregation(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            for record in fixture.records():
                if record["class"] != "timed" or record["workers"] != 4:
                    continue
                path, timing = fixture.timing(record)
                for draw in timing["draw_timings"]:
                    final = draw["final_state"]
                    if record["mode"] == "B":
                        final["pageable_dtoh_host_api_ms"] = 1.0
                    elif record["mode"] == "C":
                        final["pinned_dtoh_enqueue_api_ms"] = 0.338
                        final["wait_to_pinned_host_readable_ms"] = 0.562
                path.write_text(json.dumps(timing) + "\n")
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            result = ANALYZER.analyze(fixture.root)
            gate = result["thresholds"]["C_host_mechanism"]
            self.assertEqual(Decimal(gate["actual_decimal"]), Decimal("0.90"))
            self.assertTrue(gate["passed"])

    def test_nsight_mechanism_exact_decimal_boundary_survives_interval_math(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            for mode, duration in (("B", Decimal("1.0")), ("C", Decimal("0.9"))):
                record = next(
                    item
                    for item in fixture.records()
                    if item["class"] == "profile" and item["mode"] == mode
                )
                destination = "Pageable" if mode == "B" else "Pinned"
                rows = [
                    "Start (ms),Duration (ms),Name,Bytes,SrcMemKd,DstMemKd,Ctx,Strm"
                ]
                for index in range(4):
                    start = duration * index
                    rows.append(
                        f"{start},{duration},[CUDA memcpy DtoH],2097152,Device,{destination},1,7"
                    )
                path = pathlib.Path(record["arm_dir"]) / "profile" / "cuda_gpu_trace.csv"
                path.write_text("\n".join(rows) + "\n")
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            result = ANALYZER.analyze(fixture.root)
            gate = result["thresholds"]["C_nsight_mechanism"]
            self.assertEqual(Decimal(gate["actual_decimal"]), Decimal("0.90"))
            self.assertTrue(gate["passed"])

    def test_nsight_ratio_above_boundary_beyond_decimal_context_fails(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            durations = {
                "B": "1",
                "C": "0.90000000000000000000000000001",
            }
            for mode, duration in durations.items():
                record = next(
                    item
                    for item in fixture.records()
                    if item["class"] == "profile" and item["mode"] == mode
                )
                destination = "Pageable" if mode == "B" else "Pinned"
                rows = [
                    "Start (ms),Duration (ms),Name,Bytes,SrcMemKd,DstMemKd,Ctx,Strm"
                ]
                for index in range(4):
                    rows.append(
                        f"{index * 2},{duration},[CUDA memcpy DtoH],2097152,Device,{destination},1,7"
                    )
                path = pathlib.Path(record["arm_dir"]) / "profile" / "cuda_gpu_trace.csv"
                path.write_text("\n".join(rows) + "\n")
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            gate = ANALYZER.analyze(fixture.root)["thresholds"]["C_nsight_mechanism"]
            self.assertEqual(
                gate["actual_decimal"], "0.90000000000000000000000000001"
            )
            self.assertFalse(gate["passed"])

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
                    payload["resource_samples"][0]["rss_bytes"] = 200 * 1024**3
                    path.write_text(json.dumps(payload) + "\n")
                    break
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            with self.assertRaises(ANALYZER.AnalysisError):
                ANALYZER.analyze(fixture.root)

    def test_claimed_peaks_query_flags_and_samples_must_reconcile(self):
        mutations = ("hidden-large-sample", "failed-query-with-bytes", "wrong-peak")
        for mutation in mutations:
            with self.subTest(mutation=mutation):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = EvidenceFixture(pathlib.Path(temporary))
                    record = fixture.records()[4]
                    path = pathlib.Path(record["arm_dir"]) / "record.json"
                    payload = json.loads(path.read_text())
                    sample = payload["resource_samples"][0]
                    if mutation == "hidden-large-sample":
                        sample["rss_bytes"] = 200 * 1024**3
                    elif mutation == "failed-query-with-bytes":
                        sample["rss_query_succeeded"] = False
                    else:
                        payload["peak_vram_bytes"] += 1
                    path.write_text(json.dumps(payload) + "\n")
                    DRIVER_TEST.MODULE.write_checksums(fixture.root)
                    with self.assertRaises(ANALYZER.AnalysisError):
                        ANALYZER.analyze(fixture.root)

    def test_contradictory_parity_and_negative_control_are_rejected(self):
        for mutation in ("parity", "negative"):
            with self.subTest(mutation=mutation):
                with tempfile.TemporaryDirectory() as temporary:
                    fixture = EvidenceFixture(pathlib.Path(temporary))
                    if mutation == "parity":
                        path = fixture.root / "comparisons.json"
                        value = json.loads(path.read_text())
                        value[0].update(
                            passed=True, tree_parity=False, digest_parity=False
                        )
                        value[0]["A_B"]["equal"] = False
                    else:
                        path = fixture.root / "negative-control.json"
                        value = json.loads(path.read_text())
                        value.update(accepted=False, rejected=True)
                        value["comparison"].update(equal=True, changed=[])
                    path.write_text(json.dumps(value) + "\n")
                    DRIVER_TEST.MODULE.write_checksums(fixture.root)
                    with self.assertRaises(ANALYZER.AnalysisError):
                        ANALYZER.analyze(fixture.root)

    def test_profile_exports_must_belong_to_their_arm(self):
        with tempfile.TemporaryDirectory() as temporary:
            fixture = EvidenceFixture(pathlib.Path(temporary))
            profiles = {
                record["mode"]: record
                for record in fixture.records()
                if record["class"] == "profile"
            }
            a_record = pathlib.Path(profiles["A"]["arm_dir"]) / "record.json"
            b_record = pathlib.Path(profiles["B"]["arm_dir"]) / "record.json"
            a_payload = json.loads(a_record.read_text())
            b_payload = json.loads(b_record.read_text())
            b_payload["nsys_exports"]["cuda_gpu_trace"] = a_payload["nsys_exports"][
                "cuda_gpu_trace"
            ]
            b_record.write_text(json.dumps(b_payload) + "\n")
            DRIVER_TEST.MODULE.write_checksums(fixture.root)
            with self.assertRaisesRegex(ANALYZER.AnalysisError, "owning arm"):
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
            for required in (
                "Preflight performance (informational only)",
                "whole_sweep_wall_time_ms",
                "setup_wall_time_ms",
                "execution_window_wall_time_ms",
                "publication_wall_time_ms",
                "final_state_seam_total_ms",
                "pageable_dtoh_host_api_ms",
                "pinned_dtoh_enqueue_api_ms",
                "wait_to_pinned_host_readable_ms",
                "host_state_reconstruction_ms",
                "cpu_sha256_ms",
                "large_copy_count",
                "copy_union_duration_ms",
                "copy_kernel_overlap_ms",
                "peak_rss_bytes",
                "peak_vram_bytes",
                "requested_pinned_bytes",
                "requested_staging_bytes",
                "Frozen thresholds",
                "Raw adjacent repetitions",
                "Residual risks",
            ):
                self.assertIn(required, text)
            self.assertTrue(result["preflight_performance"]["informational_only"])
            self.assertEqual(len(result["preflight_performance"]["commands"]), 3)
            self.assertEqual(result["thresholds"]["C_mechanism_OR"]["operator"], "OR")
            command = next(iter(result["absolute_command_metrics"].values()))
            for identity in (
                "id", "class", "mode", "workers", "draws", "noise",
                "repetition", "profiled", "included_in_performance",
            ):
                self.assertIn(identity, command)
            self.assertIn("final_state_seam_total_ms", command)
            self.assertIn("ratio_decimal", result["nsight_mechanism"])


if __name__ == "__main__":
    unittest.main()

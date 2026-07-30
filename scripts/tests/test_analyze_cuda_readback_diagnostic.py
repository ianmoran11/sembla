import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "analyze-cuda-readback-diagnostic.py"
SPEC = importlib.util.spec_from_file_location("cuda_readback_analyzer", SCRIPT)
ANALYZER = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader
SPEC.loader.exec_module(ANALYZER)
KERNELS = [
    "sembla_prepare_effects",
    "sembla_bound_grouped_view",
    "sembla_observe_view",
    "sembla_observe_grouped_view",
    "sembla_count_deferred",
]


class DiagnosticAnalyzerTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        phase = {
            "schema": "sembla-execution-timing-v1",
            "session": {
                "backend": "cuda",
                "scale": 10_000_000,
                "ticks": 24,
                "seed": 9009,
                "repository_commit": "a" * 40,
                "binary_sha256": "b" * 64,
            },
            "ticks": [
                {
                    "tick": tick,
                    "wall_time_ms": 10.0,
                    "phases_ms": {
                        "kernels": 4.0,
                        "readback_control": 3.0,
                        "state_transfer": 0.0,
                        "state_reconstruct": 0.0,
                        "state_hash": 0.0,
                        "observe_views": 0.0,
                        "report": 2.0,
                        "other": 1.0,
                    },
                }
                for tick in range(24)
            ],
            "totals": {
                "wall_time_ms": 240.0,
                "phases_ms": {
                    "kernels": 96.0,
                    "readback_control": 72.0,
                    "state_transfer": 0.0,
                    "state_reconstruct": 0.0,
                    "state_hash": 0.0,
                    "observe_views": 0.0,
                    "report": 48.0,
                    "other": 24.0,
                },
            },
            "self_check": {"all_ticks_reconciled": True, "other_non_negative": True},
        }
        self._json("phase.json", phase)
        self._json(
            "sequential-timing.json",
            {
                "schema": "sembla-sweep-timing-v1",
                "backend": "cuda",
                "repository_commit": "a" * 40,
                "binary_sha256": "b" * 64,
                "draws": 4,
                "ticks_per_draw": 24,
                "whole_sweep_wall_time_ms": 1000.0,
            },
        )
        self._json(
            "concurrent-timing.json",
            {
                "schema": "sembla-sweep-concurrency-spike-timing-v1",
                "backend": "cuda",
                "repository_commit": "a" * 40,
                "binary_sha256": "b" * 64,
                "draws": 4,
                "ticks_per_draw": 24,
                "requested_draw_workers": 4,
                "effective_draw_workers": 4,
                "execution_mode": "cuda-free-nonblocking-streams",
                "execution_window_wall_time_ms": 500.0,
                "whole_sweep_wall_time_ms": 700.0,
            },
        )
        self._trace("sequential-trace.csv", concurrent=False)
        self._trace("concurrent-trace.csv", concurrent=True)
        self._api("sequential-api.csv")
        self._api("concurrent-api.csv")

    def tearDown(self):
        self.temp.cleanup()

    def _json(self, name, value):
        (self.root / name).write_text(json.dumps(value))

    def _trace(self, name, concurrent):
        header = (
            "Start (ns),Duration (ns),CorrId,GrdX,GrdY,GrdZ,BlkX,BlkY,BlkZ,"
            "Reg/Trd,StcSMem (MB),DymSMem (MB),Bytes (MB),Throughput (MB/s),"
            "SrcMemKd,DstMemKd,Device,Ctx,GreenCtx,Strm,Name\n"
        )
        rows = ["preamble\n", header]
        durations = [200, 180, 160, 140, 120] if concurrent else [100] * 5
        for index, (kernel, duration) in enumerate(zip(KERNELS, durations)):
            stream = str(index % 4 + 1) if concurrent else "1"
            rows.append(
                f"{index * 1000 + 100},{duration},1,1,1,1,32,1,1,1,0,0,,,,,GPU,2,,{stream},{kernel}\n"
            )
        for index in range(4):
            stream = str(index + 1) if concurrent else "1"
            # The large copy overlaps its kernel; the tiny copy is unoverlapped.
            rows.append(
                f"{index * 1000 + 150},50,1,,,,,,,,,,48.0,1.0,Device,Pageable,GPU,2,,{stream},[CUDA memcpy Device-to-Host]\n"
            )
            rows.append(
                f"{index * 1000 + 400},20,1,,,,,,,,,,0.001,1.0,Device,Pageable,GPU,2,,{stream},[CUDA memcpy Device-to-Host]\n"
            )
        (self.root / name).write_text("".join(rows))

    def _api(self, name):
        (self.root / name).write_text(
            "preamble\n"
            "Time (%),Total Time (ns),Num Calls,Avg (ns),Med (ns),Min (ns),Max (ns),StdDev (ns),Name\n"
            "50,1000,8,125,125,1,200,1,cuMemcpyDtoHAsync_v2\n"
            "25,500,4,125,125,1,200,1,cuStreamWaitEvent\n"
            "25,500,5,100,100,1,200,1,cuLaunchKernel\n"
        )

    def _ncu(self, path, kernel, report_type, omit=None):
        families = (
            [
                ("GPU Speed Of Light Throughput", "Memory Throughput"),
                ("Occupancy", "Achieved Occupancy"),
                ("Launch Statistics", "Block Size"),
            ]
            if report_type == "sol"
            else [
                ("Memory Workload Analysis", "DRAM Throughput"),
                ("Scheduler Statistics", "Issued Warp Per Scheduler"),
                ("Warp State Statistics", "Warp Cycles Per Issued Instruction"),
            ]
        )
        rows = [
            '"ID","Kernel Name","Section Name","Metric Name","Metric Value"\n'
        ]
        for index, (section, metric) in enumerate(families, 1):
            if omit == section:
                continue
            rows.append(f'"{index}","{kernel}","{section}","{metric}","42.0"\n')
        path.write_text("preamble\n" + "".join(rows))

    def command(self, ncu_dir=None):
        command = [
            sys.executable,
            str(SCRIPT),
            "--phase-timing",
            str(self.root / "phase.json"),
            "--sequential-trace",
            str(self.root / "sequential-trace.csv"),
            "--concurrent-trace",
            str(self.root / "concurrent-trace.csv"),
            "--sequential-api",
            str(self.root / "sequential-api.csv"),
            "--concurrent-api",
            str(self.root / "concurrent-api.csv"),
            "--sequential-timing",
            str(self.root / "sequential-timing.json"),
            "--concurrent-timing",
            str(self.root / "concurrent-timing.json"),
            "--selected-kernels-out",
            str(self.root / "selected.txt"),
            "--output",
            str(self.root / "analysis.json"),
        ]
        if ncu_dir is not None:
            command += [
                "--ncu-dir",
                str(ncu_dir),
                "--assertions",
                str(self.root / "assertions.txt"),
            ]
        return command

    def test_analyzes_traces_and_validates_bounded_ncu_files(self):
        subprocess.run(self.command(), check=True)
        selected = (self.root / "selected.txt").read_text().splitlines()
        self.assertEqual(selected, KERNELS[:3])

        ncu = self.root / "ncu"
        ncu.mkdir()
        for kernel in selected:
            self._ncu(ncu / f"{kernel}-sol.csv", kernel, "sol")
        for kernel in (selected[0], "sembla_count_deferred"):
            self._ncu(ncu / f"{kernel}-detail.csv", kernel, "detail")
        subprocess.run(self.command(ncu), check=True)

        analysis = json.loads((self.root / "analysis.json").read_text())
        self.assertEqual(analysis["schema"], "sembla-cuda-readback-diagnostic-v1")
        self.assertEqual(analysis["systems"]["workers_4"]["kernel_count"], 5)
        self.assertEqual(analysis["systems"]["workers_4"]["d2h"]["calls"], 8)
        self.assertGreater(analysis["systems"]["workers_4"]["d2h"]["kernel_overlap_ms"], 0)
        self.assertTrue(analysis["decision_inputs"]["all_d2h_destinations_pageable"])
        self.assertTrue(analysis["ncu"]["collected"])
        self.assertIn("PASS bounded Nsight Compute", (self.root / "assertions.txt").read_text())

    def test_rejects_missing_concurrent_streams(self):
        self._trace("concurrent-trace.csv", concurrent=False)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not use four nonempty streams", result.stderr)

    def test_interval_math_treats_touching_intervals_as_nonoverlap(self):
        self.assertEqual(ANALYZER.interval_duration([(0, 10), (5, 15), (15, 20)]), 20)
        self.assertEqual(ANALYZER.intersection_duration([(0, 10)], [(10, 20)]), 0)
        self.assertEqual(ANALYZER.intersection_duration([(0, 10)], [(5, 15)]), 5)
        self.assertEqual(ANALYZER.maximum_concurrency([(0, 10), (5, 15), (15, 20)]), 2)

    def test_rejects_missing_ncu_metric_family(self):
        subprocess.run(self.command(), check=True)
        selected = (self.root / "selected.txt").read_text().splitlines()
        ncu = self.root / "ncu"
        ncu.mkdir()
        for kernel in selected:
            self._ncu(ncu / f"{kernel}-sol.csv", kernel, "sol")
        self._ncu(
            ncu / f"{selected[0]}-detail.csv",
            selected[0],
            "detail",
            omit="Warp State Statistics",
        )
        self._ncu(
            ncu / "sembla_count_deferred-detail.csv",
            "sembla_count_deferred",
            "detail",
        )
        result = subprocess.run(self.command(ncu), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing numeric NCU metric family", result.stderr)

    def test_rejects_nonnumeric_ncu_metric_family(self):
        subprocess.run(self.command(), check=True)
        selected = (self.root / "selected.txt").read_text().splitlines()
        ncu = self.root / "ncu"
        ncu.mkdir()
        for kernel in selected:
            self._ncu(ncu / f"{kernel}-sol.csv", kernel, "sol")
        for kernel in (selected[0], "sembla_count_deferred"):
            self._ncu(ncu / f"{kernel}-detail.csv", kernel, "detail")
        target = ncu / f"{selected[0]}-sol.csv"
        target.write_text(target.read_text().replace('"42.0"', '"n/a"', 1))
        result = subprocess.run(self.command(ncu), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing numeric NCU metric family", result.stderr)

    def test_ncu_family_must_come_from_section_name(self):
        subprocess.run(self.command(), check=True)
        selected = (self.root / "selected.txt").read_text().splitlines()
        ncu = self.root / "ncu"
        ncu.mkdir()
        for kernel in selected:
            self._ncu(ncu / f"{kernel}-sol.csv", kernel, "sol")
        for kernel in (selected[0], "sembla_count_deferred"):
            self._ncu(ncu / f"{kernel}-detail.csv", kernel, "detail")
        target = ncu / f"{selected[0]}-sol.csv"
        target.write_text(target.read_text().replace("GPU Speed Of Light Throughput", "Unrelated Section"))
        result = subprocess.run(self.command(ncu), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing numeric NCU metric family", result.stderr)

    def test_rejects_timing_provenance_mismatch(self):
        document = json.loads((self.root / "concurrent-timing.json").read_text())
        document["repository_commit"] = "c" * 40
        self._json("concurrent-timing.json", document)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("repository commit differs", result.stderr)

    def test_rejects_blank_d2h_size(self):
        path = self.root / "concurrent-trace.csv"
        path.write_text(path.read_text().replace(",,48.0,1.0,Device", ",,,1.0,Device", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing numeric CSV field", result.stderr)

    def test_rejects_missing_essential_api_row(self):
        path = self.root / "concurrent-api.csv"
        path.write_text(
            "\n".join(
                line
                for line in path.read_text().splitlines()
                if "cuMemcpyDtoHAsync_v2" not in line
            )
            + "\n"
        )
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("missing essential CUDA API rows", result.stderr)

    def test_rejects_missing_tick_phase_even_when_tick_reconciles(self):
        document = json.loads((self.root / "phase.json").read_text())
        document["ticks"][0]["phases_ms"].pop("readback_control")
        document["ticks"][0]["wall_time_ms"] = 7.0
        document["totals"]["phases_ms"]["readback_control"] = 69.0
        document["totals"]["wall_time_ms"] = 237.0
        self._json("phase.json", document)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unexpected phase set", result.stderr)

    def test_rejects_totals_that_differ_from_tick_aggregates(self):
        document = json.loads((self.root / "phase.json").read_text())
        document["totals"]["phases_ms"]["kernels"] += 1.0
        document["totals"]["wall_time_ms"] += 1.0
        self._json("phase.json", document)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("differs from summed tick", result.stderr)

    def test_rejects_negative_tick_phase(self):
        document = json.loads((self.root / "phase.json").read_text())
        document["ticks"][0]["phases_ms"]["kernels"] = -1.0
        self._json("phase.json", document)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("invalid phase duration", result.stderr)

    def test_rejects_invalid_phase_provenance_identifier(self):
        document = json.loads((self.root / "phase.json").read_text())
        document["session"]["repository_commit"] = " " * 40
        self._json("phase.json", document)
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("not hexadecimal", result.stderr)

    def test_rejects_nonnumeric_grid_dimension(self):
        path = self.root / "concurrent-trace.csv"
        path.write_text(path.read_text().replace(",1,1,1,1,32", ",1,garbage,1,1,32", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("nonnumeric GrdX", result.stderr)

    def test_rejects_blank_kernel_grid_dimension(self):
        path = self.root / "concurrent-trace.csv"
        path.write_text(path.read_text().replace(",1,1,1,1,32", ",1,,1,1,32", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("nonnumeric GrdX", result.stderr)

    def test_rejects_zero_relevant_timestamp(self):
        path = self.root / "concurrent-trace.csv"
        lines = path.read_text().splitlines()
        row = next(index for index, line in enumerate(lines) if "sembla_prepare_effects" in line)
        lines[row] = "0," + lines[row].split(",", 1)[1]
        path.write_text("\n".join(lines) + "\n")
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("non-positive relevant timeline row", result.stderr)

    def test_rejects_negative_d2h_size(self):
        path = self.root / "concurrent-trace.csv"
        path.write_text(path.read_text().replace(",,48.0,1.0,Device", ",,-1.0,1.0,Device", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("negative byte count", result.stderr)

    def test_rejects_nonpositive_api_count(self):
        path = self.root / "concurrent-api.csv"
        path.write_text(path.read_text().replace("50,1000,8,", "50,1000,-8,", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("nonpositive CUDA API count/time", result.stderr)

    def test_rejects_multiple_cuda_contexts(self):
        path = self.root / "concurrent-trace.csv"
        path.write_text(path.read_text().replace("GPU,2,,4,sembla", "GPU,3,,4,sembla", 1))
        result = subprocess.run(self.command(), text=True, capture_output=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("did not use one shared CUDA context", result.stderr)


if __name__ == "__main__":
    unittest.main()

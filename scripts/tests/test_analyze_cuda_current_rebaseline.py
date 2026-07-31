import importlib.util
import json
import pathlib
import shutil
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path); module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module); return module
ANALYZER = load("current_analyzer", ROOT / "scripts/analyze-cuda-current-rebaseline.py")
DRIVER_TEST = load("current_driver_test", ROOT / "scripts/tests/test_run_cuda_current_rebaseline.py")

class Evidence:
    def __init__(self, root):
        self.fixture = DRIVER_TEST.Fixture(root); self.root = self.fixture.evidence; self.calls = []
        def exporter(record):
            profile = pathlib.Path(record["arm_dir"]) / "profile"; profile.mkdir(parents=True, exist_ok=True); pathlib.Path(record["nsys_report"]).write_bytes(b"raw")
            trace = profile / "cuda_gpu_trace.csv"; rows = ["Start (ms),Duration (ms),Name,Bytes (MB),SrcMemKd,DstMemKd,Ctx,GreenCtx,Strm"]
            for i in range(4): rows.append(f"{i * 3},2,[CUDA memcpy DtoH],2.097152,Device,Pageable,1,0,7")
            rows += ["0,1,my_kernel,0,Device,Device,1,0,7", "11,1,[CUDA memcpy DtoH],0.000,Device,Pageable,1,0,7"]; trace.write_text("\n".join(rows) + "\n")
            api = profile / "cuda_api_sum.csv"; api.write_text("Time (%),Total Time (ms),Num Calls,Name\n100,8,4,cuMemcpyDtoH\n")
            kernels = profile / "cuda_gpu_kern_sum.csv"; kernels.write_text("Time (%),Total Time (ms),Instances,Name\n100,1,1,my_kernel\n")
            return {name: (pathlib.Path(record["arm_relative"]) / "profile" / f"{name}.csv").as_posix() for name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum")}
        DRIVER_TEST.MODULE.execute_protocol(self.fixture.manifest, self.root, self.fixture.executor(self.calls), exporter)
        # Make raw repetitions and setup variability visible.
        for record in self.fixture.manifest["executions"][2:5]:
            path = pathlib.Path(record["timing_json"]); timing = json.loads(path.read_text()); repetition = record["repetition"]; timing["whole_sweep_wall_time_ms"] = 500 + repetition * 10; timing["setup_wall_time_ms"] = 10 + repetition; path.write_text(json.dumps(timing))
        DRIVER_TEST.MODULE.write_checksums(self.root)
    def checksum(self): DRIVER_TEST.MODULE.write_checksums(self.root)

class NsightCompatibilityTests(unittest.TestCase):
    def test_exact_ctx_preferred_and_rounded_tiny_row_accepted(self):
        with tempfile.TemporaryDirectory() as td:
            evidence = Evidence(pathlib.Path(td)); result = ANALYZER.analyze(evidence.root); trace = result["profile"]["trace"]
            self.assertEqual(trace["large_copy_count"], 4); self.assertEqual(trace["contexts"], ["1"]); self.assertEqual(trace["unmatched_tiny_dtoh"][0]["bytes"], 0)

class AnalysisTests(unittest.TestCase):
    def test_complete_analysis_emits_raw_statistics_phases_bytes_resources_and_profile(self):
        with tempfile.TemporaryDirectory() as td:
            evidence = Evidence(pathlib.Path(td)); result = ANALYZER.analyze(evidence.root); text = ANALYZER.markdown(result)
            self.assertTrue(result["complete"]); self.assertEqual(result["absolute_metrics"]["whole_sweep_wall_time_ms"], {"raw": [510.0, 520.0, 530.0], "median": 520.0, "minimum": 510.0, "maximum": 530.0, "range": 20.0})
            self.assertFalse(result["interpretation"]["optimization_authorized"]); self.assertIsNone(result["interpretation"]["performance_threshold"])
            for required in ("setup_wall_time_ms", "execution_window_wall_time_ms", "publication_wall_time_ms", "per_draw_wall_time_ms", "final_state_seam_total_ms", "pageable_dtoh_host_api_ms", "cpu_sha256_ms", "attributed_phase_sum_ms", "unattributed_timer_overhead_ms", "downloaded_total_bytes", "peak_rss_bytes", "peak_vram_bytes"):
                self.assertIn(required, result["absolute_metrics"]); self.assertIn(required, text)
            for required in ("large-copy count", "copy-union", "copy/kernel overlap", "exposed D2H", "historical and non-binding", "no performance threshold"):
                self.assertIn(required, text)
    def test_transferred_evidence_reanalyzes(self):
        with tempfile.TemporaryDirectory() as td:
            base = pathlib.Path(td); evidence = Evidence(base / "original"); moved = base / "moved"; shutil.copytree(evidence.root, moved); shutil.rmtree(base / "original"); self.assertTrue(ANALYZER.analyze(moved)["complete"])
    def test_missing_extra_wrong_identity_argv_timing_sample_pinned_export_and_negative_fail_closed(self):
        mutations = ("missing-arm", "extra-arm", "identity", "argv", "timing", "sample", "pinned", "export", "negative", "stdout", "stderr")
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as td:
                evidence = Evidence(pathlib.Path(td)); records = evidence.fixture.manifest["executions"]
                if mutation == "missing-arm": shutil.rmtree(pathlib.Path(records[3]["arm_dir"]))
                elif mutation == "extra-arm": (evidence.root / "arms" / "07-extra").mkdir()
                elif mutation in ("identity", "argv", "sample", "export"):
                    path = pathlib.Path(records[3 if mutation != "export" else 5]["arm_dir"]) / "record.json"; value = json.loads(path.read_text())
                    if mutation == "identity": value["provenance"]["repository_commit"] = "b" * 40
                    elif mutation == "argv": value["argv"].append("--bad")
                    elif mutation == "sample": value["resource_samples"] = []
                    else: value["nsys_exports"].pop("cuda_api_sum")
                    path.write_text(json.dumps(value))
                elif mutation in ("timing", "pinned"):
                    path = pathlib.Path(records[3]["timing_json"]); value = json.loads(path.read_text())
                    if mutation == "timing": value["schema"] = "wrong"
                    else: value["final_state_buffer_accounting"]["requested_pinned_bytes"] = 1
                    path.write_text(json.dumps(value))
                elif mutation == "negative":
                    path = evidence.root / "negative-control.json"; value = json.loads(path.read_text()); value["accepted"] = True; path.write_text(json.dumps(value))
                else:
                    (pathlib.Path(records[3]["arm_dir"]) / f"{mutation}.txt").unlink()
                evidence.checksum()
                with self.assertRaises(ANALYZER.AnalysisError): ANALYZER.analyze(evidence.root)
    def test_incomplete_status_and_malformed_nsys_fail_closed(self):
        for mutation in ("status", "trace", "api", "kernel"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as td:
                evidence = Evidence(pathlib.Path(td))
                if mutation == "status": path = evidence.root / "protocol-status.json"; value = json.loads(path.read_text()); value["completed"] = False; path.write_text(json.dumps(value))
                else:
                    record = evidence.fixture.manifest["executions"][5]; path = pathlib.Path(record["arm_dir"]) / "profile" / f"cuda_{'gpu_' if mutation != 'api' else ''}{'kern_sum' if mutation == 'kernel' else mutation + ('_sum' if mutation == 'api' else '')}.csv"
                    path.write_text("malformed\n")
                evidence.checksum()
                with self.assertRaises(ANALYZER.AnalysisError): ANALYZER.analyze(evidence.root)

if __name__ == "__main__": unittest.main()

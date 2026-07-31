#!/usr/bin/env python3
"""Collect the fixed six-execution CUDA current-path rebaseline protocol."""
from __future__ import annotations

import argparse
import csv
import hashlib
import importlib.util
import json
import math
import os
import pathlib
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from typing import Any, Callable

HERE = pathlib.Path(__file__).resolve().parent
_SPEC = importlib.util.spec_from_file_location("cuda_final_state_support", HERE / "run-cuda-final-state-decision.py")
if _SPEC is None or _SPEC.loader is None:
    raise RuntimeError("could not load CUDA final-state support")
support = importlib.util.module_from_spec(_SPEC)
_previous_dont_write_bytecode = sys.dont_write_bytecode
try:
    # Loading support must not dirty the pinned evidence checkout with a .pyc.
    sys.dont_write_bytecode = True
    _SPEC.loader.exec_module(support)
finally:
    sys.dont_write_bytecode = _previous_dont_write_bytecode

SCHEMA = "sembla-cuda-current-rebaseline-protocol-v1"
TIMING_SCHEMAS = {1: "sembla-sweep-timing-v3", 4: "sembla-sweep-concurrency-spike-timing-v3"}
FINAL_STATE_SCHEMA = "sembla-cuda-final-state-readback-v2"
SELECTOR = "SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE"
RETIRED_SELECTORS = (
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256",
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256_VERIFY",
)
FORBIDDEN_INHERITED = (SELECTOR, *RETIRED_SELECTORS)
SCALE, TICKS, SEED = 10_000_000, 24, 9009
ARM_TIMEOUT_SECONDS = 1_200
PROFILE_EXPORT_TIMEOUT_SECONDS = 300
RESOURCE_SAMPLE_SECONDS = 0.2


class ProtocolError(RuntimeError):
    pass


def atomic_json(path: pathlib.Path, value: Any) -> None:
    support.atomic_json(path, value)


def sha256_file(path: pathlib.Path) -> str:
    return support.sha256_file(path)


def compare_output_trees(left: pathlib.Path, right: pathlib.Path) -> dict[str, Any]:
    return support.compare_output_trees(left, right)


def final_state_hashes(output: pathlib.Path) -> list[str]:
    try:
        return support.final_state_hashes(output)
    except support.ProtocolError as error:
        raise ProtocolError(str(error)) from error


def binary_argv(binary: pathlib.Path, model: pathlib.Path, state: pathlib.Path,
                output: pathlib.Path, timing: pathlib.Path, draws: int, workers: int) -> list[str]:
    return [str(binary), "sweep", str(model), "--population", str(state),
            "--backend", "cuda", "--seed", str(SEED), "--draws", str(draws),
            "--draw-workers", str(workers), "--ticks", str(TICKS), "--noise", "independent",
            "--enable", "grouped-observations", "--timing-json", str(timing), "--out", str(output)]


def build_manifest(binary: pathlib.Path, model: pathlib.Path, state: pathlib.Path,
                   evidence: pathlib.Path, provenance: dict[str, Any]) -> dict[str, Any]:
    shapes = [
        ("control-preflight", "materialized", 1, 1, None, False, False),
        ("current-preflight", "current", 1, 1, None, False, False),
        ("timed", "current", 4, 4, 1, False, True),
        ("timed", "current", 4, 4, 2, False, True),
        ("timed", "current", 4, 4, 3, False, True),
        ("profile", "current", 4, 4, None, True, False),
    ]
    records = []
    for index, (kind, path_mode, workers, draws, repetition, profiled, included) in enumerate(shapes, 1):
        suffix = f"-r{repetition}" if repetition else ""
        name = f"{index:02d}-{kind}-w{workers}-{path_mode}{suffix}"
        arm = evidence / "arms" / name
        output, timing = arm / "output", arm / "timing.json"
        benchmark = binary_argv(binary, model, state, output, timing, draws, workers)
        report = arm / "profile" / "current-path" if profiled else None
        argv = (["nsys", "profile", "--trace=cuda", "--sample=none", "--cpuctxsw=none",
                 "--stats=false", "--force-overwrite=true", "-o", str(report), *benchmark]
                if profiled else benchmark)
        identity = {key: provenance.get(key) for key in
                    ("repository_commit", "binary_sha256", "model_sha256", "state_sha256")}
        records.append({
            "id": index, "name": name, "class": kind, "path_mode": path_mode,
            "selector": "materialized" if index == 1 else None,
            "workers": workers, "draws": draws, "ticks": TICKS, "slots": SCALE,
            "seed": SEED, "noise": "independent", "repetition": repetition,
            "profiled": profiled, "included_in_performance": included,
            "environment": ({SELECTOR: "materialized"} if index == 1 else {}),
            "environment_unset": list(FORBIDDEN_INHERITED), "provenance": identity,
            "argv": argv, "benchmark_argv": benchmark, "arm_dir": str(arm),
            "arm_relative": arm.relative_to(evidence).as_posix(), "output_dir": str(output),
            "output_relative": output.relative_to(evidence).as_posix(), "timing_json": str(timing),
            "timing_relative": timing.relative_to(evidence).as_posix(),
            "nsys_report": str(report) + ".nsys-rep" if report else None,
            "nsys_report_relative": (pathlib.Path(str(report) + ".nsys-rep").relative_to(evidence).as_posix() if report else None),
        })
    manifest = {"schema": SCHEMA, "frozen": {"slots": SCALE, "ticks": TICKS, "seed": SEED,
        "arm_timeout_seconds": ARM_TIMEOUT_SECONDS, "execution_count": 6, "draw_count": 18},
        "provenance": provenance, "evidence_root_at_collection": str(evidence), "executions": records}
    validate_manifest(manifest)
    return manifest


def validate_manifest(manifest: dict[str, Any]) -> None:
    records = manifest.get("executions")
    if manifest.get("schema") != SCHEMA or not isinstance(records, list) or len(records) != 6:
        raise ProtocolError("current-path protocol must contain exactly six executions")
    if [r.get("id") for r in records] != list(range(1, 7)):
        raise ProtocolError("execution IDs/order must be 1..6")
    expected = [
        ("control-preflight", "materialized", 1, 1, None, False, False),
        ("current-preflight", "current", 1, 1, None, False, False),
        ("timed", "current", 4, 4, 1, False, True),
        ("timed", "current", 4, 4, 2, False, True),
        ("timed", "current", 4, 4, 3, False, True),
        ("profile", "current", 4, 4, None, True, False),
    ]
    provenance = manifest.get("provenance", {})
    try:
        binary, model, state = map(pathlib.Path, (provenance["binary"], provenance["model"], provenance["state"]))
    except (KeyError, TypeError) as error:
        raise ProtocolError("binary/model/state provenance is missing") from error
    for record, shape in zip(records, expected):
        actual = tuple(record.get(k) for k in ("class", "path_mode", "workers", "draws", "repetition", "profiled", "included_in_performance"))
        if actual != shape:
            raise ProtocolError(f"execution {record.get('id')} shape/order drifted")
        if record.get("slots") != SCALE or record.get("ticks") != TICKS or record.get("seed") != SEED or record.get("noise") != "independent":
            raise ProtocolError("scientific shape drifted from 10M/24/9009/independent")
        expected_env = {SELECTOR: "materialized"} if record["id"] == 1 else {}
        if record.get("environment") != expected_env or record.get("environment_unset") != list(FORBIDDEN_INHERITED):
            raise ProtocolError("selector environment is not frozen")
        identity = {key: provenance.get(key) for key in ("repository_commit", "binary_sha256", "model_sha256", "state_sha256")}
        if record.get("provenance") != identity or any(not value for value in identity.values()):
            raise ProtocolError("execution provenance is incomplete")
        expected_command = binary_argv(binary, model, state, pathlib.Path(record["output_dir"]), pathlib.Path(record["timing_json"]), record["draws"], record["workers"])
        if record.get("benchmark_argv") != expected_command:
            raise ProtocolError("benchmark argv drifted")
        if record["profiled"]:
            prefix = ["nsys", "profile", "--trace=cuda", "--sample=none", "--cpuctxsw=none", "--stats=false", "--force-overwrite=true", "-o"]
            expected_report = str(pathlib.Path(record["arm_dir"]) / "profile" / "current-path")
            expected_profile_command = [*prefix, expected_report, *expected_command]
            if record.get("argv") != expected_profile_command:
                raise ProtocolError("Nsight wrapper or output target drifted")
            if record.get("nsys_report") != expected_report + ".nsys-rep":
                raise ProtocolError("Nsight report path drifted")
        elif record.get("argv") != expected_command:
            raise ProtocolError("ordinary execution unexpectedly wrapped")
    if sum(r["draws"] for r in records) != 18 or sum(r["profiled"] for r in records) != 1:
        raise ProtocolError("protocol must contain exactly 18 draws and one profile")


def finite(value: Any, label: str, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ProtocolError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0 or (positive and number <= 0):
        raise ProtocolError(f"{label} must be finite and {'positive' if positive else 'non-negative'}")
    return number


def validate_timing(path: pathlib.Path, record: dict[str, Any]) -> dict[str, Any]:
    try:
        doc = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ProtocolError(f"invalid timing document {path}: {error}") from error
    if doc.get("schema") != TIMING_SCHEMAS[record["workers"]]:
        raise ProtocolError(f"unexpected timing schema in {path}")
    identity = record["provenance"]
    if doc.get("repository_commit") != identity["repository_commit"] or doc.get("binary_sha256") != identity["binary_sha256"]:
        raise ProtocolError("timing identity disagrees with manifest")
    if doc.get("draws") != record["draws"] or doc.get("ticks_per_draw") != TICKS:
        raise ProtocolError("timing dimensions disagree with command")
    for field in ("setup_wall_time_ms", "whole_sweep_wall_time_ms"):
        finite(doc.get(field), field, field.startswith("whole"))
    if record["workers"] == 4:
        finite(doc.get("execution_window_wall_time_ms"), "execution window")
        finite(doc.get("publication_wall_time_ms"), "publication")
    draws = doc.get("draw_timings")
    if not isinstance(draws, list) or len(draws) != record["draws"] or [d.get("k") for d in draws] != list(range(record["draws"])):
        raise ProtocolError("draw timing list is incomplete")
    components = ("state", "inputs", "input_counts")
    for draw in draws:
        finite(draw.get("wall_time_ms"), "draw wall", True)
        final = draw.get("final_state")
        if not isinstance(final, dict) or final.get("schema") != FINAL_STATE_SCHEMA:
            raise ProtocolError("final-state diagnostic is missing")
        expected_mode = "materialized" if record["id"] == 1 else "packed-pageable"
        if final.get("mode") != expected_mode:
            raise ProtocolError(f"current path did not report {expected_mode}")
        for field in ("one_time_allocation_ms", "cpu_sha256_ms", "attributed_phase_sum_ms",
                      "unattributed_timer_overhead_ms", "final_state_seam_total_ms", "timer_tolerance_ms"):
            finite(final.get(field), field)
        if final.get("pageable_dtoh_host_api_ms") is None:
            raise ProtocolError("pageable D2H field is missing")
        finite(final["pageable_dtoh_host_api_ms"], "pageable D2H")
        nullable = ("pinned_dtoh_enqueue_api_ms", "wait_to_pinned_host_readable_ms", "pinned_to_cacheable_staging_copy_ms")
        if any(final.get(field) is not None for field in nullable):
            raise ProtocolError("pageable path reported pinned phases")
        reconstruction = final.get("host_state_reconstruction_ms")
        if record["id"] == 1:
            finite(reconstruction, "materialized reconstruction")
        elif reconstruction is not None:
            raise ProtocolError("current packed-pageable path reported reconstruction")
        if final.get("phases_reconcile") is not True or final.get("allocation_plus_seam_reconciles_with_draw_wall") is not True or final.get("final_state_seam_total_excludes_one_time_allocation") is not True:
            raise ProtocolError("final-state timing reconciliation failed")
        downloaded = final.get("downloaded_bytes")
        if not isinstance(downloaded, dict) or any(isinstance(downloaded.get(c), bool) or not isinstance(downloaded.get(c), int) or downloaded[c] < 0 for c in components) or downloaded.get("total") != sum(downloaded[c] for c in components):
            raise ProtocolError("downloaded byte accounting is malformed")
        accounting = final.get("buffer_accounting")
        fields = ("buffer_set_count", "underlying_pinned_allocation_count", "pinned_bytes", "cacheable_staging_bytes")
        if not isinstance(accounting, dict) or any(accounting.get(field) != 0 for field in fields):
            raise ProtocolError("pageable path reported pinned/staging buffers")
    aggregate = doc.get("final_state_buffer_accounting")
    fields = ("requested_buffer_set_count", "buffer_set_count", "requested_underlying_pinned_allocation_count",
              "underlying_pinned_allocation_count", "effective_pinned_bytes", "effective_cacheable_staging_bytes",
              "requested_pinned_bytes", "requested_cacheable_staging_bytes")
    if not isinstance(aggregate, dict) or aggregate.get("requested_lane_count") != record["workers"] or aggregate.get("retained_lane_count") != record["workers"] or any(aggregate.get(field) != 0 for field in fields):
        raise ProtocolError("pageable aggregate pinned/staging accounting is nonzero or malformed")
    return doc


def sample_resources(pid: int) -> dict[str, Any]:
    return support.sample_resources(pid)


def run_command(record: dict[str, Any], timeout_seconds: float = ARM_TIMEOUT_SECONDS) -> dict[str, Any]:
    arm = pathlib.Path(record["arm_dir"]); arm.mkdir(parents=True, exist_ok=True)
    if record["profiled"]:
        pathlib.Path(record["nsys_report"]).parent.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    for name in record["environment_unset"]:
        environment.pop(name, None)
    environment.update(record["environment"])
    started, samples, timed_out = time.monotonic(), [], False
    with (arm / "stdout.txt").open("wb") as stdout, (arm / "stderr.txt").open("wb") as stderr:
        process = subprocess.Popen(record["argv"], env=environment, stdout=stdout, stderr=stderr, start_new_session=True)
        deadline = started + timeout_seconds
        while process.poll() is None:
            samples.append(sample_resources(process.pid))
            if time.monotonic() >= deadline:
                timed_out = True
                try: os.killpg(process.pid, signal.SIGTERM)
                except ProcessLookupError: pass
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    try: os.killpg(process.pid, signal.SIGKILL)
                    except ProcessLookupError: pass
                break
            time.sleep(RESOURCE_SAMPLE_SECONDS)
        return_code = process.wait()
    complete = bool(samples) and any(s.get("rss_query_succeeded") and s.get("rss_bytes", 0) > 0 for s in samples) and any(s.get("vram_query_succeeded") and s.get("vram_bytes", 0) > 0 for s in samples)
    effective = 124 if timed_out else (125 if return_code == 0 and not complete else return_code)
    result = {**record, "return_code": effective, "timed_out": timed_out,
              "wall_time_seconds": time.monotonic() - started, "resource_sampling_complete": complete,
              "resource_sampling_error": None if complete else "no positive process RSS and GPU VRAM sample was captured",
              "resource_samples": samples, "peak_rss_bytes": max((s.get("rss_bytes", 0) for s in samples), default=0),
              "peak_vram_bytes": max((s.get("vram_bytes", 0) for s in samples), default=0),
              "stdout": str(arm / "stdout.txt"), "stderr": str(arm / "stderr.txt")}
    atomic_json(arm / "record.json", result)
    return result


def export_nsys(record: dict[str, Any]) -> dict[str, str]:
    report, arm = pathlib.Path(record["nsys_report"]), pathlib.Path(record["arm_dir"])
    if not report.is_file(): raise ProtocolError(f"missing Nsight report: {report}")
    exports = {}
    for name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum"):
        output = arm / "profile" / f"{name}.csv"
        completed = subprocess.run(["nsys", "stats", "--force-export=true", "--report", name, "--format", "csv", str(report)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=PROFILE_EXPORT_TIMEOUT_SECONDS)
        output.write_bytes(completed.stdout); output.with_suffix(".stderr").write_bytes(completed.stderr)
        if completed.returncode or not output.stat().st_size: raise ProtocolError(f"Nsight export {name} failed")
        exports[name] = (pathlib.Path(record["arm_relative"]) / "profile" / output.name).as_posix()
    return exports


def comparison(label: str, left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
    trees = compare_output_trees(pathlib.Path(left["output_dir"]), pathlib.Path(right["output_dir"]))
    hashes = {"left": final_state_hashes(pathlib.Path(left["output_dir"])), "right": final_state_hashes(pathlib.Path(right["output_dir"]))}
    result = {"label": label, "record_ids": [left["id"], right["id"]], "tree": trees,
              "final_state_sha256": hashes, "tree_parity": trees["equal"], "digest_parity": hashes["left"] == hashes["right"]}
    result["passed"] = result["tree_parity"] and result["digest_parity"]
    return result


def negative_control(output: pathlib.Path, root: pathlib.Path) -> dict[str, Any]:
    perturbed = root / "negative-control-tree"
    if perturbed.exists(): shutil.rmtree(perturbed)
    shutil.copytree(output, perturbed)
    files = sorted(p for p in perturbed.rglob("draw_*.csv") if p.is_file())
    if not files: raise ProtocolError("negative control has no scientific draw CSV to mutate")
    target = files[0]; data = bytearray(target.read_bytes())
    if not data: data.extend(b"\x00")
    else: data[0] ^= 1
    target.write_bytes(data)
    check = compare_output_trees(output, perturbed); relative = target.relative_to(perturbed).as_posix()
    shutil.rmtree(perturbed)
    if check["equal"] or check["changed"] != [relative]:
        raise ProtocolError("comparator accepted deliberate one-byte mutation")
    return {"accepted": False, "rejected": True, "mutation_bytes": 1, "perturbed_relative_path": relative, "comparison": check}


Executor = Callable[[dict[str, Any]], dict[str, Any]]
def execute_protocol(manifest: dict[str, Any], evidence: pathlib.Path, executor: Executor = run_command,
                     nsys_exporter: Callable[[dict[str, Any]], dict[str, str]] = export_nsys) -> dict[str, Any]:
    validate_manifest(manifest); evidence.mkdir(parents=True, exist_ok=True)
    atomic_json(evidence / "execution-manifest.json", manifest)
    status = {"schema": SCHEMA, "phase": "preflight", "timed_started": False, "completed": False, "execution_count": 0}
    atomic_json(evidence / "protocol-status.json", status); completed = []; comparisons = []
    try:
        for record in manifest["executions"][:2]:
            result = executor(record); completed.append(result); status["execution_count"] = len(completed)
            atomic_json(evidence / "protocol-status.json", status)
            if result.get("return_code") != 0: raise ProtocolError(f"preflight execution {record['id']} failed with {result.get('return_code')}")
            validate_timing(pathlib.Path(record["timing_json"]), record)
        parity = comparison("control-current-preflight", *manifest["executions"][:2]); comparisons.append(parity)
        if not parity["passed"]: raise ProtocolError("control/current preflight output or digest parity failed")
        control = negative_control(pathlib.Path(manifest["executions"][0]["output_dir"]), evidence)
        atomic_json(evidence / "negative-control.json", control)
        if control.get("accepted") is not False or control.get("rejected") is not True: raise ProtocolError("negative control was accepted")
        atomic_json(evidence / "comparisons.json", comparisons)
        status.update(phase="timed", preflight_passed=True, timed_started=True); atomic_json(evidence / "protocol-status.json", status)
        current_anchor = manifest["executions"][2]
        for record in manifest["executions"][2:]:
            result = executor(record); completed.append(result); status["execution_count"] = len(completed)
            atomic_json(evidence / "protocol-status.json", status)
            if result.get("return_code") != 0: raise ProtocolError(f"execution {record['id']} failed with {result.get('return_code')}")
            validate_timing(pathlib.Path(record["timing_json"]), record)
            if record["id"] > 3:
                parity = comparison(f"current-3-{record['id']}", current_anchor, record); comparisons.append(parity)
                if not parity["passed"]: raise ProtocolError(f"current output parity failed for execution {record['id']}")
            if record["profiled"]:
                result["nsys_exports"] = nsys_exporter(record); atomic_json(pathlib.Path(record["arm_dir"]) / "record.json", result)
            atomic_json(evidence / "comparisons.json", comparisons)
        status.update(phase="complete", completed=True, execution_count=6); atomic_json(evidence / "protocol-status.json", status)
        return status
    except Exception as error:
        status.update(completed=False, execution_count=len(completed), error=str(error)); atomic_json(evidence / "protocol-status.json", status)
        atomic_json(evidence / "comparisons.json", comparisons); write_checksums(evidence, "SHA256SUMS.partial"); raise


def write_checksums(root: pathlib.Path, name: str = "SHA256SUMS") -> None:
    lines = [f"{sha256_file(path)}  {path.relative_to(root).as_posix()}" for path in sorted(root.rglob("*")) if path.is_file() and path.name != name]
    (root / name).write_text("\n".join(lines) + "\n")


def provenance(binary: pathlib.Path, model: pathlib.Path, state: pathlib.Path) -> dict[str, Any]:
    def command(*argv: str) -> str:
        return subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT).stdout.strip()
    return {"repository_commit": command("git", "rev-parse", "HEAD"),
            "repository_status": command("git", "status", "--porcelain=v1", "--untracked-files=all"),
            "binary": str(binary), "binary_sha256": sha256_file(binary), "model": str(model),
            "model_sha256": sha256_file(model), "state": str(state), "state_sha256": sha256_file(state),
            "cuda_version": command("nvcc", "--version"),
            "gpu_driver_identity": command("nvidia-smi", "--query-gpu=name,driver_version,uuid,pci.bus_id", "--format=csv,noheader")}


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    for name in ("binary", "model", "state", "evidence"): parser.add_argument(f"--{name}", type=pathlib.Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    inherited = [name for name in FORBIDDEN_INHERITED if name in os.environ]
    if inherited: raise ProtocolError("inherited CUDA final-state selectors are forbidden: " + ", ".join(inherited))
    args = parse_args(sys.argv[1:] if argv is None else argv)
    for path in (args.binary, args.model, args.state):
        if not path.is_file(): raise ProtocolError(f"required input is missing: {path}")
    evidence = args.evidence.resolve()
    if evidence.exists() and any(evidence.iterdir()): raise ProtocolError(f"evidence directory is not empty: {evidence}")
    evidence.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(args.binary.resolve(), args.model.resolve(), args.state.resolve(), evidence,
                              provenance(args.binary, args.model, args.state))
    execute_protocol(manifest, evidence); write_checksums(evidence); return 0

if __name__ == "__main__":
    try: raise SystemExit(main())
    except ProtocolError as error:
        print(f"CUDA current-path rebaseline failed: {error}", file=sys.stderr); raise SystemExit(1)

#!/usr/bin/env python3
"""Fail-closed analysis for the fixed CUDA current-path rebaseline."""
from __future__ import annotations

import argparse
import importlib.util
import json
import math
import pathlib
import statistics
import sys
from typing import Any

HERE = pathlib.Path(__file__).resolve().parent

def load(name: str, path: pathlib.Path):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None: raise RuntimeError(f"could not load {path}")
    module = importlib.util.module_from_spec(spec)
    previous = sys.dont_write_bytecode
    try:
        # Analysis must not dirty the pinned evidence checkout with support .pyc files.
        sys.dont_write_bytecode = True
        spec.loader.exec_module(module)
    finally:
        sys.dont_write_bytecode = previous
    return module

protocol = load("cuda_current_rebaseline_protocol", HERE / "run-cuda-current-rebaseline.py")
nsight_support = load("cuda_final_state_analysis_support", HERE / "analyze-cuda-final-state-decision.py")
SCHEMA = "sembla-cuda-current-rebaseline-analysis-v1"

class AnalysisError(RuntimeError): pass

def load_json(path: pathlib.Path) -> Any:
    try: return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error: raise AnalysisError(f"invalid JSON {path}: {error}") from error

def evidence_path(root: pathlib.Path, manifest: dict[str, Any], value: str | None, relative: str | None = None) -> pathlib.Path:
    if relative: return root / relative
    if not value: raise AnalysisError("evidence path is missing")
    path = pathlib.Path(value)
    if not path.is_absolute(): return root / path
    try: return root / path.relative_to(pathlib.Path(manifest["evidence_root_at_collection"]))
    except (ValueError, KeyError):
        if path.exists(): return path
        raise AnalysisError(f"absolute evidence path is outside recorded root: {path}")

def verify_checksums(root: pathlib.Path) -> None:
    try: lines = [line for line in (root / "SHA256SUMS").read_text().splitlines() if line]
    except OSError as error: raise AnalysisError(f"checksum manifest is missing: {error}") from error
    expected = {}
    for line in lines:
        try: digest, relative = line.split(maxsplit=1)
        except ValueError as error: raise AnalysisError("malformed checksum manifest") from error
        relative = relative.removeprefix("*").removeprefix("./")
        if len(digest) != 64 or relative in expected: raise AnalysisError("malformed or duplicate checksum entry")
        expected[relative] = digest
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file() and p.name != "SHA256SUMS"}
    if set(expected) != actual: raise AnalysisError("checksum file set is incomplete or has extras")
    for relative, digest in expected.items():
        if protocol.sha256_file(root / relative) != digest: raise AnalysisError(f"checksum mismatch: {relative}")

def validate_resources(record: dict[str, Any]) -> tuple[int, int]:
    samples = record.get("resource_samples")
    if record.get("resource_sampling_complete") is not True or not isinstance(samples, list) or not samples:
        raise AnalysisError(f"record {record.get('id')} has incomplete resource samples")
    rss, vram = [], []
    for sample in samples:
        if not isinstance(sample, dict): raise AnalysisError("resource sample is malformed")
        for field in ("rss_bytes", "vram_bytes"):
            value = sample.get(field)
            if isinstance(value, bool) or not isinstance(value, int) or value < 0: raise AnalysisError(f"resource {field} is malformed")
        if sample.get("rss_query_succeeded") is True: rss.append(sample["rss_bytes"])
        elif sample.get("rss_query_succeeded") is not False or sample["rss_bytes"] != 0: raise AnalysisError("failed RSS query reported bytes")
        if sample.get("vram_query_succeeded") is True: vram.append(sample["vram_bytes"])
        elif sample.get("vram_query_succeeded") is not False or sample["vram_bytes"] != 0: raise AnalysisError("failed VRAM query reported bytes")
    if not rss or max(rss) <= 0 or not vram or max(vram) <= 0: raise AnalysisError("resource samples lack positive RSS/VRAM")
    if record.get("peak_rss_bytes") != max(rss) or record.get("peak_vram_bytes") != max(vram): raise AnalysisError("resource peaks disagree with samples")
    return max(rss), max(vram)

def validate_negative(value: Any) -> None:
    if not isinstance(value, dict) or value.get("accepted") is not False or value.get("rejected") is not True or value.get("mutation_bytes") != 1:
        raise AnalysisError("deliberate one-byte negative control was accepted or malformed")
    comparison, target = value.get("comparison"), value.get("perturbed_relative_path")
    if not isinstance(comparison, dict) or not isinstance(target, str) or comparison.get("equal") is not False or comparison.get("changed") != [target] or comparison.get("missing_from_right") != [] or comparison.get("extra_in_right") != []:
        raise AnalysisError("negative-control rejection evidence is inconsistent")

def recompute_comparison(root: pathlib.Path, manifest: dict[str, Any], left: dict[str, Any], right: dict[str, Any]) -> dict[str, Any]:
    left_output = evidence_path(root, manifest, left["output_dir"], left.get("output_relative"))
    right_output = evidence_path(root, manifest, right["output_dir"], right.get("output_relative"))
    try:
        tree = protocol.compare_output_trees(left_output, right_output)
        hashes = {"left": protocol.final_state_hashes(left_output), "right": protocol.final_state_hashes(right_output)}
    except (protocol.ProtocolError, protocol.support.ProtocolError) as error:
        raise AnalysisError(str(error)) from error
    return {"tree": tree, "hashes": hashes, "passed": tree["equal"] and hashes["left"] == hashes["right"]}

def validate_comparisons(root: pathlib.Path, manifest: dict[str, Any], comparisons: Any) -> None:
    records = manifest["executions"]
    pairs = [(records[0], records[1]), *((records[2], record) for record in records[3:])]
    labels = ["control-current-preflight", *[f"current-3-{i}" for i in range(4, 7)]]
    if not isinstance(comparisons, list) or len(comparisons) != 4: raise AnalysisError("parity comparisons are incomplete")
    for recorded, (left, right), label in zip(comparisons, pairs, labels):
        actual = recompute_comparison(root, manifest, left, right)
        if recorded.get("label") != label or recorded.get("record_ids") != [left["id"], right["id"]]: raise AnalysisError("parity comparison identity drifted")
        if recorded.get("passed") is not True or recorded.get("tree_parity") is not True or recorded.get("digest_parity") is not True or not actual["passed"]: raise AnalysisError(f"output parity failed: {label}")
        if recorded.get("final_state_sha256") != actual["hashes"]: raise AnalysisError("recorded final-state hashes disagree with outputs")
        tree = recorded.get("tree", {})
        for key in ("missing_from_right", "extra_in_right", "changed"):
            if tree.get(key) != []: raise AnalysisError("recorded tree parity is contradictory")

def timing_metrics(timing: dict[str, Any]) -> dict[str, Any]:
    draws = timing["draw_timings"]; finals = [draw["final_state"] for draw in draws]
    def total(field: str) -> float: return sum(float(final[field]) for final in finals)
    bytes_fields = ("state", "inputs", "input_counts", "total")
    return {
        "whole_sweep_wall_time_ms": float(timing["whole_sweep_wall_time_ms"]),
        "setup_wall_time_ms": float(timing["setup_wall_time_ms"]),
        "execution_window_wall_time_ms": float(timing.get("execution_window_wall_time_ms", sum(float(draw["wall_time_ms"]) for draw in draws))),
        "publication_wall_time_ms": float(timing.get("publication_wall_time_ms", 0)),
        "per_draw_wall_time_ms": statistics.fmean(float(draw["wall_time_ms"]) for draw in draws),
        "per_draw_wall_time_raw_ms": [float(draw["wall_time_ms"]) for draw in draws],
        "final_state_seam_total_ms": total("final_state_seam_total_ms"),
        "pageable_dtoh_host_api_ms": total("pageable_dtoh_host_api_ms"),
        "cpu_sha256_ms": total("cpu_sha256_ms"),
        "attributed_phase_sum_ms": total("attributed_phase_sum_ms"),
        "unattributed_timer_overhead_ms": total("unattributed_timer_overhead_ms"),
        "downloaded_bytes": {field: sum(final["downloaded_bytes"][field] for final in finals) for field in bytes_fields},
    }

def summary(values: list[float | int]) -> dict[str, Any]:
    return {"raw": values, "median": statistics.median(values), "minimum": min(values), "maximum": max(values), "range": max(values) - min(values)}

def analyze(root: pathlib.Path) -> dict[str, Any]:
    verify_checksums(root); manifest = load_json(root / "execution-manifest.json")
    try: protocol.validate_manifest(manifest)
    except protocol.ProtocolError as error: raise AnalysisError(str(error)) from error
    status = load_json(root / "protocol-status.json")
    if (status.get("schema") != protocol.SCHEMA or status.get("phase") != "complete"
            or status.get("completed") is not True or status.get("execution_count") != 6
            or status.get("preflight_passed") is not True or status.get("timed_started") is not True):
        raise AnalysisError("protocol is incomplete")
    validate_negative(load_json(root / "negative-control.json"))
    validate_comparisons(root, manifest, load_json(root / "comparisons.json"))
    expected_arm_names = {record["name"] for record in manifest["executions"]}
    arms_root = root / "arms"
    actual_arm_names = {path.name for path in arms_root.iterdir() if path.is_dir()} if arms_root.is_dir() else set()
    if actual_arm_names != expected_arm_names: raise AnalysisError("missing or extra arm directories")
    provenance = manifest.get("provenance", {})
    if provenance.get("repository_status") not in ("", None) or len(str(provenance.get("repository_commit", ""))) != 40 or any(len(str(provenance.get(field, ""))) != 64 for field in ("binary_sha256", "model_sha256", "state_sha256")):
        raise AnalysisError("repository or artifact identity is malformed/dirty")
    records, timings, metrics = [], {}, {}
    compared_fields = ("id", "name", "class", "path_mode", "selector", "workers", "draws", "ticks", "slots", "seed", "noise", "repetition", "profiled", "included_in_performance", "environment", "environment_unset", "provenance", "argv", "benchmark_argv")
    for expected in manifest["executions"]:
        arm = evidence_path(root, manifest, expected["arm_dir"], expected.get("arm_relative"))
        record = load_json(arm / "record.json")
        if any(record.get(field) != expected.get(field) for field in compared_fields): raise AnalysisError(f"record {expected['id']} identity/argv drifted")
        if record.get("return_code") != 0 or record.get("timed_out") is not False: raise AnalysisError(f"record {expected['id']} failed or timed out")
        for stream_name in ("stdout", "stderr"):
            stream = arm / f"{stream_name}.txt"
            recorded_stream = record.get(stream_name)
            if (not isinstance(recorded_stream, str)
                    or pathlib.Path(recorded_stream).name != stream.name
                    or not stream.is_file()):
                raise AnalysisError(f"record {expected['id']} is missing {stream_name} evidence")
        rss, vram = validate_resources(record); record["peak_rss_bytes"], record["peak_vram_bytes"] = rss, vram
        timing_path = evidence_path(root, manifest, expected["timing_json"], expected.get("timing_relative"))
        try: timing = protocol.validate_timing(timing_path, expected)
        except protocol.ProtocolError as error: raise AnalysisError(str(error)) from error
        timings[expected["id"]], metrics[expected["id"]] = timing, timing_metrics(timing); records.append(record)
    profile = records[-1]; exports = profile.get("nsys_exports")
    expected_exports = {name: (pathlib.Path(profile["arm_relative"]) / "profile" / f"{name}.csv").as_posix() for name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum")}
    if exports != expected_exports: raise AnalysisError("profile exports are missing, extra, or not bound to the profile arm")
    raw_report = evidence_path(root, manifest, profile["nsys_report"], profile.get("nsys_report_relative"))
    if not raw_report.is_file() or raw_report.stat().st_size == 0: raise AnalysisError("raw Nsight profile is missing")
    try:
        nsight = {
            "trace": nsight_support.analyze_nsys_trace(evidence_path(root, manifest, exports["cuda_gpu_trace"]), timings[6], "B"),
            "api": nsight_support.analyze_nsys_api(evidence_path(root, manifest, exports["cuda_api_sum"])),
            "kernel_summary": nsight_support.analyze_nsys_kernel_summary(evidence_path(root, manifest, exports["cuda_gpu_kern_sum"])),
        }
    except nsight_support.AnalysisError as error: raise AnalysisError(str(error)) from error
    timed = records[2:5]
    raw = []
    for record in timed:
        raw.append({"id": record["id"], "repetition": record["repetition"], "peak_rss_bytes": record["peak_rss_bytes"], "peak_vram_bytes": record["peak_vram_bytes"], **metrics[record["id"]]})
    scalar_fields = ("whole_sweep_wall_time_ms", "setup_wall_time_ms", "execution_window_wall_time_ms", "publication_wall_time_ms", "per_draw_wall_time_ms", "final_state_seam_total_ms", "pageable_dtoh_host_api_ms", "cpu_sha256_ms", "attributed_phase_sum_ms", "unattributed_timer_overhead_ms", "peak_rss_bytes", "peak_vram_bytes")
    summaries = {field: summary([item[field] for item in raw]) for field in scalar_fields}
    for field in ("state", "inputs", "input_counts", "total"):
        summaries[f"downloaded_{field}_bytes"] = summary([item["downloaded_bytes"][field] for item in raw])
    return {"schema": SCHEMA, "complete": True, "protocol_schema": manifest["schema"], "provenance": provenance,
            "interpretation": {"performance_threshold": None, "optimization_authorized": False,
                "historical_context": "Prior 2026-07-31 H100 B results are historical, non-binding, and not paired with this session."},
            "timed_repetitions": raw, "absolute_metrics": summaries, "profile": nsight,
            "residual_risks": ["Cross-session comparisons are contextual because commit, host load, image, driver, and Nsight versions can differ.", "Nsight CSV attribution is tool-version-specific and fails closed."]}

def markdown(result: dict[str, Any]) -> str:
    lines = ["# CUDA current-path rebaseline", "", "Evidence status: **complete and interpretable**.", "",
             "This report has no performance threshold and does not authorize another optimization. Prior 2026-07-31 H100 B values are historical and non-binding.", "", "## Timed repetitions", "",
             "| Rep | whole wall ms | setup ms | execution ms | publication ms | per-draw wall ms | seam ms | pageable D2H ms | CPU SHA-256 ms | attributed ms | unattributed ms | peak RSS bytes | peak VRAM bytes |",
             "|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    fields = ("whole_sweep_wall_time_ms", "setup_wall_time_ms", "execution_window_wall_time_ms", "publication_wall_time_ms", "per_draw_wall_time_ms", "final_state_seam_total_ms", "pageable_dtoh_host_api_ms", "cpu_sha256_ms", "attributed_phase_sum_ms", "unattributed_timer_overhead_ms")
    for item in result["timed_repetitions"]:
        lines.append("| " + str(item["repetition"]) + " | " + " | ".join(f"{item[f]:.3f}" for f in fields) + f" | {item['peak_rss_bytes']} | {item['peak_vram_bytes']} |")
    lines += ["", "## Median/minimum/maximum/range", "", "| Metric | Raw | Median | Minimum | Maximum | Range |", "|---|---|---:|---:|---:|---:|"]
    for name, values in result["absolute_metrics"].items():
        lines.append(f"| {name} | {values['raw']} | {values['median']:.3f} | {values['minimum']:.3f} | {values['maximum']:.3f} | {values['range']:.3f} |")
    trace = result["profile"]["trace"]
    lines += ["", "## Nsight Systems evidence", "", "| large-copy count | bytes | summed duration ms | copy-union ms | copy/kernel overlap ms | exposed D2H ms |", "|---:|---:|---:|---:|---:|---:|",
              f"| {trace['large_copy_count']} | {trace['large_copy_bytes']} | {trace['large_copy_summed_duration_ms']:.3f} | {trace['copy_union_duration_ms']:.3f} | {trace['copy_kernel_overlap_ms']:.3f} | {trace['exposed_final_state_dtoh_ms']:.3f} |", "", "Relevant CUDA API rows:", ""]
    lines.extend(f"- `{row['name']}`: {row['total_time_ms']:.3f} ms" for row in result["profile"]["api"]["selected_rows"])
    lines += ["", "## Residual risks", ""] + [f"- {risk}" for risk in result["residual_risks"]]
    return "\n".join(lines) + "\n"

def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(); parser.add_argument("evidence", type=pathlib.Path); parser.add_argument("--json", type=pathlib.Path); parser.add_argument("--markdown", type=pathlib.Path); return parser.parse_args(argv)
def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv); result = analyze(args.evidence.resolve())
    protocol.atomic_json(args.json or args.evidence / "current-rebaseline.json", result)
    (args.markdown or args.evidence / "current-rebaseline.md").write_text(markdown(result)); return 0
if __name__ == "__main__":
    try: raise SystemExit(main())
    except AnalysisError as error: print(f"CUDA current-path rebaseline analysis failed: {error}", file=sys.stderr); raise SystemExit(1)

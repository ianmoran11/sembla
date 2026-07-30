#!/usr/bin/env python3
"""Validate focused CUDA final-state evidence and emit the frozen decision gate."""

from __future__ import annotations

import argparse
import csv
import importlib.util
import json
import math
import pathlib
import statistics
import sys
from typing import Any, Iterable

HERE = pathlib.Path(__file__).resolve().parent
_PROTOCOL_SPEC = importlib.util.spec_from_file_location(
    "cuda_final_state_protocol", HERE / "run-cuda-final-state-decision.py"
)
if _PROTOCOL_SPEC is None or _PROTOCOL_SPEC.loader is None:
    raise RuntimeError("could not load focused protocol support")
protocol = importlib.util.module_from_spec(_PROTOCOL_SPEC)
_PROTOCOL_SPEC.loader.exec_module(protocol)

SCHEMA = "sembla-cuda-final-state-decision-v1"
MIB = 1024 * 1024
THRESHOLDS = {
    "B_workers4_wall": {
        "numerator": "B",
        "denominator": "A",
        "aggregate": "median_of_three_adjacent_within_repetition_ratios",
        "direction": "<=",
        "value": 0.95,
        "workers": 4,
        "binding": True,
    },
    "B_workers1_wall": {
        "numerator": "B",
        "denominator": "A",
        "aggregate": "median_of_three_adjacent_within_repetition_ratios",
        "direction": "<=",
        "value": 1.02,
        "workers": 1,
        "binding": False,
    },
    "C_workers4_wall": {
        "numerator": "C",
        "denominator": "B",
        "aggregate": "median_of_three_adjacent_within_repetition_ratios",
        "direction": "<=",
        "value": 0.95,
        "workers": 4,
        "binding": True,
    },
    "C_workers1_wall": {
        "numerator": "C",
        "denominator": "B",
        "aggregate": "median_of_three_adjacent_within_repetition_ratios",
        "direction": "<=",
        "value": 1.02,
        "workers": 1,
        "binding": False,
    },
    "C_host_mechanism": {
        "numerator": "C_host_blocking_ms=pinned_dtoh_enqueue_api_ms+wait_to_pinned_host_readable_ms",
        "denominator": "B_host_blocking_ms=pageable_dtoh_host_api_ms",
        "aggregate": "median_of_three_adjacent_workers4_within_repetition_ratios",
        "direction": "<=",
        "value": 0.90,
        "workers": 4,
        "binding": True,
    },
    "C_nsight_mechanism": {
        "numerator": "C_exposed_final_state_dtoh_ms",
        "denominator": "B_exposed_final_state_dtoh_ms",
        "aggregate": "single_post_matrix_profile_ratio",
        "direction": "<=",
        "value": 0.90,
        "workers": 4,
        "binding": True,
    },
    "C_mechanism_OR": {
        "operator": "OR",
        "operands": ["C_host_mechanism", "C_nsight_mechanism"],
        "binding": True,
    },
}


class AnalysisError(RuntimeError):
    pass


def finite(value: Any, label: str, *, positive: bool = False) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise AnalysisError(f"{label} must be numeric")
    result = float(value)
    if not math.isfinite(result) or result < 0 or (positive and result <= 0):
        raise AnalysisError(f"{label} must be finite and {'positive' if positive else 'non-negative'}")
    return result


def load_json(path: pathlib.Path) -> Any:
    try:
        return json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise AnalysisError(f"invalid JSON {path}: {error}") from error


def evidence_path(
    root: pathlib.Path,
    manifest: dict[str, Any],
    value: str | None,
    relative: str | None = None,
) -> pathlib.Path:
    if relative:
        return root / relative
    if not value:
        raise AnalysisError("evidence path is missing")
    path = pathlib.Path(value)
    if not path.is_absolute():
        return root / path
    collection_root = pathlib.Path(manifest.get("evidence_root_at_collection", ""))
    try:
        return root / path.relative_to(collection_root)
    except (ValueError, TypeError):
        if path.exists():
            return path
        raise AnalysisError(f"absolute evidence path is outside the recorded root: {path}")


def verify_checksums(root: pathlib.Path) -> None:
    manifest = root / "SHA256SUMS"
    try:
        lines = [line for line in manifest.read_text().splitlines() if line]
    except OSError as error:
        raise AnalysisError(f"focused checksum manifest is missing: {error}") from error
    expected: dict[str, str] = {}
    for line in lines:
        try:
            digest, relative = line.split(maxsplit=1)
        except ValueError as error:
            raise AnalysisError("malformed focused checksum manifest") from error
        relative = relative.removeprefix("*").removeprefix("./")
        if len(digest) != 64 or relative in expected:
            raise AnalysisError("malformed or duplicate focused checksum entry")
        expected[relative] = digest
    actual_names = {
        path.relative_to(root).as_posix()
        for path in root.rglob("*")
        if path.is_file() and path.name != "SHA256SUMS"
    }
    if set(expected) != actual_names:
        raise AnalysisError("focused checksum file set is incomplete or has extras")
    for relative, digest in expected.items():
        if protocol.sha256_file(root / relative) != digest:
            raise AnalysisError(f"focused checksum mismatch: {relative}")


def _unit_factor(header: str, *, bytes_value: bool = False) -> float:
    lower = header.lower()
    if bytes_value:
        if "gib" in lower:
            return 1024.0**3
        if "gb" in lower:
            return 1_000_000_000.0
        if "mib" in lower:
            return 1024.0**2
        if "mb" in lower:
            return 1_000_000.0
        if "kib" in lower:
            return 1024.0
        if "kb" in lower:
            return 1_000.0
        return 1.0
    if "(ns" in lower or lower.endswith(" ns"):
        return 1e-6
    if "(us" in lower or "µs" in lower or lower.endswith(" us"):
        return 1e-3
    if "(s" in lower and "ms" not in lower:
        return 1000.0
    return 1.0


def _header_and_rows(path: pathlib.Path, required_terms: tuple[str, ...]) -> tuple[list[str], list[dict[str, str]]]:
    try:
        raw = list(csv.reader(path.read_text().splitlines()))
    except OSError as error:
        raise AnalysisError(f"missing Nsight CSV {path}: {error}") from error
    header_index = None
    for index, row in enumerate(raw):
        lowered = [cell.strip().lower() for cell in row]
        if all(any(term in cell for cell in lowered) for term in required_terms):
            header_index = index
            break
    if header_index is None:
        raise AnalysisError(f"could not locate required Nsight header in {path}")
    header = [cell.strip() for cell in raw[header_index]]
    rows = [
        dict(zip(header, row))
        for row in raw[header_index + 1 :]
        if row and any(cell.strip() for cell in row)
    ]
    return header, rows


def _column(header: Iterable[str], *terms: str) -> str:
    matches = [name for name in header if all(term in name.lower() for term in terms)]
    if len(matches) != 1:
        raise AnalysisError(f"expected one Nsight column containing {terms}, found {matches}")
    return matches[0]


def _column_alias(header: Iterable[str], aliases: tuple[str, ...]) -> str:
    matches = [name for name in header if name.lower() in aliases or any(alias in name.lower() for alias in aliases)]
    if len(matches) != 1:
        raise AnalysisError(f"expected one Nsight column matching {aliases}, found {matches}")
    return matches[0]


def analyze_nsys_kernel_summary(path: pathlib.Path) -> dict[str, Any]:
    header, rows = _header_and_rows(path, ("name", "total", "time"))
    name_col = _column(header, "name")
    time_col = _column(header, "total", "time")
    calls_candidates = [
        name
        for name in header
        if "instance" in name.lower() or "calls" in name.lower()
    ]
    if len(calls_candidates) != 1:
        raise AnalysisError(f"expected one kernel instance column in {path}")
    calls_col = calls_candidates[0]
    factor = _unit_factor(time_col)
    kernels: list[dict[str, Any]] = []
    for row in rows:
        name = row.get(name_col, "").strip()
        if not name:
            continue
        try:
            total_ms = (
                finite(float(row[time_col]), f"{path}:kernel time", positive=True)
                * factor
            )
            instances = int(float(row[calls_col]))
        except (KeyError, ValueError) as error:
            raise AnalysisError(
                f"malformed Nsight kernel summary row in {path}: {row}"
            ) from error
        if instances <= 0:
            raise AnalysisError(f"non-positive kernel instance count in {path}: {row}")
        kernels.append(
            {"name": name, "total_time_ms": total_ms, "instances": instances}
        )
    if not kernels:
        raise AnalysisError(f"Nsight kernel summary contains no kernels: {path}")
    return {
        "kernel_count": len(kernels),
        "total_instances": sum(item["instances"] for item in kernels),
        "total_kernel_time_ms": sum(item["total_time_ms"] for item in kernels),
        "kernels": kernels,
    }


def merge_intervals(intervals: list[tuple[float, float]]) -> list[tuple[float, float]]:
    merged: list[list[float]] = []
    for start, end in sorted(intervals):
        if end <= start:
            raise AnalysisError("Nsight interval duration must be positive")
        if not merged or start > merged[-1][1]:
            merged.append([start, end])
        else:
            merged[-1][1] = max(merged[-1][1], end)
    return [(start, end) for start, end in merged]


def interval_duration(intervals: list[tuple[float, float]]) -> float:
    return sum(end - start for start, end in merge_intervals(intervals))


def interval_overlap(left: list[tuple[float, float]], right: list[tuple[float, float]]) -> float:
    total = 0.0
    for a0, a1 in merge_intervals(left):
        for b0, b1 in merge_intervals(right):
            total += max(0.0, min(a1, b1) - max(a0, b0))
    return total


def expected_final_state_copy_sizes(timing: dict[str, Any]) -> list[int]:
    sizes: list[int] = []
    for draw in timing["draw_timings"]:
        downloaded = draw["final_state"]["downloaded_bytes"]
        for name in ("state", "inputs", "input_counts"):
            size = downloaded[name]
            if size >= MIB:
                sizes.append(size)
    if not sizes:
        raise AnalysisError("profile timing exposes no >=1 MiB final-state component")
    return sorted(sizes)


def analyze_nsys_trace(path: pathlib.Path, timing: dict[str, Any], mode: str) -> dict[str, Any]:
    header, rows = _header_and_rows(path, ("start", "duration", "name", "bytes"))
    start_col = _column(header, "start")
    duration_col = _column(header, "duration")
    name_col = _column(header, "name")
    bytes_col = _column(header, "bytes")
    src_col = _column(header, "src")
    dst_col = _column(header, "dst")
    context_col = _column_alias(header, ("context", "ctx"))
    stream_col = _column_alias(header, ("stream", "strm"))
    start_factor = _unit_factor(start_col)
    duration_factor = _unit_factor(duration_col)
    bytes_factor = _unit_factor(bytes_col, bytes_value=True)

    copies: list[dict[str, Any]] = []
    kernels: list[tuple[float, float]] = []
    unmatched_tiny: list[dict[str, Any]] = []
    for row in rows:
        name = row.get(name_col, "")
        try:
            start = finite(float(row[start_col]), f"{path}:start") * start_factor
            duration = finite(
                float(row[duration_col]), f"{path}:duration", positive=True
            ) * duration_factor
        except (KeyError, ValueError) as error:
            raise AnalysisError(f"malformed relevant Nsight row in {path}: {row}") from error
        end = start + duration
        lowered = name.lower()
        if "dtoh" in lowered or ("memcpy" in lowered and "device" in row.get(src_col, "").lower()):
            try:
                size = int(
                    round(
                        finite(float(row[bytes_col]), f"{path}:bytes", positive=True)
                        * bytes_factor
                    )
                )
            except (KeyError, ValueError) as error:
                raise AnalysisError(f"malformed D2H byte value in {path}: {row}") from error
            item = {
                "start_ms": start,
                "end_ms": end,
                "duration_ms": duration,
                "bytes": size,
                "name": name,
                "source": row.get(src_col, ""),
                "destination": row.get(dst_col, ""),
                "context": row.get(context_col, ""),
                "stream": row.get(stream_col, ""),
            }
            if size < MIB:
                unmatched_tiny.append(item)
            else:
                copies.append(item)
        elif "kernel" in lowered or ("memcpy" not in lowered and "memset" not in lowered):
            kernels.append((start, end))

    expected = expected_final_state_copy_sizes(timing)
    remaining = copies.copy()
    matched: list[dict[str, Any]] = []
    for size in expected:
        tolerance = max(4096, int(size * 0.0001))
        candidates = [item for item in remaining if abs(item["bytes"] - size) <= tolerance]
        if not candidates:
            raise AnalysisError(f"missing final-state D2H copy near {size} bytes in {path}")
        chosen = min(candidates, key=lambda item: abs(item["bytes"] - size))
        remaining.remove(chosen)
        matched.append(chosen)
    if remaining:
        raise AnalysisError(f"unmatched >=1 MiB D2H rows remain in {path}: {len(remaining)}")
    destinations = " ".join(item["destination"].lower() for item in matched)
    if mode in ("A", "B") and "page" not in destinations:
        raise AnalysisError(f"{mode} Nsight copies do not identify pageable destinations")
    if mode == "C" and "pin" not in destinations:
        raise AnalysisError("C Nsight copies do not identify pinned/page-locked destinations")
    copy_intervals = [(item["start_ms"], item["end_ms"]) for item in matched]
    copy_union = interval_duration(copy_intervals)
    overlap = interval_overlap(copy_intervals, kernels)
    return {
        "matched_final_state_copies": matched,
        "unmatched_tiny_dtoh": unmatched_tiny,
        "large_copy_count": len(matched),
        "large_copy_bytes": sum(item["bytes"] for item in matched),
        "large_copy_summed_duration_ms": sum(item["duration_ms"] for item in matched),
        "copy_union_duration_ms": copy_union,
        "copy_kernel_overlap_ms": overlap,
        "exposed_final_state_dtoh_ms": copy_union - overlap,
        "destination_kinds": sorted({item["destination"] for item in matched}),
        "streams": sorted({item["stream"] for item in matched}),
        "contexts": sorted({item["context"] for item in matched}),
        "kernel_union_duration_ms": interval_duration(kernels) if kernels else 0.0,
    }


def analyze_nsys_api(path: pathlib.Path) -> dict[str, Any]:
    header, rows = _header_and_rows(path, ("name", "time"))
    name_col = _column(header, "name")
    time_candidates = [name for name in header if "time" in name.lower() and "%" not in name]
    if not time_candidates:
        raise AnalysisError(f"Nsight API CSV has no total-time column: {path}")
    time_col = time_candidates[0]
    calls_col = next((name for name in header if "call" in name.lower()), None)
    factor = _unit_factor(time_col)
    selected = []
    for row in rows:
        name = row.get(name_col, "")
        if any(term in name for term in ("Memcpy", "Synchronize", "Event")):
            try:
                selected.append(
                    {
                        "name": name,
                        "total_time_ms": finite(
                            float(row[time_col]), f"{path}:api time"
                        )
                        * factor,
                        "calls": (
                            int(float(row[calls_col]))
                            if calls_col and row.get(calls_col)
                            else None
                        ),
                    }
                )
            except (KeyError, ValueError) as error:
                raise AnalysisError(f"malformed Nsight API row in {path}: {row}") from error
    if not selected:
        raise AnalysisError(f"Nsight API CSV has no memcpy/synchronization rows: {path}")
    return {"selected_rows": selected}


def _timing_metrics(timing: dict[str, Any]) -> dict[str, Any]:
    finals = [draw["final_state"] for draw in timing["draw_timings"]]
    def total(field: str) -> float:
        return sum(float(final.get(field) or 0.0) for final in finals)
    return {
        "whole_sweep_wall_time_ms": finite(timing["whole_sweep_wall_time_ms"], "whole sweep wall", positive=True),
        "setup_wall_time_ms": finite(timing["setup_wall_time_ms"], "setup wall"),
        "execution_window_wall_time_ms": finite(timing.get("execution_window_wall_time_ms", sum(draw["wall_time_ms"] for draw in timing["draw_timings"])), "execution wall"),
        "publication_wall_time_ms": finite(timing.get("publication_wall_time_ms", 0.0), "publication wall"),
        "final_state_seam_total_ms": total("final_state_seam_total_ms"),
        "pageable_dtoh_host_api_ms": total("pageable_dtoh_host_api_ms"),
        "pinned_dtoh_enqueue_api_ms": total("pinned_dtoh_enqueue_api_ms"),
        "wait_to_pinned_host_readable_ms": total("wait_to_pinned_host_readable_ms"),
        "pinned_to_cacheable_staging_copy_ms": total("pinned_to_cacheable_staging_copy_ms"),
        "host_state_reconstruction_ms": total("host_state_reconstruction_ms"),
        "cpu_sha256_ms": total("cpu_sha256_ms"),
        "buffer_accounting": timing["final_state_buffer_accounting"],
    }


def _threshold(name: str, actual: float) -> dict[str, Any]:
    definition = dict(THRESHOLDS[name])
    definition.update({"actual": actual, "passed": actual <= definition["value"]})
    return definition


def evaluate_thresholds(
    ratios: dict[str, float], host_ratio: float, nsight_ratio: float
) -> dict[str, Any]:
    results = {
        "B_workers4_wall": _threshold("B_workers4_wall", ratios["B_A_workers4"]),
        "B_workers1_wall": _threshold("B_workers1_wall", ratios["B_A_workers1"]),
        "C_workers4_wall": _threshold("C_workers4_wall", ratios["C_B_workers4"]),
        "C_workers1_wall": _threshold("C_workers1_wall", ratios["C_B_workers1"]),
        "C_host_mechanism": _threshold("C_host_mechanism", host_ratio),
        "C_nsight_mechanism": _threshold("C_nsight_mechanism", nsight_ratio),
    }
    mechanism_passed = results["C_host_mechanism"]["passed"] or results["C_nsight_mechanism"]["passed"]
    results["C_mechanism_OR"] = {
        **THRESHOLDS["C_mechanism_OR"],
        "passed": mechanism_passed,
        "operand_results": {
            "C_host_mechanism": results["C_host_mechanism"]["passed"],
            "C_nsight_mechanism": results["C_nsight_mechanism"]["passed"],
        },
    }
    return results


def _records_by_key(records: list[dict[str, Any]]) -> dict[tuple[Any, ...], dict[str, Any]]:
    return {
        (record["class"], record["workers"], record["repetition"], record["mode"]): record
        for record in records
    }


def _ratios(records: dict[tuple[Any, ...], dict[str, Any]], metrics: dict[int, dict[str, Any]], workers: int, numerator: str, denominator: str, field: str) -> dict[str, Any]:
    raw = []
    for repetition in (1, 2, 3):
        num_record = records[("timed", workers, repetition, numerator)]
        den_record = records[("timed", workers, repetition, denominator)]
        num = metrics[num_record["id"]][field]
        den = metrics[den_record["id"]][field]
        if den <= 0:
            raise AnalysisError(f"zero denominator for {numerator}/{denominator} workers {workers}")
        raw.append(
            {
                "repetition": repetition,
                "numerator_record_id": num_record["id"],
                "denominator_record_id": den_record["id"],
                "numerator_ms": num,
                "denominator_ms": den,
                "ratio": num / den,
            }
        )
    return {"raw": raw, "median_ratio": statistics.median(item["ratio"] for item in raw)}


def analyze(root: pathlib.Path) -> dict[str, Any]:
    verify_checksums(root)
    manifest = load_json(root / "execution-manifest.json")
    try:
        protocol.validate_manifest(manifest)
    except protocol.ProtocolError as error:
        raise AnalysisError(str(error)) from error
    status = load_json(root / "protocol-status.json")
    if status.get("completed") is not True or status.get("execution_count") != 27:
        raise AnalysisError("focused protocol is incomplete")
    negative = load_json(root / "negative-control.json")
    if negative.get("accepted") is not False or negative.get("rejected") is not True:
        raise AnalysisError("deliberate negative control was not rejected")
    comparisons = load_json(root / "comparisons.json")
    if len(comparisons) != 9 or any(item.get("passed") is not True for item in comparisons):
        raise AnalysisError("independent/CRN/profile output parity evidence is incomplete")

    records: list[dict[str, Any]] = []
    metrics: dict[int, dict[str, Any]] = {}
    provenance = manifest.get("provenance")
    if not isinstance(provenance, dict) or not provenance.get("repository_commit"):
        raise AnalysisError("protocol provenance is incomplete")
    if len(str(provenance["repository_commit"])) != 40 or any(
        len(str(provenance.get(field, ""))) != 64
        for field in ("binary_sha256", "model_sha256", "state_sha256")
    ):
        raise AnalysisError("commit or binary/model/state provenance digest is malformed")
    if provenance.get("repository_status") not in ("", None):
        raise AnalysisError("focused evidence checkout was dirty")
    for expected in manifest["executions"]:
        record = load_json(
            evidence_path(root, manifest, expected["arm_dir"], expected.get("arm_relative"))
            / "record.json"
        )
        for field in (
            "id", "class", "mode", "selector", "workers", "draws", "ticks", "seed",
            "noise", "repetition", "profiled", "included_in_performance", "argv", "environment",
            "provenance",
        ):
            if record.get(field) != expected.get(field):
                raise AnalysisError(f"record {expected['id']} changed command field {field}")
        if record.get("return_code") != 0 or record.get("timed_out") is not False:
            raise AnalysisError(f"record {expected['id']} did not complete successfully")
        samples = record.get("resource_samples")
        if (
            record.get("resource_sampling_complete") is not True
            or not isinstance(samples, list)
            or not samples
            or int(record.get("peak_rss_bytes", 0)) <= 0
            or int(record.get("peak_vram_bytes", 0)) <= 0
        ):
            raise AnalysisError(f"record {expected['id']} has incomplete resource evidence")
        if any(
            not isinstance(sample, dict)
            or not isinstance(sample.get("rss_bytes"), int)
            or not isinstance(sample.get("vram_bytes"), int)
            or sample["rss_bytes"] < 0
            or sample["vram_bytes"] < 0
            for sample in samples
        ):
            raise AnalysisError(f"record {expected['id']} has malformed resource samples")
        if expected["profiled"]:
            raw_report = evidence_path(
                root,
                manifest,
                expected.get("nsys_report"),
                expected.get("nsys_report_relative"),
            )
            if not raw_report.is_file() or raw_report.stat().st_size == 0:
                raise AnalysisError(f"profile {expected['id']} is missing its raw Nsight report")
        try:
            timing = protocol.validate_timing(
                evidence_path(
                    root, manifest, expected["timing_json"], expected.get("timing_relative")
                ),
                expected,
            )
        except protocol.ProtocolError as error:
            raise AnalysisError(str(error)) from error
        metrics[expected["id"]] = _timing_metrics(timing)
        records.append(record)

    keyed = _records_by_key(records)
    ratio_sets = {
        "B_A_workers1": _ratios(keyed, metrics, 1, "B", "A", "whole_sweep_wall_time_ms"),
        "B_A_workers4": _ratios(keyed, metrics, 4, "B", "A", "whole_sweep_wall_time_ms"),
        "C_B_workers1": _ratios(keyed, metrics, 1, "C", "B", "whole_sweep_wall_time_ms"),
        "C_B_workers4": _ratios(keyed, metrics, 4, "C", "B", "whole_sweep_wall_time_ms"),
    }
    host_ratios = _ratios(keyed, metrics, 4, "C", "B", "pageable_dtoh_host_api_ms")
    # Replace the C numerator with the exact pinned host-blocking definition.
    for item in host_ratios["raw"]:
        c_record = keyed[("timed", 4, item["repetition"], "C")]
        c_metrics = metrics[c_record["id"]]
        numerator = c_metrics["pinned_dtoh_enqueue_api_ms"] + c_metrics["wait_to_pinned_host_readable_ms"]
        item["numerator_ms"] = numerator
        item["ratio"] = numerator / item["denominator_ms"]
    host_ratios["median_ratio"] = statistics.median(item["ratio"] for item in host_ratios["raw"])

    profiles: dict[str, Any] = {}
    for mode in "ABC":
        record = keyed[("profile", 4, 1, mode)]
        exports = record.get("nsys_exports")
        if not isinstance(exports, dict):
            raise AnalysisError(f"profile {mode} is missing Nsight exports")
        timing = load_json(
            evidence_path(
                root, manifest, record["timing_json"], record.get("timing_relative")
            )
        )
        profiles[mode] = {
            "trace": analyze_nsys_trace(
                evidence_path(root, manifest, exports["cuda_gpu_trace"]), timing, mode
            ),
            "api": analyze_nsys_api(
                evidence_path(root, manifest, exports["cuda_api_sum"])
            ),
            "kernel_summary": analyze_nsys_kernel_summary(
                evidence_path(root, manifest, exports.get("cuda_gpu_kern_sum"))
            ),
        }
    b_exposed = profiles["B"]["trace"]["exposed_final_state_dtoh_ms"]
    c_exposed = profiles["C"]["trace"]["exposed_final_state_dtoh_ms"]
    if b_exposed <= 0:
        raise AnalysisError("B exposed final-state D2H denominator is zero")
    nsight_ratio = c_exposed / b_exposed

    threshold_results = evaluate_thresholds(
        {
            "B_A_workers4": ratio_sets["B_A_workers4"]["median_ratio"],
            "B_A_workers1": ratio_sets["B_A_workers1"]["median_ratio"],
            "C_B_workers4": ratio_sets["C_B_workers4"]["median_ratio"],
            "C_B_workers1": ratio_sets["C_B_workers1"]["median_ratio"],
        },
        host_ratios["median_ratio"],
        nsight_ratio,
    )
    mechanism_passed = threshold_results["C_mechanism_OR"]["passed"]

    c_resources_bounded = True
    resources_by_workers: dict[str, Any] = {}
    h100_vram_bytes = 80 * 1_000_000_000
    h100_host_bytes = 177 * 1024**3
    for workers in (1, 4):
        relevant = [
            record
            for record in records
            if record["class"] == "timed" and record["workers"] == workers
        ]
        by_mode = {
            mode: [
                record
                for record in relevant
                if record["mode"] == mode
            ]
            for mode in "ABC"
        }
        peak_rss_by_mode = {
            mode: max(int(record.get("peak_rss_bytes", 0)) for record in mode_records)
            for mode, mode_records in by_mode.items()
        }
        peak_vram_by_mode = {
            mode: max(int(record.get("peak_vram_bytes", 0)) for record in mode_records)
            for mode, mode_records in by_mode.items()
        }
        accounting_by_mode = {
            mode: [metrics[record["id"]]["buffer_accounting"] for record in mode_records]
            for mode, mode_records in by_mode.items()
        }
        c_accounting = accounting_by_mode["C"]
        requested_pinned = max(item["requested_pinned_bytes"] for item in c_accounting)
        requested_staging = max(
            item["requested_cacheable_staging_bytes"] for item in c_accounting
        )
        treatment_admission = requested_pinned + requested_staging
        incremental_rss = max(
            0,
            peak_rss_by_mode["C"] - peak_rss_by_mode["B"],
        )
        incremental_bound = treatment_admission + max(
            treatment_admission // 20, 64 * 1024 * 1024
        )
        for accounting in c_accounting:
            retained = accounting["retained_lane_count"]
            if (
                accounting["buffer_set_count"] > retained
                or accounting["underlying_pinned_allocation_count"] > 3 * retained
            ):
                c_resources_bounded = False
            if (
                accounting["effective_pinned_bytes"] <= 0
                or accounting["effective_cacheable_staging_bytes"] <= 0
            ):
                c_resources_bounded = False
        memory_passed = (
            treatment_admission <= h100_host_bytes
            and incremental_rss <= incremental_bound
            and max(peak_rss_by_mode.values()) <= h100_host_bytes
            and max(peak_vram_by_mode.values()) <= h100_vram_bytes
        )
        c_resources_bounded = c_resources_bounded and memory_passed
        resources_by_workers[str(workers)] = {
            "peak_rss_bytes_by_mode": peak_rss_by_mode,
            "peak_vram_bytes_by_mode": peak_vram_by_mode,
            "requested_pinned_bytes": requested_pinned,
            "requested_staging_bytes": requested_staging,
            "treatment_admission_bytes": treatment_admission,
            "incremental_c_over_b_rss_bytes": incremental_rss,
            "incremental_rss_bound_bytes": incremental_bound,
            "h100_host_headroom_bytes": h100_host_bytes,
            "h100_vram_headroom_bytes": h100_vram_bytes,
            "memory_passed": memory_passed,
            "buffer_sets_per_retained_lane": [
                item["buffer_set_count"] / item["retained_lane_count"]
                for item in c_accounting
            ],
            "pinned_allocations_per_retained_lane": [
                item["underlying_pinned_allocation_count"]
                / item["retained_lane_count"]
                for item in c_accounting
            ],
            "modes": accounting_by_mode,
        }
    if not c_resources_bounded:
        raise AnalysisError("C resource accounting or measured host/device memory is unbounded")

    b_eligible = threshold_results["B_workers4_wall"]["passed"] and threshold_results["B_workers1_wall"]["passed"]
    c_eligible = (
        threshold_results["C_workers4_wall"]["passed"]
        and threshold_results["C_workers1_wall"]["passed"]
        and mechanism_passed
        and c_resources_bounded
    )
    result = {
        "schema": SCHEMA,
        "complete": True,
        "protocol_schema": manifest["schema"],
        "provenance": provenance,
        "thresholds": threshold_results,
        "ratios": ratio_sets,
        "host_mechanism_ratios": host_ratios,
        "nsight_mechanism": {
            "B_exposed_final_state_dtoh_ms": b_exposed,
            "C_exposed_final_state_dtoh_ms": c_exposed,
            "ratio": nsight_ratio,
        },
        "absolute_command_metrics": {str(key): value for key, value in sorted(metrics.items())},
        "profiles": profiles,
        "resources_by_workers": resources_by_workers,
        "eligibility": {
            "B_packed_pageable": b_eligible,
            "C_packed_pinned": c_eligible,
            "promotion_authorized": False,
            "verdict": "GO-ELIGIBLE-FOR-LATER-PRD" if (b_eligible or c_eligible) else "NO-GO",
        },
        "residual_risks": [
            "Nsight export labels and byte attribution are tool-version-specific and fail closed.",
            "A promotion still requires a separate reviewed PRD; this report never changes defaults.",
            "Provider-console billing and zero-resource teardown confirmation remain operator checks.",
        ],
    }
    return result


def markdown(result: dict[str, Any]) -> str:
    lines = [
        "# CUDA final-state A/B/C decision",
        "",
        f"Verdict: **{result['eligibility']['verdict']}**",
        "",
        "## Frozen thresholds",
        "",
        "| Gate | Actual | Requirement | Pass |",
        "|---|---:|---:|:---:|",
    ]
    for name, gate in result["thresholds"].items():
        if name == "C_mechanism_OR":
            lines.append(f"| {name} | OR | host or Nsight | {'yes' if gate['passed'] else 'no'} |")
        else:
            lines.append(
                f"| {name} | {gate['actual']:.6f} | {gate['direction']} {gate['value']:.2f} | {'yes' if gate['passed'] else 'no'} |"
            )
    lines += ["", "## Raw adjacent repetitions", ""]
    for name, values in result["ratios"].items():
        ratios = ", ".join(f"{item['ratio']:.6f}" for item in values["raw"])
        lines.append(f"- `{name}`: {ratios}; median **{values['median_ratio']:.6f}**")
    lines += [
        "",
        "## Eligibility",
        "",
        f"- B packed-pageable: **{result['eligibility']['B_packed_pageable']}**",
        f"- C packed-pinned: **{result['eligibility']['C_packed_pinned']}**",
        "- Production promotion authorized here: **false**",
        "",
        "## Residual risks",
        "",
    ]
    lines.extend(f"- {risk}" for risk in result["residual_risks"])
    return "\n".join(lines) + "\n"


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence", type=pathlib.Path)
    parser.add_argument("--json", type=pathlib.Path)
    parser.add_argument("--markdown", type=pathlib.Path)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    root = args.evidence.resolve()
    result = analyze(root)
    json_path = args.json or root / "decision.json"
    markdown_path = args.markdown or root / "decision.md"
    protocol.atomic_json(json_path, result)
    markdown_path.write_text(markdown(result))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except AnalysisError as error:
        print(f"final-state decision analysis failed: {error}", file=sys.stderr)
        raise SystemExit(1)

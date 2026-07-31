#!/usr/bin/env python3
"""Validate focused CUDA final-state evidence and emit the frozen decision gate."""

from __future__ import annotations

import argparse
import csv
import importlib.util
import json
import math
import pathlib
import re
import sys
from decimal import Decimal, InvalidOperation, localcontext
from fractions import Fraction
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


def decimal_value(value: Any, label: str, *, positive: bool = False) -> Decimal:
    if isinstance(value, bool) or not isinstance(value, (int, float, Decimal, str)):
        raise AnalysisError(f"{label} must be decimal-compatible")
    try:
        result = Decimal(str(value))
    except InvalidOperation as error:
        raise AnalysisError(f"{label} is not a decimal value") from error
    if not result.is_finite() or result < 0 or (positive and result <= 0):
        raise AnalysisError(
            f"{label} must be finite and {'positive' if positive else 'non-negative'}"
        )
    return result


def exact_ratio(numerator: Any, denominator: Any, label: str) -> Fraction:
    numerator_fraction = (
        numerator
        if isinstance(numerator, Fraction)
        else Fraction(decimal_value(numerator, f"{label} numerator"))
    )
    denominator_fraction = (
        denominator
        if isinstance(denominator, Fraction)
        else Fraction(decimal_value(denominator, f"{label} denominator", positive=True))
    )
    if denominator_fraction <= 0:
        raise AnalysisError(f"{label} denominator must be positive")
    if numerator_fraction < 0:
        raise AnalysisError(f"{label} numerator must be non-negative")
    return numerator_fraction / denominator_fraction


def ratio_decimal_text(value: Fraction) -> str:
    digits = max(
        len(str(abs(value.numerator))),
        len(str(value.denominator)),
    )
    with localcontext() as context:
        context.prec = max(50, digits * 2 + 10)
        return format(Decimal(value.numerator) / Decimal(value.denominator), "f")


def median_decimal(values: Iterable[Fraction]) -> Fraction:
    ordered = sorted(values)
    if not ordered or len(ordered) % 2 == 0:
        raise AnalysisError("a non-empty odd number of ratios is required")
    return ordered[len(ordered) // 2]


def load_json(path: pathlib.Path, *, exact_decimals: bool = False) -> Any:
    try:
        options = {"parse_float": Decimal} if exact_decimals else {}
        return json.loads(path.read_text(), **options)
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
    names = list(header)
    exact = [name for name in names if name.lower() in aliases]
    if len(exact) == 1:
        return exact[0]
    if len(exact) > 1:
        raise AnalysisError(
            f"expected one exact Nsight column matching {aliases}, found {exact}"
        )
    matches = [
        name for name in names if any(alias in name.lower() for alias in aliases)
    ]
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


def merge_intervals(
    intervals: list[tuple[Fraction, Fraction]],
) -> list[tuple[Fraction, Fraction]]:
    merged: list[list[Fraction]] = []
    for start, end in sorted(intervals):
        if end <= start:
            raise AnalysisError("Nsight interval duration must be positive")
        if not merged or start > merged[-1][1]:
            merged.append([start, end])
        else:
            merged[-1][1] = max(merged[-1][1], end)
    return [(start, end) for start, end in merged]


def interval_duration(intervals: list[tuple[Fraction, Fraction]]) -> Fraction:
    return sum(
        (end - start for start, end in merge_intervals(intervals)), Fraction(0)
    )


def interval_overlap(
    left: list[tuple[Fraction, Fraction]], right: list[tuple[Fraction, Fraction]]
) -> Fraction:
    total = Fraction(0)
    for a0, a1 in merge_intervals(left):
        for b0, b1 in merge_intervals(right):
            total += max(Fraction(0), min(a1, b1) - max(a0, b0))
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


def _memory_kind(value: str) -> str:
    lowered = value.strip().lower()
    if "device" in lowered:
        return "device"
    if "pin" in lowered or "page-lock" in lowered:
        return "pinned"
    if "page" in lowered:
        return "pageable"
    if "host" in lowered:
        return "host"
    return "unknown"


def _activity_direction(name: str) -> str | None:
    compact = re.sub(r"[^a-z]", "", name.lower())
    directions = {
        direction
        for direction, tokens in {
            "dtoh": ("dtoh", "devicetohost"),
            "htod": ("htod", "hosttodevice"),
            "dtod": ("dtod", "devicetodevice"),
        }.items()
        if any(token in compact for token in tokens)
    }
    if len(directions) > 1:
        raise AnalysisError(f"Nsight memcpy activity has ambiguous direction: {name}")
    return next(iter(directions), None)


def _memory_direction(source: str, destination: str) -> str | None:
    source_kind = _memory_kind(source)
    destination_kind = _memory_kind(destination)
    host_kinds = {"pageable", "pinned", "host"}
    if source_kind == "device" and destination_kind in host_kinds:
        return "dtoh"
    if source_kind in host_kinds and destination_kind == "device":
        return "htod"
    if source_kind == "device" and destination_kind == "device":
        return "dtod"
    return None


def _is_dtoh_activity(name: str, source: str, destination: str) -> bool:
    if "memcpy" not in name.lower():
        return False
    activity_direction = _activity_direction(name)
    memory_direction = _memory_direction(source, destination)
    if (
        activity_direction is not None
        and memory_direction is not None
        and activity_direction != memory_direction
    ):
        raise AnalysisError(
            "Nsight memcpy activity direction disagrees with source/destination memory kinds"
        )
    if activity_direction == "dtoh" and memory_direction != "dtoh":
        raise AnalysisError(
            "Nsight D2H activity lacks matching device-to-host memory kinds"
        )
    # Both independent sources must explicitly identify D2H. Unknown or
    # directionless memcpy rows cannot satisfy final-state copy completeness.
    return activity_direction == "dtoh" and memory_direction == "dtoh"


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
    start_factor = Fraction(Decimal(str(_unit_factor(start_col))))
    duration_factor = Fraction(Decimal(str(_unit_factor(duration_col))))
    bytes_factor = _unit_factor(bytes_col, bytes_value=True)

    copies: list[dict[str, Any]] = []
    kernels: list[tuple[Fraction, Fraction]] = []
    unmatched_tiny: list[dict[str, Any]] = []
    for row in rows:
        name = row.get(name_col, "")
        source = row.get(src_col, "")
        destination = row.get(dst_col, "")
        try:
            start = Fraction(decimal_value(row[start_col], f"{path}:start")) * start_factor
            duration = Fraction(
                decimal_value(row[duration_col], f"{path}:duration", positive=True)
            ) * duration_factor
        except (KeyError, ValueError) as error:
            raise AnalysisError(f"malformed relevant Nsight row in {path}: {row}") from error
        end = start + duration
        lowered = name.lower()
        if _is_dtoh_activity(name, source, destination):
            try:
                size = int(
                    round(
                        finite(float(row[bytes_col]), f"{path}:bytes")
                        * bytes_factor
                    )
                )
            except (KeyError, ValueError) as error:
                raise AnalysisError(f"malformed D2H byte value in {path}: {row}") from error
            item = {
                "start_ms": float(start),
                "end_ms": float(end),
                "duration_ms": float(duration),
                "start_ms_decimal": ratio_decimal_text(start),
                "end_ms_decimal": ratio_decimal_text(end),
                "duration_ms_decimal": ratio_decimal_text(duration),
                "bytes": size,
                "name": name,
                "source": source,
                "destination": destination,
                "destination_kind": _memory_kind(destination),
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
    expected_destination = "pageable" if mode in ("A", "B") else "pinned"
    if any(item["destination_kind"] != expected_destination for item in matched):
        raise AnalysisError(
            f"{mode} Nsight final-state copies must all use {expected_destination} destinations"
        )
    copy_intervals = [
        (
            Fraction(Decimal(item["start_ms_decimal"])),
            Fraction(Decimal(item["end_ms_decimal"])),
        )
        for item in matched
    ]
    copy_union = interval_duration(copy_intervals)
    overlap = interval_overlap(copy_intervals, kernels)
    summed_duration = sum(
        (Fraction(Decimal(item["duration_ms_decimal"])) for item in matched),
        Fraction(0),
    )
    exposed = copy_union - overlap
    kernel_union = interval_duration(kernels) if kernels else Fraction(0)
    return {
        "matched_final_state_copies": matched,
        "unmatched_tiny_dtoh": unmatched_tiny,
        "large_copy_count": len(matched),
        "large_copy_bytes": sum(item["bytes"] for item in matched),
        "large_copy_summed_duration_ms": float(summed_duration),
        "large_copy_summed_duration_ms_decimal": ratio_decimal_text(summed_duration),
        "copy_union_duration_ms": float(copy_union),
        "copy_union_duration_ms_decimal": ratio_decimal_text(copy_union),
        "copy_kernel_overlap_ms": float(overlap),
        "copy_kernel_overlap_ms_decimal": ratio_decimal_text(overlap),
        "exposed_final_state_dtoh_ms": float(exposed),
        "exposed_final_state_dtoh_ms_decimal": ratio_decimal_text(exposed),
        "destination_kinds": sorted({item["destination_kind"] for item in matched}),
        "streams": sorted({item["stream"] for item in matched}),
        "contexts": sorted({item["context"] for item in matched}),
        "kernel_union_duration_ms": float(kernel_union),
        "kernel_union_duration_ms_decimal": ratio_decimal_text(kernel_union),
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

    def fraction_value(value: Any, label: str, *, positive: bool = False) -> Fraction:
        if isinstance(value, Fraction):
            result = value
            if result < 0 or (positive and result <= 0):
                raise AnalysisError(
                    f"{label} must be {'positive' if positive else 'non-negative'}"
                )
            return result
        return Fraction(decimal_value(value, label, positive=positive))

    def total_fraction(field: str) -> Fraction:
        return sum(
            (
                fraction_value(final.get(field) or 0, field)
                for final in finals
            ),
            Fraction(0),
        )

    execution_default = sum(
        (
            fraction_value(draw["wall_time_ms"], "draw wall")
            for draw in timing["draw_timings"]
        ),
        Fraction(0),
    )
    accounting = timing["final_state_buffer_accounting"]
    accounting_fields = (
        "requested_lane_count",
        "retained_lane_count",
        "requested_buffer_set_count",
        "buffer_set_count",
        "requested_underlying_pinned_allocation_count",
        "underlying_pinned_allocation_count",
        "effective_pinned_bytes",
        "effective_cacheable_staging_bytes",
        "requested_pinned_bytes",
        "requested_cacheable_staging_bytes",
    )
    exact_values = {
        "whole_sweep_wall_time_ms": fraction_value(
            timing["whole_sweep_wall_time_ms"], "whole sweep wall", positive=True
        ),
        "setup_wall_time_ms": fraction_value(
            timing["setup_wall_time_ms"], "setup wall"
        ),
        "execution_window_wall_time_ms": fraction_value(
            timing.get("execution_window_wall_time_ms", execution_default),
            "execution wall",
        ),
        "publication_wall_time_ms": fraction_value(
            timing.get("publication_wall_time_ms", 0), "publication wall"
        ),
    }
    for field in (
        "final_state_seam_total_ms",
        "pageable_dtoh_host_api_ms",
        "pinned_dtoh_enqueue_api_ms",
        "wait_to_pinned_host_readable_ms",
        "pinned_to_cacheable_staging_copy_ms",
        "host_state_reconstruction_ms",
        "cpu_sha256_ms",
    ):
        exact_values[field] = total_fraction(field)

    metrics = {field: float(value) for field, value in exact_values.items()}
    metrics["buffer_accounting"] = {
        field: accounting[field] for field in accounting_fields
    }
    for field, value in exact_values.items():
        metrics[f"{field}_decimal"] = ratio_decimal_text(value)
        metrics[f"{field}_fraction"] = {
            "numerator": value.numerator,
            "denominator": value.denominator,
        }
    return metrics


def _threshold(name: str, actual: Any) -> dict[str, Any]:
    definition = dict(THRESHOLDS[name])
    if isinstance(actual, Fraction):
        actual_ratio = actual
    else:
        actual_ratio = Fraction(decimal_value(actual, f"{name} actual"))
    threshold_ratio = Fraction(Decimal(str(definition["value"])))
    definition.update(
        {
            "actual": float(actual_ratio),
            "actual_decimal": ratio_decimal_text(actual_ratio),
            "passed": actual_ratio <= threshold_ratio,
        }
    )
    return definition


def evaluate_thresholds(
    ratios: dict[str, Any], host_ratio: Any, nsight_ratio: Any
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
    exact_values: list[Fraction] = []
    for repetition in (1, 2, 3):
        num_record = records[("timed", workers, repetition, numerator)]
        den_record = records[("timed", workers, repetition, denominator)]
        num = metrics[num_record["id"]][field]
        den = metrics[den_record["id"]][field]
        fraction_field = f"{field}_fraction"
        try:
            num_encoded = metrics[num_record["id"]][fraction_field]
            den_encoded = metrics[den_record["id"]][fraction_field]
            num_exact = Fraction(num_encoded["numerator"], num_encoded["denominator"])
            den_exact = Fraction(den_encoded["numerator"], den_encoded["denominator"])
        except (KeyError, TypeError, ZeroDivisionError) as error:
            raise AnalysisError(f"exact metric is missing: {fraction_field}") from error
        ratio = exact_ratio(
            num_exact,
            den_exact,
            f"{numerator}/{denominator} workers {workers} repetition {repetition}",
        )
        exact_values.append(ratio)
        raw.append(
            {
                "repetition": repetition,
                "numerator_record_id": num_record["id"],
                "denominator_record_id": den_record["id"],
                "numerator_ms": num,
                "denominator_ms": den,
                "numerator_ms_decimal": ratio_decimal_text(num_exact),
                "denominator_ms_decimal": ratio_decimal_text(den_exact),
                "ratio": float(ratio),
                "ratio_decimal": ratio_decimal_text(ratio),
            }
        )
    median = median_decimal(exact_values)
    return {
        "raw": raw,
        "median_ratio": float(median),
        "median_ratio_decimal": ratio_decimal_text(median),
        "median_ratio_fraction": {
            "numerator": median.numerator,
            "denominator": median.denominator,
        },
    }


def _comparison_groups(manifest: dict[str, Any]) -> list[tuple[str, list[dict[str, Any]]]]:
    executions = manifest["executions"]
    groups: list[tuple[str, list[dict[str, Any]]]] = [
        ("preflight-independent", executions[:3])
    ]
    for offset in range(3, 21, 3):
        group = executions[offset : offset + 3]
        groups.append(
            (
                f"{group[0]['class']}-{group[0]['workers']}-{group[0]['repetition']}",
                group,
            )
        )
    groups.append(("crn-4-1", executions[21:24]))
    groups.append(("profile-4-1", executions[24:27]))
    return groups


def _recompute_comparison(
    root: pathlib.Path,
    manifest: dict[str, Any],
    label: str,
    group: list[dict[str, Any]],
) -> dict[str, Any]:
    by_mode = {record["mode"]: record for record in group}
    if set(by_mode) != set(protocol.MODES):
        raise AnalysisError(f"{label} does not contain exactly A/B/C")
    outputs = {
        mode: evidence_path(
            root,
            manifest,
            record["output_dir"],
            record.get("output_relative"),
        )
        for mode, record in by_mode.items()
    }
    ab = protocol.compare_output_trees(outputs["A"], outputs["B"])
    ac = protocol.compare_output_trees(outputs["A"], outputs["C"])
    hashes = {
        mode: protocol.final_state_hashes(output) for mode, output in outputs.items()
    }
    return {
        "label": label,
        "record_ids": [record["id"] for record in group],
        "A_B": ab,
        "A_C": ac,
        "final_state_sha256": hashes,
        "tree_parity": ab["equal"] and ac["equal"],
        "digest_parity": hashes["A"] == hashes["B"] == hashes["C"],
    }


def validate_comparisons(
    root: pathlib.Path,
    manifest: dict[str, Any],
    comparisons: Any,
) -> None:
    groups = _comparison_groups(manifest)
    if not isinstance(comparisons, list) or len(comparisons) != len(groups):
        raise AnalysisError("independent/CRN/profile output parity evidence is incomplete")
    for recorded, (label, group) in zip(comparisons, groups):
        if not isinstance(recorded, dict):
            raise AnalysisError(f"comparison {label} is malformed")
        recomputed = _recompute_comparison(root, manifest, label, group)
        if recorded.get("label") != label or recorded.get("record_ids") != recomputed["record_ids"]:
            raise AnalysisError(f"comparison {label} identity is inconsistent")
        for field in ("tree_parity", "digest_parity", "passed"):
            if recorded.get(field) is not True:
                raise AnalysisError(f"comparison {label} reports failed {field}")
        for field in ("A_B", "A_C"):
            nested = recorded.get(field)
            if (
                not isinstance(nested, dict)
                or nested.get("equal") is not True
                or nested.get("missing_from_right") != []
                or nested.get("extra_in_right") != []
                or nested.get("changed") != []
                or nested.get("left_file_count") != nested.get("right_file_count")
                or nested.get("left_file_count", 0) <= 0
            ):
                raise AnalysisError(f"comparison {label} has contradictory {field} evidence")
        if recorded.get("final_state_sha256") != recomputed["final_state_sha256"]:
            raise AnalysisError(f"comparison {label} digest evidence disagrees with output trees")
        if not recomputed["tree_parity"] or not recomputed["digest_parity"]:
            raise AnalysisError(f"comparison {label} output parity failed on recomputation")


def validate_negative_control(negative: Any) -> None:
    if not isinstance(negative, dict):
        raise AnalysisError("deliberate negative control is malformed")
    comparison = negative.get("comparison")
    target = negative.get("perturbed_relative_path")
    if (
        negative.get("accepted") is not False
        or negative.get("rejected") is not True
        or not isinstance(comparison, dict)
        or comparison.get("equal") is not False
        or not isinstance(target, str)
        or not target
        or comparison.get("changed") != [target]
        or comparison.get("missing_from_right") != []
        or comparison.get("extra_in_right") != []
    ):
        raise AnalysisError("deliberate negative control was not consistently rejected")


def _strict_nonnegative_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AnalysisError(f"{label} must be a non-negative integer")
    return value


def validate_resource_samples(record: dict[str, Any]) -> tuple[int, int]:
    samples = record.get("resource_samples")
    if record.get("resource_sampling_complete") is not True or not isinstance(samples, list) or not samples:
        raise AnalysisError(f"record {record['id']} has incomplete resource evidence")
    successful_rss: list[int] = []
    successful_vram: list[int] = []
    for index, sample in enumerate(samples):
        if not isinstance(sample, dict):
            raise AnalysisError(f"record {record['id']} resource sample {index} is malformed")
        rss = _strict_nonnegative_int(
            sample.get("rss_bytes"), f"record {record['id']} sample {index} RSS"
        )
        vram = _strict_nonnegative_int(
            sample.get("vram_bytes"), f"record {record['id']} sample {index} VRAM"
        )
        rss_succeeded = sample.get("rss_query_succeeded")
        vram_succeeded = sample.get("vram_query_succeeded")
        if not isinstance(rss_succeeded, bool) or not isinstance(vram_succeeded, bool):
            raise AnalysisError(f"record {record['id']} resource query flags are malformed")
        if rss_succeeded:
            successful_rss.append(rss)
        elif rss != 0:
            raise AnalysisError(f"record {record['id']} failed RSS query reported bytes")
        if vram_succeeded:
            successful_vram.append(vram)
        elif vram != 0:
            raise AnalysisError(f"record {record['id']} failed VRAM query reported bytes")
    if not successful_rss or max(successful_rss) <= 0 or not successful_vram or max(successful_vram) <= 0:
        raise AnalysisError(f"record {record['id']} has no positive successful RSS/VRAM sample")
    peak_rss = max(successful_rss)
    peak_vram = max(successful_vram)
    if _strict_nonnegative_int(record.get("peak_rss_bytes"), f"record {record['id']} peak RSS") != peak_rss:
        raise AnalysisError(f"record {record['id']} peak RSS does not match samples")
    if _strict_nonnegative_int(record.get("peak_vram_bytes"), f"record {record['id']} peak VRAM") != peak_vram:
        raise AnalysisError(f"record {record['id']} peak VRAM does not match samples")
    return peak_rss, peak_vram


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
    validate_negative_control(negative)
    comparisons = load_json(root / "comparisons.json")
    validate_comparisons(root, manifest, comparisons)

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
        peak_rss, peak_vram = validate_resource_samples(record)
        record["peak_rss_bytes"] = peak_rss
        record["peak_vram_bytes"] = peak_vram
        if expected["profiled"]:
            raw_report = evidence_path(
                root,
                manifest,
                expected.get("nsys_report"),
                expected.get("nsys_report_relative"),
            )
            if not raw_report.is_file() or raw_report.stat().st_size == 0:
                raise AnalysisError(f"profile {expected['id']} is missing its raw Nsight report")
        timing_path = evidence_path(
            root, manifest, expected["timing_json"], expected.get("timing_relative")
        )
        try:
            protocol.validate_timing(timing_path, expected)
        except protocol.ProtocolError as error:
            raise AnalysisError(str(error)) from error
        # Structural validation above remains shared with the collector. Re-read
        # the same bytes without binary-float coercion for every gate operand.
        timing = load_json(timing_path, exact_decimals=True)
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
    def metric_fraction(item: dict[str, Any], field: str) -> Fraction:
        encoded = item[f"{field}_fraction"]
        return Fraction(encoded["numerator"], encoded["denominator"])

    host_exact: list[Fraction] = []
    for item in host_ratios["raw"]:
        c_record = keyed[("timed", 4, item["repetition"], "C")]
        b_record = keyed[("timed", 4, item["repetition"], "B")]
        c_metrics = metrics[c_record["id"]]
        b_metrics = metrics[b_record["id"]]
        numerator = (
            metric_fraction(c_metrics, "pinned_dtoh_enqueue_api_ms")
            + metric_fraction(c_metrics, "wait_to_pinned_host_readable_ms")
        )
        denominator = metric_fraction(b_metrics, "pageable_dtoh_host_api_ms")
        ratio = exact_ratio(
            numerator,
            denominator,
            f"C/B host mechanism repetition {item['repetition']}",
        )
        host_exact.append(ratio)
        item["numerator_ms"] = float(numerator)
        item["denominator_ms"] = float(denominator)
        item["numerator_ms_decimal"] = ratio_decimal_text(numerator)
        item["denominator_ms_decimal"] = ratio_decimal_text(denominator)
        item["ratio"] = float(ratio)
        item["ratio_decimal"] = ratio_decimal_text(ratio)
    host_median = median_decimal(host_exact)
    host_ratios["median_ratio"] = float(host_median)
    host_ratios["median_ratio_decimal"] = ratio_decimal_text(host_median)
    host_ratios["median_ratio_fraction"] = {
        "numerator": host_median.numerator,
        "denominator": host_median.denominator,
    }

    profiles: dict[str, Any] = {}
    for mode in "ABC":
        record = keyed[("profile", 4, 1, mode)]
        exports = record.get("nsys_exports")
        expected_record = manifest["executions"][record["id"] - 1]
        expected_exports = {
            name: (
                pathlib.Path(expected_record["arm_relative"])
                / "profile"
                / f"{name}.csv"
            ).as_posix()
            for name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum")
        }
        if exports != expected_exports:
            raise AnalysisError(f"profile {mode} exports are not bound to their owning arm")
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
    b_exposed_decimal = profiles["B"]["trace"][
        "exposed_final_state_dtoh_ms_decimal"
    ]
    c_exposed_decimal = profiles["C"]["trace"][
        "exposed_final_state_dtoh_ms_decimal"
    ]
    nsight_ratio_exact = exact_ratio(
        c_exposed_decimal,
        b_exposed_decimal,
        "C/B Nsight exposed final-state D2H",
    )
    nsight_ratio = float(nsight_ratio_exact)

    def median_fraction(name: str) -> Fraction:
        encoded = ratio_sets[name]["median_ratio_fraction"]
        return Fraction(encoded["numerator"], encoded["denominator"])

    threshold_results = evaluate_thresholds(
        {
            "B_A_workers4": median_fraction("B_A_workers4"),
            "B_A_workers1": median_fraction("B_A_workers1"),
            "C_B_workers4": median_fraction("C_B_workers4"),
            "C_B_workers1": median_fraction("C_B_workers1"),
        },
        host_median,
        nsight_ratio_exact,
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
    command_metrics: dict[str, dict[str, Any]] = {}
    for record in sorted(records, key=lambda item: item["id"]):
        command_metrics[str(record["id"])] = {
            "id": record["id"],
            "class": record["class"],
            "mode": record["mode"],
            "selector": record["selector"],
            "workers": record["workers"],
            "draws": record["draws"],
            "noise": record["noise"],
            "repetition": record["repetition"],
            "profiled": record["profiled"],
            "included_in_performance": record["included_in_performance"],
            "peak_rss_bytes": record["peak_rss_bytes"],
            "peak_vram_bytes": record["peak_vram_bytes"],
            **metrics[record["id"]],
        }
    preflight_performance = [
        command_metrics[str(record["id"])]
        for record in records
        if record["class"] == "preflight"
    ]
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
            "ratio_decimal": ratio_decimal_text(nsight_ratio_exact),
        },
        "preflight_performance": {
            "informational_only": True,
            "excluded_from_promotion": True,
            "commands": preflight_performance,
        },
        "absolute_command_metrics": command_metrics,
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
    def ms(value: Any) -> str:
        return f"{float(value):.3f}"

    lines = [
        "# CUDA final-state A/B/C decision",
        "",
        f"Verdict: **{result['eligibility']['verdict']}**",
        "",
        "## Preflight performance (informational only)",
        "",
        "These one-draw results establish correctness and diagnostics only; they are excluded from promotion.",
        "",
        "| ID | Mode | whole_sweep_wall_time_ms | setup_wall_time_ms | execution_window_wall_time_ms | publication_wall_time_ms | final_state_seam_total_ms |",
        "|---:|:---:|---:|---:|---:|---:|---:|",
    ]
    for item in result["preflight_performance"]["commands"]:
        lines.append(
            f"| {item['id']} | {item['mode']} | {ms(item['whole_sweep_wall_time_ms'])} | "
            f"{ms(item['setup_wall_time_ms'])} | {ms(item['execution_window_wall_time_ms'])} | "
            f"{ms(item['publication_wall_time_ms'])} | {ms(item['final_state_seam_total_ms'])} |"
        )

    lines += [
        "",
        "## Absolute command and phase evidence",
        "",
        "| ID | Class | Mode | Workers | Rep | whole_sweep_wall_time_ms | setup_wall_time_ms | execution_window_wall_time_ms | publication_wall_time_ms | final_state_seam_total_ms | pageable_dtoh_host_api_ms | pinned_dtoh_enqueue_api_ms | wait_to_pinned_host_readable_ms | pinned_to_cacheable_staging_copy_ms | host_state_reconstruction_ms | cpu_sha256_ms |",
        "|---:|---|:---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
    ]
    for item in result["absolute_command_metrics"].values():
        repetition = "-" if item["repetition"] is None else str(item["repetition"])
        lines.append(
            f"| {item['id']} | {item['class']} | {item['mode']} | {item['workers']} | {repetition} | "
            f"{ms(item['whole_sweep_wall_time_ms'])} | {ms(item['setup_wall_time_ms'])} | "
            f"{ms(item['execution_window_wall_time_ms'])} | {ms(item['publication_wall_time_ms'])} | "
            f"{ms(item['final_state_seam_total_ms'])} | {ms(item['pageable_dtoh_host_api_ms'])} | "
            f"{ms(item['pinned_dtoh_enqueue_api_ms'])} | {ms(item['wait_to_pinned_host_readable_ms'])} | "
            f"{ms(item['pinned_to_cacheable_staging_copy_ms'])} | {ms(item['host_state_reconstruction_ms'])} | "
            f"{ms(item['cpu_sha256_ms'])} |"
        )

    lines += [
        "",
        "## Raw adjacent repetitions",
        "",
        "| Comparison | Repetition | Numerator ms | Denominator ms | Ratio | Median ratio |",
        "|---|---:|---:|---:|---:|---:|",
    ]
    for name, values in result["ratios"].items():
        for item in values["raw"]:
            lines.append(
                f"| {name} | {item['repetition']} | {ms(item['numerator_ms'])} | "
                f"{ms(item['denominator_ms'])} | {item['ratio_decimal']} | "
                f"{values['median_ratio_decimal']} |"
            )

    lines += [
        "",
        "## C mechanism evidence",
        "",
        "| Mechanism | Repetition | B denominator ms | C numerator ms | Ratio |",
        "|---|---:|---:|---:|---:|",
    ]
    for item in result["host_mechanism_ratios"]["raw"]:
        lines.append(
            f"| Host blocking | {item['repetition']} | {ms(item['denominator_ms'])} | "
            f"{ms(item['numerator_ms'])} | {item['ratio_decimal']} |"
        )
    nsight = result["nsight_mechanism"]
    lines.append(
        f"| Nsight exposed D2H | - | {ms(nsight['B_exposed_final_state_dtoh_ms'])} | "
        f"{ms(nsight['C_exposed_final_state_dtoh_ms'])} | {nsight['ratio_decimal']} |"
    )

    lines += [
        "",
        "## Nsight final-state copy evidence",
        "",
        "| Mode | large_copy_count | large_copy_bytes | large_copy_summed_duration_ms | copy_union_duration_ms | copy_kernel_overlap_ms | exposed_final_state_dtoh_ms |",
        "|:---:|---:|---:|---:|---:|---:|---:|",
    ]
    for mode, profile in result["profiles"].items():
        trace = profile["trace"]
        lines.append(
            f"| {mode} | {trace['large_copy_count']} | {trace['large_copy_bytes']} | "
            f"{ms(trace['large_copy_summed_duration_ms'])} | {ms(trace['copy_union_duration_ms'])} | "
            f"{ms(trace['copy_kernel_overlap_ms'])} | {ms(trace['exposed_final_state_dtoh_ms'])} |"
        )

    lines += [
        "",
        "## Resource evidence by worker count",
        "",
        "| Workers | Mode | peak_rss_bytes | peak_vram_bytes | requested_pinned_bytes | requested_staging_bytes | buffer_sets_per_retained_lane | pinned_allocations_per_retained_lane | Pass |",
        "|---:|:---:|---:|---:|---:|---:|---:|---:|:---:|",
    ]
    for workers, resource in result["resources_by_workers"].items():
        for mode in "ABC":
            sets = ",".join(f"{value:.3f}" for value in resource["buffer_sets_per_retained_lane"])
            allocations = ",".join(
                f"{value:.3f}" for value in resource["pinned_allocations_per_retained_lane"]
            )
            lines.append(
                f"| {workers} | {mode} | {resource['peak_rss_bytes_by_mode'][mode]} | "
                f"{resource['peak_vram_bytes_by_mode'][mode]} | "
                f"{resource['requested_pinned_bytes'] if mode == 'C' else 0} | "
                f"{resource['requested_staging_bytes'] if mode == 'C' else 0} | "
                f"{sets if mode == 'C' else '0'} | {allocations if mode == 'C' else '0'} | "
                f"{'yes' if resource['memory_passed'] else 'no'} |"
            )

    lines += [
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
                f"| {name} | {gate['actual_decimal']} | {gate['direction']} {gate['value']:.2f} | "
                f"{'yes' if gate['passed'] else 'no'} |"
            )
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

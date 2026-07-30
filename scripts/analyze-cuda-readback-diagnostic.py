#!/usr/bin/env python3
"""Analyze the bounded CUDA readback/contended-kernel diagnostic artifacts."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
from collections import Counter, defaultdict
from pathlib import Path
from typing import Iterable

DETAIL_REFERENCE_KERNEL = "sembla_count_deferred"


def csv_rows(
    path: Path, first_header: str, required_headers: Iterable[str] = ()
) -> list[dict[str, str]]:
    lines = path.read_text(errors="replace").splitlines()
    try:
        start = next(i for i, line in enumerate(lines) if line.startswith(first_header + ","))
    except StopIteration as error:
        raise ValueError(f"{path}: missing CSV header beginning {first_header!r}") from error
    reader = csv.DictReader(lines[start:])
    missing = set(required_headers) - set(reader.fieldnames or ())
    if missing:
        raise ValueError(f"{path}: missing CSV columns {sorted(missing)}")
    return list(reader)


def integer(row: dict[str, str], key: str) -> int:
    return int(row[key].strip())


def require_hex_identifier(value: object, length: int, label: str) -> str:
    identifier = value if isinstance(value, str) else ""
    if len(identifier) != length:
        raise ValueError(f"invalid {label}: expected {length} hexadecimal characters")
    try:
        int(identifier, 16)
    except ValueError as error:
        raise ValueError(f"invalid {label}: not hexadecimal") from error
    return identifier


def number(row: dict[str, str], key: str) -> float:
    value = row.get(key, "").strip()
    if not value:
        raise ValueError(f"missing numeric CSV field {key!r}")
    result = float(value)
    if not math.isfinite(result):
        raise ValueError(f"non-finite CSV field {key!r}: {value!r}")
    return result


def merge_intervals(intervals: Iterable[tuple[int, int]]) -> list[tuple[int, int]]:
    merged: list[list[int]] = []
    for start, end in sorted(intervals):
        if end < start:
            raise ValueError(f"negative interval: {start}..{end}")
        if not merged or start > merged[-1][1]:
            merged.append([start, end])
        else:
            merged[-1][1] = max(merged[-1][1], end)
    return [(start, end) for start, end in merged]


def interval_duration(intervals: Iterable[tuple[int, int]]) -> int:
    return sum(end - start for start, end in merge_intervals(intervals))


def intersection_duration(
    left: Iterable[tuple[int, int]], right: Iterable[tuple[int, int]]
) -> int:
    a = merge_intervals(left)
    b = merge_intervals(right)
    i = j = total = 0
    while i < len(a) and j < len(b):
        start = max(a[i][0], b[j][0])
        end = min(a[i][1], b[j][1])
        if end > start:
            total += end - start
        if a[i][1] <= b[j][1]:
            i += 1
        else:
            j += 1
    return total


def maximum_concurrency(intervals: Iterable[tuple[int, int]]) -> int:
    events: list[tuple[int, int]] = []
    for start, end in intervals:
        events.extend(((start, 1), (end, -1)))
    # Ends precede starts at equal timestamps: touching intervals do not overlap.
    events.sort(key=lambda event: (event[0], event[1]))
    active = maximum = 0
    for _, change in events:
        active += change
        maximum = max(maximum, active)
    return maximum


def analyze_trace(path: Path) -> dict[str, object]:
    rows = csv_rows(
        path,
        "Start (ns)",
        (
            "Start (ns)",
            "Duration (ns)",
            "GrdX",
            "Bytes (MB)",
            "SrcMemKd",
            "DstMemKd",
            "Ctx",
            "Strm",
            "Name",
        ),
    )
    kernels: list[tuple[int, int, str, str, str]] = []
    d2h: list[tuple[int, int, float, str, str, str]] = []
    for row in rows:
        name = row.get("Name", "")
        grid_x_raw = row.get("GrdX", "").strip()
        is_kernel = bool(name) and not name.startswith("[CUDA")
        is_d2h = name == "[CUDA memcpy Device-to-Host]"
        if not is_kernel and not is_d2h:
            continue
        try:
            start = integer(row, "Start (ns)")
            duration = integer(row, "Duration (ns)")
        except (KeyError, ValueError) as error:
            raise ValueError(f"{path}: malformed relevant timeline row {row}") from error
        if start <= 0 or duration <= 0:
            raise ValueError(f"{path}: non-positive relevant timeline row {row}")
        end = start + duration
        if is_kernel:
            try:
                grid_x = int(grid_x_raw)
            except ValueError as error:
                raise ValueError(f"{path}: kernel row has nonnumeric GrdX {row}") from error
            if grid_x <= 0:
                raise ValueError(f"{path}: kernel row has nonpositive GrdX {row}")
            context = row.get("Ctx", "").strip()
            stream = row.get("Strm", "").strip()
            if not name or not context or not stream:
                raise ValueError(f"{path}: kernel row lacks name/context/stream {row}")
            kernels.append((start, end, context, stream, name))
        else:
            source = row.get("SrcMemKd", "").strip()
            destination = row.get("DstMemKd", "").strip()
            stream = row.get("Strm", "").strip()
            if not source or not destination or not stream:
                raise ValueError(f"{path}: D2H row lacks memory kind/stream {row}")
            bytes_mb = number(row, "Bytes (MB)")
            if bytes_mb < 0:
                raise ValueError(f"{path}: D2H row has negative byte count {row}")
            d2h.append((start, end, bytes_mb, source, destination, stream))

    if not kernels:
        raise ValueError(f"{path}: no CUDA kernel rows")
    if not d2h:
        raise ValueError(f"{path}: no CUDA D2H rows")

    kernel_intervals = [(start, end) for start, end, *_ in kernels]
    d2h_intervals = [(start, end) for start, end, *_ in d2h]
    kernel_totals: dict[str, int] = defaultdict(int)
    kernel_instances = Counter()
    for start, end, _, _, name in kernels:
        kernel_totals[name] += end - start
        kernel_instances[name] += 1

    size_groups: dict[str, dict[str, float | int]] = {}
    for label, predicate in (
        ("small_lt_1mb", lambda size: size < 1.0),
        ("large_ge_1mb", lambda size: size >= 1.0),
    ):
        selected = [copy for copy in d2h if predicate(copy[2])]
        intervals = [(start, end) for start, end, *_ in selected]
        union = interval_duration(intervals)
        overlap = intersection_duration(intervals, kernel_intervals)
        size_groups[label] = {
            "calls": len(selected),
            "bytes_mb": sum(copy[2] for copy in selected),
            "summed_duration_ms": sum(end - start for start, end, *_ in selected)
            / 1e6,
            "union_duration_ms": union / 1e6,
            "kernel_overlap_ms": overlap / 1e6,
            "unoverlapped_ms": (union - overlap) / 1e6,
        }

    d2h_union = interval_duration(d2h_intervals)
    d2h_overlap = intersection_duration(d2h_intervals, kernel_intervals)
    kernel_union = interval_duration(kernel_intervals)
    return {
        "kernel_count": len(kernels),
        "kernel_contexts": dict(sorted(Counter(kernel[2] for kernel in kernels).items())),
        "kernel_streams": dict(sorted(Counter(kernel[3] for kernel in kernels).items())),
        "kernel_summed_duration_ms": sum(end - start for start, end, *_ in kernels)
        / 1e6,
        "kernel_union_duration_ms": kernel_union / 1e6,
        "kernel_maximum_concurrency": maximum_concurrency(kernel_intervals),
        "kernel_totals_ms": {
            name: duration / 1e6 for name, duration in sorted(kernel_totals.items())
        },
        "kernel_instances": dict(sorted(kernel_instances.items())),
        "d2h": {
            "calls": len(d2h),
            "bytes_mb": sum(copy[2] for copy in d2h),
            "summed_duration_ms": sum(end - start for start, end, *_ in d2h) / 1e6,
            "union_duration_ms": d2h_union / 1e6,
            "kernel_overlap_ms": d2h_overlap / 1e6,
            "unoverlapped_ms": (d2h_union - d2h_overlap) / 1e6,
            "overlap_fraction": d2h_overlap / d2h_union if d2h_union else 0.0,
            "destinations": dict(sorted(Counter(copy[4] for copy in d2h).items())),
            "streams": dict(sorted(Counter(copy[5] for copy in d2h).items())),
            "size_groups": size_groups,
        },
    }


def analyze_api(path: Path) -> dict[str, dict[str, float | int]]:
    selected = {
        "cuMemcpyDtoHAsync_v2",
        "cuStreamWaitEvent",
        "cuStreamSynchronize",
        "cuCtxSynchronize",
        "cuLaunchKernel",
        "cuEventRecord",
    }
    result: dict[str, dict[str, float | int]] = {}
    for row in csv_rows(
        path, "Time (%)", ("Total Time (ns)", "Num Calls", "Name")
    ):
        name = row.get("Name", "")
        if name not in selected:
            continue
        calls_key = "Num Calls" if "Num Calls" in row else "Instances"
        calls = integer(row, calls_key)
        total_ns = integer(row, "Total Time (ns)")
        if calls <= 0 or total_ns <= 0:
            raise ValueError(f"{path}: nonpositive CUDA API count/time for {name}")
        result[name] = {
            "calls": calls,
            "summed_thread_time_ms": total_ns / 1e6,
        }
    required = {"cuMemcpyDtoHAsync_v2", "cuLaunchKernel"}
    missing = required - set(result)
    if missing:
        raise ValueError(f"{path}: missing essential CUDA API rows {sorted(missing)}")
    return result


def phase_timing(path: Path, expected_scale: int, expected_ticks: int) -> dict[str, object]:
    document = json.loads(path.read_text())
    if document.get("schema") != "sembla-execution-timing-v1":
        raise ValueError(f"{path}: unexpected timing schema {document.get('schema')!r}")
    session = document.get("session", {})
    if session.get("backend") != "cuda":
        raise ValueError(f"{path}: expected CUDA backend")
    if session.get("scale") != expected_scale or session.get("ticks") != expected_ticks:
        raise ValueError(f"{path}: unexpected scale/tick identity")
    require_hex_identifier(session.get("repository_commit"), 40, "repository commit")
    require_hex_identifier(session.get("binary_sha256"), 64, "binary SHA-256")
    ticks = document.get("ticks", [])
    if len(ticks) != expected_ticks:
        raise ValueError(f"{path}: missing tick timing rows")
    tolerance = float(document.get("self_check", {}).get("tolerance_ms", 0.001))
    if not math.isfinite(tolerance) or tolerance < 0:
        raise ValueError(f"{path}: invalid timing tolerance")
    expected_phases = {
        "kernels",
        "readback_control",
        "state_transfer",
        "state_reconstruct",
        "state_hash",
        "observe_views",
        "report",
        "other",
    }
    tick_phase_totals = {name: 0.0 for name in expected_phases}
    tick_wall_total = 0.0
    for expected_tick, tick in enumerate(ticks):
        if tick.get("tick") != expected_tick:
            raise ValueError(f"{path}: tick rows are not contiguous and ordered")
        tick_wall = float(tick.get("wall_time_ms", 0.0))
        tick_phases = tick.get("phases_ms", {})
        if not math.isfinite(tick_wall) or tick_wall <= 0:
            raise ValueError(f"{path}: tick {expected_tick} has invalid wall duration")
        if set(tick_phases) != expected_phases:
            raise ValueError(f"{path}: tick {expected_tick} has an unexpected phase set")
        values = [float(value) for value in tick_phases.values()]
        if any(not math.isfinite(value) or value < 0 for value in values):
            raise ValueError(f"{path}: tick {expected_tick} has invalid phase duration")
        measured_sum = float(tick.get("phase_sum_ms", sum(values)))
        if not math.isfinite(measured_sum) or abs(measured_sum - sum(values)) > tolerance:
            raise ValueError(f"{path}: tick {expected_tick} phase sum is inconsistent")
        if abs(measured_sum - tick_wall) > tolerance or not tick.get("within_tolerance", True):
            raise ValueError(f"{path}: tick {expected_tick} phases do not reconcile to wall")
        tick_wall_total += tick_wall
        for name, value in tick_phases.items():
            tick_phase_totals[name] += float(value)
    totals = document.get("totals", {})
    wall = float(totals.get("wall_time_ms", 0.0))
    if not math.isfinite(wall) or wall <= 0:
        raise ValueError(f"{path}: invalid total wall duration")
    self_check = document.get("self_check", {})
    if not self_check.get("all_ticks_reconciled") or not self_check.get(
        "other_non_negative"
    ):
        raise ValueError(f"{path}: timing self-check failed")
    phases = totals.get("phases_ms", {})
    if set(phases) != expected_phases:
        raise ValueError(f"{path}: totals have an unexpected phase set")
    if any(not math.isfinite(float(value)) or float(value) < 0 for value in phases.values()):
        raise ValueError(f"{path}: invalid phase duration")
    total_phase_sum = float(totals.get("phase_sum_ms", sum(map(float, phases.values()))))
    if not math.isfinite(total_phase_sum) or abs(total_phase_sum - sum(map(float, phases.values()))) > tolerance:
        raise ValueError(f"{path}: total phase sum is inconsistent")
    if abs(total_phase_sum - wall) > tolerance:
        raise ValueError(f"{path}: total phases do not reconcile to wall")
    if abs(wall - tick_wall_total) > tolerance:
        raise ValueError(f"{path}: total wall differs from summed tick walls")
    for name in expected_phases:
        if abs(float(phases[name]) - tick_phase_totals[name]) > tolerance:
            raise ValueError(f"{path}: total {name} differs from summed tick phases")
    return document


def sweep_timing(
    path: Path,
    expected_workers: int,
    expected_draws: int,
    expected_commit: str,
    expected_binary: str,
) -> dict[str, object]:
    document = json.loads(path.read_text())
    if document.get("backend") != "cuda":
        raise ValueError(f"{path}: expected CUDA backend")
    require_hex_identifier(document.get("repository_commit"), 40, "repository commit")
    require_hex_identifier(document.get("binary_sha256"), 64, "binary SHA-256")
    if document.get("repository_commit") != expected_commit:
        raise ValueError(f"{path}: repository commit differs from phase timing")
    if document.get("binary_sha256") != expected_binary:
        raise ValueError(f"{path}: binary hash differs from phase timing")
    if document.get("draws") != expected_draws or document.get("ticks_per_draw") != 24:
        raise ValueError(f"{path}: unexpected sweep dimensions")
    whole = float(document.get("whole_sweep_wall_time_ms", 0.0))
    if not math.isfinite(whole) or whole <= 0:
        raise ValueError(f"{path}: invalid whole-sweep duration")
    if expected_workers == 1:
        if document.get("schema") != "sembla-sweep-timing-v1":
            raise ValueError(f"{path}: unexpected sequential timing schema")
    else:
        if document.get("schema") != "sembla-sweep-concurrency-spike-timing-v1":
            raise ValueError(f"{path}: unexpected concurrent timing schema")
        if document.get("requested_draw_workers") != expected_workers:
            raise ValueError(f"{path}: requested worker mismatch")
        if document.get("effective_draw_workers") != expected_workers:
            raise ValueError(f"{path}: effective worker mismatch")
        if document.get("execution_mode") != "cuda-free-nonblocking-streams":
            raise ValueError(f"{path}: wrong execution mode")
        window = float(document.get("execution_window_wall_time_ms", 0.0))
        if not math.isfinite(window) or window <= 0:
            raise ValueError(f"{path}: invalid execution-window duration")
    return document


def ncu_rows(path: Path) -> list[dict[str, str]]:
    lines = path.read_text(errors="replace").splitlines()
    start = None
    for index, line in enumerate(lines):
        parsed = next(csv.reader([line]), [])
        if parsed and parsed[0].strip() == "ID":
            start = index
            break
    if start is None:
        raise ValueError(f"{path}: missing NCU details CSV header")
    reader = csv.DictReader(lines[start:])
    required = {"Kernel Name", "Section Name", "Metric Name", "Metric Value"}
    missing = required - set(reader.fieldnames or ())
    if missing:
        raise ValueError(f"{path}: missing NCU columns {sorted(missing)}")
    return list(reader)


def validate_ncu_csv(path: Path, kernel: str, report_type: str) -> dict[str, object]:
    rows = [row for row in ncu_rows(path) if row.get("Kernel Name", "").strip() == kernel]
    if not rows:
        raise ValueError(f"{path}: no NCU rows for selected kernel {kernel}")
    families = (
        (("speed of light", "throughput"), ("occupancy",), ("launch",))
        if report_type == "sol"
        else (("memory workload",), ("scheduler",), ("warp state",))
    )
    matched: dict[str, int] = {}
    for family in families:
        label = "/".join(family)
        family_rows = []
        for row in rows:
            section = row.get("Section Name", "").lower()
            if any(term in section for term in family):
                family_rows.append(row)
        numeric = 0
        for row in family_rows:
            raw = row.get("Metric Value", "").strip().replace(",", "").rstrip("%")
            try:
                value = float(raw)
            except ValueError:
                continue
            numeric += math.isfinite(value)
        if not family_rows or numeric == 0:
            raise ValueError(f"{path}: missing numeric NCU metric family {label}")
        matched[label] = numeric
    return {"rows": len(rows), "numeric_metric_families": matched}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--phase-timing", type=Path, required=True)
    parser.add_argument("--sequential-trace", type=Path, required=True)
    parser.add_argument("--concurrent-trace", type=Path, required=True)
    parser.add_argument("--sequential-api", type=Path, required=True)
    parser.add_argument("--concurrent-api", type=Path, required=True)
    parser.add_argument("--sequential-timing", type=Path, required=True)
    parser.add_argument("--concurrent-timing", type=Path, required=True)
    parser.add_argument("--expected-scale", type=int, default=10_000_000)
    parser.add_argument("--expected-ticks", type=int, default=24)
    parser.add_argument("--draws-per-arm", type=int, default=4)
    parser.add_argument("--selected-kernels-out", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--ncu-dir", type=Path)
    parser.add_argument("--assertions", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    phases = phase_timing(args.phase_timing, args.expected_scale, args.expected_ticks)
    sequential = analyze_trace(args.sequential_trace)
    concurrent = analyze_trace(args.concurrent_trace)
    expected_commit = require_hex_identifier(
        phases["session"].get("repository_commit"), 40, "repository commit"
    )
    expected_binary = require_hex_identifier(
        phases["session"].get("binary_sha256"), 64, "binary SHA-256"
    )
    sequential_timing = sweep_timing(
        args.sequential_timing, 1, args.draws_per_arm, expected_commit, expected_binary
    )
    concurrent_timing = sweep_timing(
        args.concurrent_timing, 4, args.draws_per_arm, expected_commit, expected_binary
    )

    if sequential["kernel_count"] != concurrent["kernel_count"]:
        raise ValueError("Systems arms contain different kernel counts")
    if sequential["kernel_instances"] != concurrent["kernel_instances"]:
        raise ValueError("Systems arms contain different per-kernel instance counts")
    concurrent_streams = {name for name in concurrent["kernel_streams"] if name}
    concurrent_contexts = {name for name in concurrent["kernel_contexts"] if name}
    if len(concurrent_streams) < 4:
        raise ValueError("four-worker Systems arm did not use four nonempty streams")
    if len(concurrent_contexts) != 1:
        raise ValueError("four-worker Systems arm did not use one shared CUDA context")

    comparisons = []
    common_kernels = sorted(
        set(sequential["kernel_totals_ms"]) & set(concurrent["kernel_totals_ms"])
    )
    for name in common_kernels:
        seq_ms = float(sequential["kernel_totals_ms"].get(name, 0.0))
        conc_ms = float(concurrent["kernel_totals_ms"].get(name, 0.0))
        if seq_ms == 0.0 or conc_ms == 0.0:
            continue
        comparisons.append(
            {
                "name": name,
                "sequential_total_ms": seq_ms,
                "concurrent_total_ms": conc_ms,
                "sequential_per_draw_ms": seq_ms / args.draws_per_arm,
                "concurrent_per_draw_ms": conc_ms / args.draws_per_arm,
                "ratio": conc_ms / seq_ms,
                "added_per_draw_ms": (conc_ms - seq_ms) / args.draws_per_arm,
            }
        )
    if len(comparisons) < 3:
        raise ValueError("fewer than three evidence-based candidate kernels were present")
    comparisons.sort(
        key=lambda item: (item["added_per_draw_ms"], item["concurrent_per_draw_ms"]),
        reverse=True,
    )
    selected = [item["name"] for item in comparisons[:3]]
    args.selected_kernels_out.write_text("\n".join(selected) + "\n")

    ncu: dict[str, object] = {"collected": False, "files": []}
    if args.ncu_dir is not None:
        detail = list(dict.fromkeys((selected[0], DETAIL_REFERENCE_KERNEL)))
        required = [args.ncu_dir / f"{name}-sol.csv" for name in selected]
        required.extend(args.ncu_dir / f"{name}-detail.csv" for name in detail)
        missing = [path for path in required if not path.is_file() or path.stat().st_size == 0]
        if missing:
            raise ValueError("missing NCU CSV: " + ", ".join(str(path) for path in missing))
        validations = []
        for path in required:
            name, report_type = path.name.rsplit("-", 1)[0], path.stem.rsplit("-", 1)[1]
            validations.append(
                {
                    "path": str(path),
                    "kernel": name,
                    "report_type": report_type,
                    **validate_ncu_csv(path, name, report_type),
                }
            )
        ncu = {
            "collected": True,
            "selected_sol_kernels": selected,
            "selected_detail_kernels": detail,
            "files": [
                {"path": str(path), "bytes": path.stat().st_size, "sha256": sha256(path)}
                for path in required
            ],
            "validations": validations,
        }

    concurrent_window = float(concurrent_timing["execution_window_wall_time_ms"])
    d2h = concurrent["d2h"]
    unoverlapped = float(d2h["unoverlapped_ms"])
    small_unoverlapped = float(d2h["size_groups"]["small_lt_1mb"]["unoverlapped_ms"])
    analysis = {
        "schema": "sembla-cuda-readback-diagnostic-v1",
        "expected_scale": args.expected_scale,
        "expected_ticks": args.expected_ticks,
        "draws_per_systems_arm": args.draws_per_arm,
        "phase_timing": {
            "wall_time_ms": phases["totals"]["wall_time_ms"],
            "phases_ms": phases["totals"]["phases_ms"],
            "repository_commit": phases["session"]["repository_commit"],
            "binary_sha256": phases["session"]["binary_sha256"],
        },
        "systems": {
            "sequential": sequential,
            "workers_4": concurrent,
            "sequential_api": analyze_api(args.sequential_api),
            "workers_4_api": analyze_api(args.concurrent_api),
            "sequential_whole_sweep_ms": sequential_timing["whole_sweep_wall_time_ms"],
            "workers_4_whole_sweep_ms": concurrent_timing["whole_sweep_wall_time_ms"],
            "workers_4_execution_window_ms": concurrent_window,
        },
        "kernel_penalties": comparisons,
        "selected_ncu_kernels": selected,
        "ncu": ncu,
        "decision_inputs": {
            "workers_4_unoverlapped_d2h_ms": unoverlapped,
            "workers_4_unoverlapped_d2h_fraction_of_execution_window": (
                unoverlapped / concurrent_window if concurrent_window else 0.0
            ),
            "projected_20_draw_unoverlapped_d2h_ms": (
                unoverlapped / args.draws_per_arm * 20
            ),
            "workers_4_small_copy_unoverlapped_ms": small_unoverlapped,
            "projected_20_draw_small_copy_unoverlapped_ms": (
                small_unoverlapped / args.draws_per_arm * 20
            ),
            "all_d2h_destinations_pageable": set(d2h["destinations"]) == {"Pageable"},
        },
    }
    args.output.write_text(json.dumps(analysis, indent=2, sort_keys=True) + "\n")

    if args.assertions is not None:
        if not ncu["collected"]:
            raise ValueError("final assertions require --ncu-dir")
        lines = [
            "PASS exact 10M grouped CUDA phase timing reconciled for 24 ticks",
            "PASS equal four-draw Systems arms contain identical kernel instances",
            "PASS four-worker Systems arm used at least four CUDA streams",
            "PASS D2H calls, bytes, union, overlap, destinations, and size classes recorded",
            "PASS evidence-selected kernel duration penalties recorded",
            "PASS bounded Nsight Compute SOL/occupancy and detailed stall CSVs recorded",
        ]
        args.assertions.write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

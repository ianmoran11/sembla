#!/usr/bin/env python3
"""Run the frozen 27-command CUDA final-state A/B/C evidence protocol.

This driver is intentionally standard-library-only and has no knobs for the
scientific shape, schedules, thresholds, or command count.  The Hyperstack
collector builds one binary, synthesizes one state, and invokes this file.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
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
from dataclasses import dataclass
from typing import Any, Callable, Iterable

SCHEMA = "sembla-cuda-final-state-decision-protocol-v1"
TIMING_SCHEMAS = {
    1: "sembla-sweep-timing-v3",
    4: "sembla-sweep-concurrency-spike-timing-v3",
}
FINAL_STATE_SCHEMA = "sembla-cuda-final-state-readback-v2"
SELECTOR = "SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE"
RETIRED_SELECTORS = (
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256",
    "SEMBLA_SWEEP_EXPERIMENT_DEVICE_FINAL_SHA256_VERIFY",
)
MODES = {
    "A": "materialized",
    "B": "packed-pageable",
    "C": "packed-pinned",
}
SCALE = 10_000_000
TICKS = 24
SEED = 9009
ARM_TIMEOUT_SECONDS = 1_200
PROFILE_EXPORT_TIMEOUT_SECONDS = 300
RESOURCE_SAMPLE_SECONDS = 0.2
WORKER_ONE_ORDERS = ("ABC", "CBA", "ABC")
WORKER_FOUR_ORDERS = ("CBA", "ABC", "CBA")


class ProtocolError(RuntimeError):
    pass


def atomic_json(path: pathlib.Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    payload = json.dumps(value, indent=2, sort_keys=True) + "\n"
    with tempfile.NamedTemporaryFile("w", dir=path.parent, delete=False) as handle:
        handle.write(payload)
        temporary = pathlib.Path(handle.name)
    temporary.replace(path)


def sha256_file(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_hashes(root: pathlib.Path) -> dict[str, str]:
    if not root.is_dir():
        raise ProtocolError(f"output tree is missing: {root}")
    return {
        path.relative_to(root).as_posix(): sha256_file(path)
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def compare_output_trees(left: pathlib.Path, right: pathlib.Path) -> dict[str, Any]:
    left_hashes = tree_hashes(left)
    right_hashes = tree_hashes(right)
    left_names, right_names = set(left_hashes), set(right_hashes)
    changed = sorted(
        name for name in left_names & right_names if left_hashes[name] != right_hashes[name]
    )
    result = {
        "left": str(left),
        "right": str(right),
        "left_file_count": len(left_hashes),
        "right_file_count": len(right_hashes),
        "missing_from_right": sorted(left_names - right_names),
        "extra_in_right": sorted(right_names - left_names),
        "changed": changed,
    }
    result["equal"] = not (
        result["missing_from_right"] or result["extra_in_right"] or changed
    )
    return result


def final_state_hashes(output_tree: pathlib.Path) -> list[str]:
    path = output_tree / "run-manifest.json"
    try:
        document = json.loads(path.read_text())
        executions = document["executions"]
        hashes = [execution["final_state_sha256"] for execution in executions]
    except (OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        raise ProtocolError(f"invalid sweep manifest {path}: {error}") from error
    if not hashes or any(not isinstance(value, str) or len(value) != 64 for value in hashes):
        raise ProtocolError(f"invalid final_state_sha256 values in {path}")
    return hashes


def _arm_name(index: int, command_class: str, mode: str, workers: int, repetition: int | None) -> str:
    suffix = f"-r{repetition}" if repetition is not None else ""
    return f"{index:02d}-{command_class}-w{workers}-{mode}{suffix}"


def _binary_argv(
    binary: pathlib.Path,
    model: pathlib.Path,
    state: pathlib.Path,
    output: pathlib.Path,
    timing: pathlib.Path,
    draws: int,
    workers: int,
    noise: str,
) -> list[str]:
    return [
        str(binary),
        "sweep",
        str(model),
        "--population",
        str(state),
        "--backend",
        "cuda",
        "--seed",
        str(SEED),
        "--draws",
        str(draws),
        "--draw-workers",
        str(workers),
        "--ticks",
        str(TICKS),
        "--noise",
        noise,
        "--enable",
        "grouped-observations",
        "--timing-json",
        str(timing),
        "--out",
        str(output),
    ]


def build_manifest(
    binary: pathlib.Path,
    model: pathlib.Path,
    state: pathlib.Path,
    evidence: pathlib.Path,
    provenance: dict[str, Any],
) -> dict[str, Any]:
    records: list[dict[str, Any]] = []

    def add(
        command_class: str,
        mode: str,
        workers: int,
        draws: int,
        noise: str,
        repetition: int | None,
        profiled: bool,
        included: bool,
    ) -> None:
        index = len(records) + 1
        name = _arm_name(index, command_class, mode, workers, repetition)
        arm = evidence / "arms" / name
        output = arm / "output"
        timing = arm / "timing.json"
        benchmark_argv = _binary_argv(
            binary, model, state, output, timing, draws, workers, noise
        )
        if profiled:
            report = arm / "profile" / "final-state"
            argv = [
                "nsys",
                "profile",
                "--trace=cuda",
                "--sample=none",
                "--cpuctxsw=none",
                "--stats=false",
                "--force-overwrite=true",
                "-o",
                str(report),
                *benchmark_argv,
            ]
        else:
            report = None
            argv = benchmark_argv
        records.append(
            {
                "id": index,
                "name": name,
                "class": command_class,
                "mode": mode,
                "selector": MODES[mode],
                "workers": workers,
                "draws": draws,
                "ticks": TICKS,
                "slots": SCALE,
                "seed": SEED,
                "noise": noise,
                "repetition": repetition,
                "profiled": profiled,
                "included_in_performance": included,
                "environment": {SELECTOR: MODES[mode]},
                "provenance": {
                    "repository_commit": provenance.get("repository_commit"),
                    "binary_sha256": provenance.get("binary_sha256"),
                    "model_sha256": provenance.get("model_sha256"),
                    "state_sha256": provenance.get("state_sha256"),
                },
                "argv": argv,
                "benchmark_argv": benchmark_argv,
                "arm_dir": str(arm),
                "arm_relative": arm.relative_to(evidence).as_posix(),
                "output_dir": str(output),
                "output_relative": output.relative_to(evidence).as_posix(),
                "timing_json": str(timing),
                "timing_relative": timing.relative_to(evidence).as_posix(),
                "nsys_report": str(report) + ".nsys-rep" if report else None,
                "nsys_report_relative": (
                    pathlib.Path(str(report) + ".nsys-rep").relative_to(evidence).as_posix()
                    if report
                    else None
                ),
            }
        )

    for mode in "ABC":
        add("preflight", mode, 1, 1, "independent", None, False, False)
    for repetition, order in enumerate(WORKER_ONE_ORDERS, 1):
        for mode in order:
            add("timed", mode, 1, 4, "independent", repetition, False, True)
    for repetition, order in enumerate(WORKER_FOUR_ORDERS, 1):
        for mode in order:
            add("timed", mode, 4, 4, "independent", repetition, False, True)
    for mode in "ABC":
        add("crn", mode, 4, 4, "crn", 1, False, False)
    for mode in "ABC":
        add("profile", mode, 4, 4, "independent", 1, True, False)

    manifest = {
        "schema": SCHEMA,
        "frozen": {
            "slots": SCALE,
            "ticks": TICKS,
            "seed": SEED,
            "arm_timeout_seconds": ARM_TIMEOUT_SECONDS,
            "worker_one_orders": list(WORKER_ONE_ORDERS),
            "worker_four_orders": list(WORKER_FOUR_ORDERS),
            "mode_selectors": MODES,
            "execution_count": 27,
        },
        "provenance": provenance,
        "evidence_root_at_collection": str(evidence),
        "executions": records,
    }
    validate_manifest(manifest)
    return manifest


def validate_manifest(manifest: dict[str, Any]) -> None:
    if manifest.get("schema") != SCHEMA:
        raise ProtocolError("unexpected protocol schema")
    records = manifest.get("executions")
    if not isinstance(records, list) or len(records) != 27:
        raise ProtocolError("the focused protocol must contain exactly 27 executions")
    if [record.get("id") for record in records] != list(range(1, 28)):
        raise ProtocolError("execution IDs must be the contiguous range 1..27")
    classes = [record.get("class") for record in records]
    expected_classes = ["preflight"] * 3 + ["timed"] * 18 + ["crn"] * 3 + ["profile"] * 3
    if classes != expected_classes:
        raise ProtocolError("execution classes/order differ from the frozen 3+18+3+3 protocol")
    if "".join(record["mode"] for record in records[:3]) != "ABC":
        raise ProtocolError("preflight mode order must be A-B-C")
    timed = records[3:21]
    expected_groups = [
        (1, 1, order) for order in WORKER_ONE_ORDERS
    ] + [(4, 1, order) for order in WORKER_FOUR_ORDERS]
    offset = 0
    for group_index, (workers, _unused, order) in enumerate(expected_groups):
        group = timed[offset : offset + 3]
        offset += 3
        actual = "".join(record["mode"] for record in group)
        repetition = group_index % 3 + 1
        if actual != order or any(record["workers"] != workers for record in group):
            raise ProtocolError(
                f"timed workers-{workers} repetition {repetition} must use {order}"
            )
        positions = {record["mode"]: index for index, record in enumerate(group)}
        if abs(positions["A"] - positions["B"]) != 1 or abs(positions["B"] - positions["C"]) != 1:
            raise ProtocolError("A/B and B/C must be adjacent in every timed repetition")
    if "".join(record["mode"] for record in records[21:24]) != "ABC":
        raise ProtocolError("CRN mode order must be A-B-C")
    if "".join(record["mode"] for record in records[24:27]) != "ABC":
        raise ProtocolError("profile mode order must be A-B-C")
    provenance = manifest.get("provenance")
    if not isinstance(provenance, dict):
        raise ProtocolError("protocol provenance is missing")
    try:
        binary = pathlib.Path(provenance["binary"])
        model = pathlib.Path(provenance["model"])
        state = pathlib.Path(provenance["state"])
    except (KeyError, TypeError) as error:
        raise ProtocolError("binary/model/state provenance is missing") from error
    for record in records:
        if record["slots"] != SCALE or record["ticks"] != TICKS or record["seed"] != SEED:
            raise ProtocolError("scientific shape drifted from 10M/24 ticks/seed 9009")
        if record["workers"] not in (1, 4) or record["draws"] not in (1, 4):
            raise ProtocolError("only workers 1/4 and draws 1/4 are permitted")
        if record["class"] != "preflight" and record["draws"] != 4:
            raise ProtocolError("every post-preflight execution must use four draws")
        if record["environment"] != {SELECTOR: MODES[record["mode"]]}:
            raise ProtocolError("unexpected environment selector set")
        expected_provenance = {
            "repository_commit": provenance.get("repository_commit"),
            "binary_sha256": provenance.get("binary_sha256"),
            "model_sha256": provenance.get("model_sha256"),
            "state_sha256": provenance.get("state_sha256"),
        }
        if record.get("provenance") != expected_provenance or any(
            not value for value in expected_provenance.values()
        ):
            raise ProtocolError(f"execution {record['id']} provenance is incomplete or inconsistent")
        expected_benchmark = _binary_argv(
            binary,
            model,
            state,
            pathlib.Path(record["output_dir"]),
            pathlib.Path(record["timing_json"]),
            record["draws"],
            record["workers"],
            record["noise"],
        )
        if record.get("benchmark_argv") != expected_benchmark:
            raise ProtocolError(f"execution {record['id']} command differs from the frozen sweep command")
        if record["profiled"]:
            expected_prefix = [
                "nsys", "profile", "--trace=cuda", "--sample=none",
                "--cpuctxsw=none", "--stats=false", "--force-overwrite=true", "-o",
            ]
            if record["argv"][:8] != expected_prefix or record["argv"][9:] != expected_benchmark:
                raise ProtocolError(f"profile execution {record['id']} changed its Nsight wrapper")
        elif record["argv"] != expected_benchmark:
            raise ProtocolError(f"execution {record['id']} unexpectedly wraps a timed command")
        argv_text = " ".join(record["argv"])
        forbidden = ("--draws 20", "--draw-workers 2", "--backend cpu")
        if any(value in argv_text for value in forbidden):
            raise ProtocolError("historical/broader benchmark shape is forbidden")
        if record["profiled"] != (record["class"] == "profile"):
            raise ProtocolError("only the three post-matrix records may be profiled")
        if record["included_in_performance"] != (record["class"] == "timed"):
            raise ProtocolError("only the 18 timed records enter performance aggregation")


def _finite_nonnegative(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise ProtocolError(f"{label} must be numeric")
    number = float(value)
    if not math.isfinite(number) or number < 0:
        raise ProtocolError(f"{label} must be finite and non-negative")
    return number


def validate_timing(path: pathlib.Path, record: dict[str, Any]) -> dict[str, Any]:
    try:
        document = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise ProtocolError(f"invalid timing document {path}: {error}") from error
    if document.get("schema") != TIMING_SCHEMAS[record["workers"]]:
        raise ProtocolError(f"unexpected timing schema in {path}")
    provenance = record.get("provenance", {})
    if document.get("repository_commit") != provenance.get("repository_commit") or document.get(
        "binary_sha256"
    ) != provenance.get("binary_sha256"):
        raise ProtocolError(f"timing binary/commit provenance disagrees with the protocol in {path}")
    if document.get("draws") != record["draws"] or document.get("ticks_per_draw") != TICKS:
        raise ProtocolError(f"timing dimensions disagree with command in {path}")
    _finite_nonnegative(document.get("setup_wall_time_ms"), f"{path}:setup wall")
    whole_wall = _finite_nonnegative(
        document.get("whole_sweep_wall_time_ms"), f"{path}:whole sweep wall"
    )
    if whole_wall <= 0:
        raise ProtocolError(f"whole-sweep wall time must be positive in {path}")
    if record["workers"] == 4:
        _finite_nonnegative(
            document.get("execution_window_wall_time_ms"), f"{path}:execution wall"
        )
        _finite_nonnegative(
            document.get("publication_wall_time_ms"), f"{path}:publication wall"
        )
    draws = document.get("draw_timings")
    if not isinstance(draws, list) or len(draws) != record["draws"]:
        raise ProtocolError(f"missing draw timings in {path}")
    required = (
        "one_time_allocation_ms",
        "cpu_sha256_ms",
        "attributed_phase_sum_ms",
        "unattributed_timer_overhead_ms",
        "final_state_seam_total_ms",
        "timer_tolerance_ms",
    )
    if [draw.get("k") for draw in draws] != list(range(record["draws"])):
        raise ProtocolError(f"draw timing order is not ascending and contiguous in {path}")
    for draw in draws:
        _finite_nonnegative(draw.get("wall_time_ms"), f"{path}:draw wall time")
        final = draw.get("final_state")
        if not isinstance(final, dict) or final.get("schema") != FINAL_STATE_SCHEMA:
            raise ProtocolError(f"missing {FINAL_STATE_SCHEMA} diagnostic in {path}")
        if final.get("mode") != MODES[record["mode"]]:
            raise ProtocolError(f"final-state mode mismatch in {path}")
        for field in required:
            _finite_nonnegative(final.get(field), f"{path}:{field}")
        nullable_phases = (
            "pageable_dtoh_host_api_ms",
            "pinned_dtoh_enqueue_api_ms",
            "wait_to_pinned_host_readable_ms",
            "pinned_to_cacheable_staging_copy_ms",
            "host_state_reconstruction_ms",
        )
        if any(field not in final for field in nullable_phases):
            raise ProtocolError(f"versioned final-state phase fields are missing in {path}")
        for field in nullable_phases:
            if final[field] is not None:
                _finite_nonnegative(final[field], f"{path}:{field}")
        if final.get("final_state_seam_total_excludes_one_time_allocation") is not True:
            raise ProtocolError(f"final-state seam total must exclude allocation in {path}")
        if final.get("phases_reconcile") is not True or final.get(
            "allocation_plus_seam_reconciles_with_draw_wall"
        ) is not True:
            raise ProtocolError(f"timer reconciliation failed in {path}")
        downloaded = final.get("downloaded_bytes")
        if not isinstance(downloaded, dict):
            raise ProtocolError(f"downloaded byte components are missing in {path}")
        components = [downloaded.get(name) for name in ("state", "inputs", "input_counts")]
        if any(not isinstance(value, int) or value < 0 for value in components):
            raise ProtocolError(f"invalid downloaded component bytes in {path}")
        if downloaded.get("total") != sum(components):
            raise ProtocolError(f"downloaded byte total does not reconcile in {path}")
        accounting = final.get("buffer_accounting")
        accounting_fields = (
            "buffer_set_count",
            "underlying_pinned_allocation_count",
            "pinned_bytes",
            "cacheable_staging_bytes",
        )
        if not isinstance(accounting, dict) or any(
            not isinstance(accounting.get(field), int) or accounting[field] < 0
            for field in accounting_fields
        ):
            raise ProtocolError(f"per-draw buffer accounting is missing or malformed in {path}")
        mode = record["mode"]
        pinned_fields = (
            "pinned_dtoh_enqueue_api_ms",
            "wait_to_pinned_host_readable_ms",
            "pinned_to_cacheable_staging_copy_ms",
        )
        if mode == "A":
            if (
                final.get("pageable_dtoh_host_api_ms") is None
                or final.get("host_state_reconstruction_ms") is None
                or any(final.get(field) is not None for field in pinned_fields)
            ):
                raise ProtocolError("A requires only pageable D2H and host reconstruction")
        elif mode == "B":
            if (
                final.get("pageable_dtoh_host_api_ms") is None
                or final.get("host_state_reconstruction_ms") is not None
                or any(final.get(field) is not None for field in pinned_fields)
            ):
                raise ProtocolError("B requires only pageable D2H without reconstruction")
        else:
            if any(
                final.get(field) is None
                for field in (
                    "pinned_dtoh_enqueue_api_ms",
                    "wait_to_pinned_host_readable_ms",
                    "pinned_to_cacheable_staging_copy_ms",
                )
            ) or final.get("pageable_dtoh_host_api_ms") is not None or final.get(
                "host_state_reconstruction_ms"
            ) is not None:
                raise ProtocolError("C requires pinned enqueue/wait/staging and no pageable fallback")
        if mode in ("A", "B"):
            if any(accounting[field] != 0 for field in accounting_fields):
                raise ProtocolError(f"non-pinned draw reported treatment buffers in {path}")
        else:
            nonempty_components = sum(value > 0 for value in components)
            if (
                accounting["buffer_set_count"] != 1
                or accounting["underlying_pinned_allocation_count"] != nonempty_components
                or accounting["pinned_bytes"] != downloaded["total"]
                or accounting["cacheable_staging_bytes"] != downloaded["total"]
            ):
                raise ProtocolError(f"C per-draw buffer accounting is not exact in {path}")
    aggregate = document.get("final_state_buffer_accounting")
    if not isinstance(aggregate, dict):
        raise ProtocolError(f"aggregate buffer accounting is missing in {path}")
    requested_lanes = aggregate.get("requested_lane_count")
    retained = aggregate.get("retained_lane_count")
    requested_sets = aggregate.get("requested_buffer_set_count")
    sets = aggregate.get("buffer_set_count")
    requested_allocations = aggregate.get("requested_underlying_pinned_allocation_count")
    allocations = aggregate.get("underlying_pinned_allocation_count")
    effective_pinned = aggregate.get("effective_pinned_bytes")
    effective_staging = aggregate.get("effective_cacheable_staging_bytes")
    requested_pinned = aggregate.get("requested_pinned_bytes")
    requested_staging = aggregate.get("requested_cacheable_staging_bytes")
    values = (
        requested_lanes,
        retained,
        requested_sets,
        sets,
        requested_allocations,
        allocations,
        effective_pinned,
        effective_staging,
        requested_pinned,
        requested_staging,
    )
    if any(isinstance(value, bool) or not isinstance(value, int) or value < 0 for value in values):
        raise ProtocolError(f"invalid aggregate buffer accounting in {path}")
    if requested_lanes != record["workers"] or retained != record["workers"]:
        raise ProtocolError(f"retained/requested lane count disagrees with the command in {path}")
    if sets > retained or allocations > 3 * retained:
        raise ProtocolError(f"unbounded buffer/allocation counts in {path}")
    if effective_pinned > requested_pinned or effective_staging > requested_staging:
        raise ProtocolError(f"effective treatment bytes exceed admission in {path}")
    if record["mode"] in ("A", "B"):
        if any(
            value != 0
            for value in (
                requested_sets,
                sets,
                requested_allocations,
                allocations,
                requested_pinned,
                effective_pinned,
                requested_staging,
                effective_staging,
            )
        ):
            raise ProtocolError(f"non-pinned mode allocated treatment buffers in {path}")
    else:
        downloaded_per_lane = draws[0]["final_state"]["downloaded_bytes"]["total"]
        nonempty_per_lane = sum(
            draws[0]["final_state"]["downloaded_bytes"][name] > 0
            for name in ("state", "inputs", "input_counts")
        )
        if (
            requested_pinned != downloaded_per_lane * retained
            or requested_staging != downloaded_per_lane * retained
            or requested_sets != retained
            or requested_allocations != nonempty_per_lane * retained
            or not (1 <= sets <= retained)
            or allocations != nonempty_per_lane * sets
            or effective_pinned != downloaded_per_lane * sets
            or effective_staging != downloaded_per_lane * sets
        ):
            raise ProtocolError(f"C aggregate pinned/staging admission is not exact in {path}")
        if record["class"] == "preflight" and (retained != 1 or sets != 1):
            raise ProtocolError("C preflight must report one retained lane and one buffer set")
    return document


def _process_tree(root_pid: int) -> set[int]:
    parents: dict[int, int] = {}
    proc = pathlib.Path("/proc")
    if proc.is_dir():
        for entry in proc.iterdir():
            if not entry.name.isdigit():
                continue
            try:
                fields = (entry / "stat").read_text().split()
                parents[int(entry.name)] = int(fields[3])
            except (OSError, ValueError, IndexError):
                continue
    else:
        try:
            output = subprocess.check_output(
                ["ps", "-axo", "pid=,ppid="], text=True, timeout=5
            )
            for line in output.splitlines():
                pid, parent = line.split()
                parents[int(pid)] = int(parent)
        except (OSError, subprocess.SubprocessError, ValueError):
            parents[root_pid] = 0
    result = {root_pid}
    changed = True
    while changed:
        changed = False
        for pid, parent in parents.items():
            if parent in result and pid not in result:
                result.add(pid)
                changed = True
    return result


def sample_resources(pid: int) -> dict[str, Any]:
    pids = _process_tree(pid)
    rss_kib = 0
    rss_query_succeeded = False
    if pathlib.Path("/proc").is_dir():
        for child in pids:
            try:
                for line in pathlib.Path(f"/proc/{child}/status").read_text().splitlines():
                    if line.startswith("VmRSS:"):
                        rss_kib += int(line.split()[1])
                        rss_query_succeeded = True
                        break
            except (OSError, ValueError):
                pass
    else:
        try:
            output = subprocess.check_output(
                ["ps", "-o", "rss=", "-p", ",".join(map(str, sorted(pids)))],
                text=True,
                timeout=5,
            )
            rss_kib = sum(int(value) for value in output.split())
            rss_query_succeeded = rss_kib > 0
        except (OSError, subprocess.SubprocessError, ValueError):
            pass
    gpu_mib = 0
    vram_query_succeeded = False
    try:
        output = subprocess.check_output(
            [
                "nvidia-smi",
                "--query-compute-apps=pid,used_gpu_memory",
                "--format=csv,noheader,nounits",
            ],
            text=True,
            stderr=subprocess.DEVNULL,
            timeout=5,
        )
        vram_query_succeeded = True
        for row in csv.reader(output.splitlines()):
            if len(row) >= 2 and int(row[0].strip()) in pids:
                gpu_mib += int(float(row[1].strip()))
    except (OSError, subprocess.SubprocessError, ValueError):
        pass
    return {
        "monotonic_seconds": time.monotonic(),
        "process_count": len(pids),
        "rss_bytes": rss_kib * 1024,
        "vram_bytes": gpu_mib * 1024 * 1024,
        "rss_query_succeeded": rss_query_succeeded,
        "vram_query_succeeded": vram_query_succeeded,
    }


def _kill_process_group(process: subprocess.Popen[Any], sig: signal.Signals) -> None:
    try:
        os.killpg(process.pid, sig)
    except ProcessLookupError:
        pass


def run_command(record: dict[str, Any], timeout_seconds: float = ARM_TIMEOUT_SECONDS) -> dict[str, Any]:
    arm = pathlib.Path(record["arm_dir"])
    arm.mkdir(parents=True, exist_ok=True)
    if record.get("profiled"):
        # Nsight does not create a missing parent for -o. When the directory is
        # absent it silently writes a generated report under /tmp, after which
        # the protocol correctly but confusingly reports its declared path as
        # missing. Create the frozen report parent before starting the process.
        pathlib.Path(record["nsys_report"]).parent.mkdir(parents=True, exist_ok=True)
    stdout_path, stderr_path = arm / "stdout.txt", arm / "stderr.txt"
    environment = os.environ.copy()
    for name in RETIRED_SELECTORS:
        environment.pop(name, None)
    environment.update(record["environment"])
    started_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    started = time.monotonic()
    timed_out = False
    samples: list[dict[str, Any]] = []
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        process = subprocess.Popen(
            record["argv"],
            env=environment,
            stdout=stdout,
            stderr=stderr,
            start_new_session=True,
        )
        deadline = started + timeout_seconds
        while process.poll() is None:
            samples.append(sample_resources(process.pid))
            if time.monotonic() >= deadline:
                timed_out = True
                _kill_process_group(process, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    _kill_process_group(process, signal.SIGKILL)
                    process.wait()
                break
            time.sleep(RESOURCE_SAMPLE_SECONDS)
        return_code = process.wait()
    finished = time.monotonic()
    resource_sampling_complete = bool(samples) and any(
        sample["rss_query_succeeded"] and sample["rss_bytes"] > 0 for sample in samples
    ) and any(
        sample["vram_query_succeeded"] and sample["vram_bytes"] > 0 for sample in samples
    )
    effective_return_code = 124 if timed_out else return_code
    if effective_return_code == 0 and not resource_sampling_complete:
        effective_return_code = 125
    result = {
        **record,
        "started_utc": started_utc,
        "finished_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "wall_time_seconds": finished - started,
        "return_code": effective_return_code,
        "timed_out": timed_out,
        "resource_sampling_complete": resource_sampling_complete,
        "resource_sampling_error": (
            None
            if resource_sampling_complete
            else "no positive process RSS and GPU VRAM sample was captured"
        ),
        "stdout": str(stdout_path),
        "stderr": str(stderr_path),
        "resource_samples": samples,
        "peak_rss_bytes": max((sample["rss_bytes"] for sample in samples), default=0),
        "peak_vram_bytes": max((sample["vram_bytes"] for sample in samples), default=0),
    }
    record_path = arm / "record.json"
    atomic_json(record_path, result)
    return result


def export_nsys(record: dict[str, Any]) -> dict[str, str]:
    arm = pathlib.Path(record["arm_dir"])
    report = pathlib.Path(record["nsys_report"])
    if not report.is_file():
        raise ProtocolError(f"missing Nsight report: {report}")
    exports: dict[str, str] = {}
    for report_name in ("cuda_gpu_trace", "cuda_api_sum", "cuda_gpu_kern_sum"):
        output = arm / "profile" / f"{report_name}.csv"
        output.parent.mkdir(parents=True, exist_ok=True)
        command = [
            "nsys",
            "stats",
            "--force-export=true",
            "--report",
            report_name,
            "--format",
            "csv",
            str(report),
        ]
        completed = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=PROFILE_EXPORT_TIMEOUT_SECONDS,
            check=False,
        )
        output.write_bytes(completed.stdout)
        (output.parent / f"{report_name}.stderr").write_bytes(completed.stderr)
        if completed.returncode != 0 or not output.is_file() or output.stat().st_size == 0:
            raise ProtocolError(f"Nsight export {report_name} failed with {completed.returncode}")
        exports[report_name] = (
            pathlib.Path(record["arm_relative"]) / "profile" / f"{report_name}.csv"
        ).as_posix()
    return exports


def _comparison(label: str, records: list[dict[str, Any]]) -> dict[str, Any]:
    by_mode = {record["mode"]: record for record in records}
    if set(by_mode) != set(MODES):
        raise ProtocolError(f"{label} does not contain A/B/C")
    outputs = {mode: pathlib.Path(record["output_dir"]) for mode, record in by_mode.items()}
    ab, ac = compare_output_trees(outputs["A"], outputs["B"]), compare_output_trees(outputs["A"], outputs["C"])
    hashes = {mode: final_state_hashes(path) for mode, path in outputs.items()}
    result = {
        "label": label,
        "record_ids": [record["id"] for record in records],
        "A_B": ab,
        "A_C": ac,
        "final_state_sha256": hashes,
        "tree_parity": ab["equal"] and ac["equal"],
        "digest_parity": hashes["A"] == hashes["B"] == hashes["C"],
    }
    result["passed"] = result["tree_parity"] and result["digest_parity"]
    return result


def negative_control(output_tree: pathlib.Path, root: pathlib.Path) -> dict[str, Any]:
    perturbed = root / "negative-control-tree"
    if perturbed.exists():
        shutil.rmtree(perturbed)
    shutil.copytree(output_tree, perturbed)
    candidates = sorted(path for path in perturbed.rglob("*") if path.is_file())
    if not candidates:
        raise ProtocolError("negative control has no file to perturb")
    target = candidates[0]
    with target.open("ab") as handle:
        handle.write(b"\nDELIBERATE-COMPARATOR-NEGATIVE-CONTROL\n")
    comparison = compare_output_trees(output_tree, perturbed)
    shutil.rmtree(perturbed)
    if comparison["equal"] or not comparison["changed"]:
        raise ProtocolError("output comparator accepted the deliberate negative control")
    return {
        "accepted": False,
        "rejected": True,
        "perturbed_relative_path": target.relative_to(perturbed).as_posix(),
        "comparison": comparison,
    }


Executor = Callable[[dict[str, Any]], dict[str, Any]]


def execute_protocol(
    manifest: dict[str, Any],
    evidence: pathlib.Path,
    executor: Executor = run_command,
    nsys_exporter: Callable[[dict[str, Any]], dict[str, str]] = export_nsys,
) -> dict[str, Any]:
    validate_manifest(manifest)
    evidence.mkdir(parents=True, exist_ok=True)
    atomic_json(evidence / "execution-manifest.json", manifest)
    completed: list[dict[str, Any]] = []
    comparisons: list[dict[str, Any]] = []
    status: dict[str, Any] = {"schema": SCHEMA, "phase": "preflight", "matrix_started": False}
    atomic_json(evidence / "protocol-status.json", status)
    try:
        # The matrix is structurally unreachable until every preflight check,
        # including the deliberate comparator rejection, has passed.
        for record in manifest["executions"][:3]:
            result = executor(record)
            completed.append(result)
            if result.get("return_code") != 0:
                raise ProtocolError(
                    f"preflight execution {record['id']} failed with {result.get('return_code')}"
                )
            validate_timing(pathlib.Path(record["timing_json"]), record)
        preflight = _comparison("preflight-independent", manifest["executions"][:3])
        comparisons.append(preflight)
        if not preflight["passed"]:
            raise ProtocolError("preflight A/B/C output or digest parity failed")
        control = negative_control(
            pathlib.Path(manifest["executions"][0]["output_dir"]), evidence
        )
        atomic_json(evidence / "negative-control.json", control)
        if control["accepted"] or not control["rejected"]:
            raise ProtocolError("negative control was accepted")
        atomic_json(evidence / "comparisons.json", comparisons)
        status.update({"phase": "matrix", "preflight_passed": True, "matrix_started": True})
        atomic_json(evidence / "protocol-status.json", status)

        rest = manifest["executions"][3:]
        group: list[dict[str, Any]] = []
        group_key: tuple[Any, ...] | None = None
        for record in rest:
            result = executor(record)
            completed.append(result)
            if result.get("return_code") != 0:
                raise ProtocolError(
                    f"execution {record['id']} failed with {result.get('return_code')}"
                )
            validate_timing(pathlib.Path(record["timing_json"]), record)
            if record["profiled"]:
                exports = nsys_exporter(record)
                result["nsys_exports"] = exports
                atomic_json(pathlib.Path(record["arm_dir"]) / "record.json", result)
            key = (record["class"], record["workers"], record["repetition"])
            if group_key is None:
                group_key = key
            if key != group_key:
                comparison = _comparison("-".join(map(str, group_key)), group)
                comparisons.append(comparison)
                if not comparison["passed"]:
                    raise ProtocolError(f"A/B/C parity failed for {comparison['label']}")
                group, group_key = [], key
            group.append(record)
            if len(group) == 3:
                comparison = _comparison("-".join(map(str, group_key)), group)
                comparisons.append(comparison)
                if not comparison["passed"]:
                    raise ProtocolError(f"A/B/C parity failed for {comparison['label']}")
                group, group_key = [], None
                atomic_json(evidence / "comparisons.json", comparisons)
        if group:
            raise ProtocolError("incomplete A/B/C execution group")
        status.update({"phase": "complete", "completed": True, "execution_count": len(completed)})
        if len(completed) != 27:
            raise ProtocolError(f"completed {len(completed)} executions, expected 27")
        atomic_json(evidence / "protocol-status.json", status)
        atomic_json(evidence / "comparisons.json", comparisons)
        return status
    except Exception as error:
        status.update(
            {
                "phase": status.get("phase", "unknown"),
                "completed": False,
                "execution_count": len(completed),
                "error": str(error),
            }
        )
        atomic_json(evidence / "protocol-status.json", status)
        atomic_json(evidence / "comparisons.json", comparisons)
        write_checksums(evidence, "SHA256SUMS.partial")
        raise


def write_checksums(root: pathlib.Path, name: str = "SHA256SUMS") -> None:
    lines = []
    for path in sorted(path for path in root.rglob("*") if path.is_file() and path.name != name):
        lines.append(f"{sha256_file(path)}  {path.relative_to(root).as_posix()}")
    (root / name).write_text("\n".join(lines) + "\n")


def provenance(binary: pathlib.Path, model: pathlib.Path, state: pathlib.Path) -> dict[str, Any]:
    def command(*argv: str) -> str:
        completed = subprocess.run(argv, text=True, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return completed.stdout.strip()

    return {
        "repository_commit": command("git", "rev-parse", "HEAD"),
        "repository_status": command("git", "status", "--porcelain=v1", "--untracked-files=all"),
        "binary": str(binary),
        "binary_sha256": sha256_file(binary),
        "model": str(model),
        "model_sha256": sha256_file(model),
        "state": str(state),
        "state_sha256": sha256_file(state),
        "cuda_version": command("nvcc", "--version"),
        "gpu_driver_identity": command(
            "nvidia-smi", "--query-gpu=name,driver_version,uuid,pci.bus_id", "--format=csv,noheader"
        ),
        "counter_restriction_before": command("sh", "-c", "grep RmProfilingAdminOnly /proc/driver/nvidia/params || true"),
        "started_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=pathlib.Path, required=True)
    parser.add_argument("--model", type=pathlib.Path, required=True)
    parser.add_argument("--state", type=pathlib.Path, required=True)
    parser.add_argument("--evidence", type=pathlib.Path, required=True)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    for path in (args.binary, args.model, args.state):
        if not path.is_file():
            raise ProtocolError(f"required input is missing: {path}")
    evidence = args.evidence.resolve()
    if evidence.exists() and any(evidence.iterdir()):
        raise ProtocolError(f"evidence directory is not empty: {evidence}")
    evidence.mkdir(parents=True, exist_ok=True)
    manifest = build_manifest(
        args.binary.resolve(),
        args.model.resolve(),
        args.state.resolve(),
        evidence,
        provenance(args.binary, args.model, args.state),
    )
    execute_protocol(manifest, evidence)
    write_checksums(evidence)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ProtocolError as error:
        print(f"final-state decision protocol failed: {error}", file=sys.stderr)
        raise SystemExit(1)

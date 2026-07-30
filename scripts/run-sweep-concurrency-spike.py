#!/usr/bin/env python3
"""Run direct sequential/concurrent sweep arms and prove output-tree equality.

This evidence driver can exercise either the supported --draw-workers option or
the retained hidden spike seams for closed-design comparisons. It keeps timing
outside scientific output directories,
captures resource samples, compares every output byte, and proves the comparator
with one deliberate perturbation.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time
from typing import Any

SCHEMA = "sembla-sweep-concurrency-spike/v1"
WORKERS_ENV = "SEMBLA_SWEEP_SPIKE_DRAW_WORKERS"
CUDA_LOCKSTEP_ENV = "SEMBLA_SWEEP_SPIKE_CUDA_LOCKSTEP_STREAMS"
CUDA_FREE_ENV = "SEMBLA_SWEEP_SPIKE_CUDA_FREE_STREAMS"
CUDA_FUSED_ENV = "SEMBLA_SWEEP_SPIKE_CUDA_FUSED_DRAWS"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def tree_hashes(output: Path, pairs: Path | None) -> dict[str, str]:
    hashes = {
        str(path.relative_to(output)): sha256(path)
        for path in sorted(output.rglob("*"))
        if path.is_file()
    }
    if pairs is not None:
        for path in [pairs, Path(f"{pairs}.meta.json")]:
            if path.exists():
                hashes[f"__external__/{path.name}"] = sha256(path)
    return hashes


def compare_hashes(reference: dict[str, str], candidate: dict[str, str]) -> dict[str, Any]:
    reference_files = set(reference)
    candidate_files = set(candidate)
    changed = sorted(
        path
        for path in reference_files & candidate_files
        if reference[path] != candidate[path]
    )
    return {
        "equal": not (reference_files ^ candidate_files) and not changed,
        "missing": sorted(reference_files - candidate_files),
        "extra": sorted(candidate_files - reference_files),
        "changed": changed,
    }


def read_int(command: list[str]) -> int | None:
    try:
        value = subprocess.check_output(command, text=True, stderr=subprocess.DEVNULL).strip()
        return int(value) if value else None
    except (FileNotFoundError, subprocess.CalledProcessError, ValueError):
        return None


def monitor_process(
    process: subprocess.Popen[bytes], samples: list[dict[str, Any]], started_ns: int
) -> None:
    while process.poll() is None:
        sample: dict[str, Any] = {
            "offset_ms": (time.monotonic_ns() - started_ns) / 1_000_000
        }
        rss = read_int(["ps", "-o", "rss=", "-p", str(process.pid)])
        if rss is not None:
            sample["process_rss_kib"] = rss
        if shutil.which("nvidia-smi"):
            try:
                raw = subprocess.check_output(
                    [
                        "nvidia-smi",
                        "--query-gpu=utilization.gpu,memory.used,memory.total",
                        "--format=csv,noheader,nounits",
                    ],
                    text=True,
                    stderr=subprocess.DEVNULL,
                ).strip().splitlines()[0]
                utilization, used, total = [int(value.strip()) for value in raw.split(",")]
                sample.update(
                    {
                        "gpu_utilization_percent": utilization,
                        "gpu_memory_used_mib": used,
                        "gpu_memory_total_mib": total,
                    }
                )
            except (IndexError, subprocess.CalledProcessError, ValueError):
                pass
        samples.append(sample)
        time.sleep(0.25)


def run_arm(
    args: argparse.Namespace,
    workers: int,
    repetition: int,
    sequential_reference: bool = False,
) -> dict[str, Any]:
    label = "sequential-reference" if sequential_reference else f"workers-{workers}"
    arm = args.output_root / label / f"rep-{repetition}"
    output = arm / "output"
    timing = arm / "timing.json"
    pairs = arm / "pairs.csv" if args.export_pairs else None
    arm.mkdir(parents=True, exist_ok=False)

    command = [
        str(args.binary),
        "sweep",
        str(args.model),
        "--population",
        str(args.population),
        "--seed",
        str(args.seed),
        "--ticks",
        str(args.ticks),
        "--noise",
        args.noise,
        "--backend",
        args.backend,
        "--out",
        str(output),
        "--timing-json",
        str(timing),
    ]
    if args.theta_file:
        command.extend(["--theta-file", str(args.theta_file)])
    else:
        command.extend(["--draws", str(args.draws)])
    if args.params:
        command.extend(["--params", str(args.params)])
    if pairs:
        command.extend(["--export-pairs", str(pairs)])
    for feature in args.enable:
        command.extend(["--enable", feature])
    if args.supported_draw_workers:
        command.extend(["--draw-workers", str(workers)])

    environment = os.environ.copy()
    if args.supported_draw_workers:
        environment.pop(WORKERS_ENV, None)
        environment.pop(CUDA_FREE_ENV, None)
    else:
        environment[WORKERS_ENV] = "1" if args.cuda_fused_grid_y else str(workers)
    if args.cuda_fused_grid_y and not sequential_reference:
        environment[CUDA_FUSED_ENV] = str(workers)
    else:
        environment.pop(CUDA_FUSED_ENV, None)
    if args.cuda_lockstep_streams and workers > 1:
        environment[CUDA_LOCKSTEP_ENV] = "1"
    else:
        environment.pop(CUDA_LOCKSTEP_ENV, None)
    if args.cuda_free_streams and workers > 1:
        environment[CUDA_FREE_ENV] = "1"
    else:
        environment.pop(CUDA_FREE_ENV, None)
    eval_threads = None
    if args.backend == "cpu":
        if args.cpu_total_threads is None:
            raise RuntimeError("--cpu-total-threads is required for CPU arms")
        eval_threads = max(1, args.cpu_total_threads // workers)
        environment["SEMBLA_EVAL_THREADS"] = str(eval_threads)

    stdout_path = arm / "stdout.txt"
    stderr_path = arm / "stderr.txt"
    samples: list[dict[str, Any]] = []
    started = time.monotonic_ns()
    with stdout_path.open("wb") as stdout, stderr_path.open("wb") as stderr:
        process = subprocess.Popen(command, env=environment, stdout=stdout, stderr=stderr)
        monitor = threading.Thread(
            target=monitor_process, args=(process, samples, started), daemon=True
        )
        monitor.start()
        return_code = process.wait()
        monitor.join()
    wall_ms = (time.monotonic_ns() - started) / 1_000_000
    record: dict[str, Any] = {
        "workers": workers,
        "repetition": repetition,
        "arm_kind": "sequential-reference" if sequential_reference else "candidate",
        "eval_threads_per_draw": eval_threads,
        "supported_draw_workers": args.supported_draw_workers,
        "cuda_lockstep_streams": args.cuda_lockstep_streams and workers > 1,
        "cuda_free_streams": args.cuda_free_streams and workers > 1,
        "cuda_fused_grid_y": args.cuda_fused_grid_y and not sequential_reference,
        "requested_fused_capacity": (
            workers if args.cuda_fused_grid_y and not sequential_reference else None
        ),
        "command": command,
        "return_code": return_code,
        "external_wall_time_ms": wall_ms,
        "stdout_sha256": sha256(stdout_path),
        "stderr_sha256": sha256(stderr_path),
        "resource_samples": samples,
        "peak_process_rss_kib": max(
            (sample.get("process_rss_kib", 0) for sample in samples), default=0
        ),
        "peak_gpu_memory_used_mib": max(
            (sample.get("gpu_memory_used_mib", 0) for sample in samples), default=0
        ),
        "peak_gpu_utilization_percent": max(
            (sample.get("gpu_utilization_percent", 0) for sample in samples), default=0
        ),
    }
    if return_code == 0:
        record["timing"] = json.loads(timing.read_text())
        record["output_hashes"] = tree_hashes(output, pairs)
    else:
        record["stderr_tail"] = stderr_path.read_text(errors="replace").splitlines()[-40:]
    return record


def negative_control(reference_output: Path, root: Path) -> dict[str, Any]:
    destination = root / "negative-control"
    shutil.copytree(reference_output, destination)
    candidates = sorted(destination.glob("*.grouped.*.csv"))
    if not candidates:
        candidates = sorted(destination.glob("draw_*.csv"))
    if not candidates:
        raise RuntimeError("negative control found no scientific CSV to perturb")
    target = candidates[0]
    with target.open("ab") as output:
        output.write(b"# deliberate comparator perturbation\n")
    comparison = compare_hashes(
        tree_hashes(reference_output, None), tree_hashes(destination, None)
    )
    if comparison["equal"]:
        raise RuntimeError("negative-control perturbation did not make the comparator fail")
    return {"target": str(target.relative_to(destination)), "comparison": comparison}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, default=Path("target/release/sembla"))
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--population", type=Path, required=True)
    parser.add_argument("--backend", choices=["cpu", "cuda"], required=True)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--draws", type=int, default=20)
    parser.add_argument("--theta-file", type=Path)
    parser.add_argument("--ticks", type=int, default=24)
    parser.add_argument("--seed", type=int, default=9009)
    parser.add_argument("--noise", choices=["crn", "independent"], default="independent")
    parser.add_argument("--workers", type=int, nargs="+", default=[1, 2, 4])
    parser.add_argument("--repetitions", type=int, default=1)
    parser.add_argument("--cpu-total-threads", type=int)
    parser.add_argument("--params", type=Path)
    parser.add_argument("--export-pairs", action="store_true")
    parser.add_argument("--supported-draw-workers", action="store_true")
    parser.add_argument("--cuda-lockstep-streams", action="store_true")
    parser.add_argument("--cuda-free-streams", action="store_true")
    parser.add_argument("--cuda-fused-grid-y", action="store_true")
    parser.add_argument("--enable", action="append", default=[])
    args = parser.parse_args()
    if args.draws <= 0 or args.ticks <= 0 or args.repetitions <= 0:
        parser.error("--draws, --ticks, and --repetitions must be positive")
    if any(workers <= 0 for workers in args.workers):
        parser.error("every --workers value must be positive")
    if len(set(args.workers)) != len(args.workers):
        parser.error("--workers values must be unique")
    if 1 not in args.workers:
        parser.error("--workers must include the sequential reference value 1")
    if args.theta_file and "--draws" in sys.argv:
        parser.error("--theta-file conflicts with --draws")
    if args.backend == "cpu" and not args.cpu_total_threads:
        parser.error("--cpu-total-threads is required for --backend cpu")
    if args.supported_draw_workers and args.backend != "cuda":
        parser.error("--supported-draw-workers requires --backend cuda")
    if args.supported_draw_workers and (
        args.cuda_lockstep_streams or args.cuda_free_streams or args.cuda_fused_grid_y
    ):
        parser.error(
            "--supported-draw-workers conflicts with hidden lockstep, free-stream, and fused modes"
        )
    if args.cuda_lockstep_streams and args.cuda_fused_grid_y:
        parser.error("--cuda-lockstep-streams conflicts with --cuda-fused-grid-y")
    if args.cuda_free_streams and args.cuda_lockstep_streams:
        parser.error("--cuda-free-streams conflicts with --cuda-lockstep-streams")
    if args.cuda_free_streams and args.cuda_fused_grid_y:
        parser.error("--cuda-free-streams conflicts with --cuda-fused-grid-y")
    if args.cuda_free_streams and args.backend != "cuda":
        parser.error("--cuda-free-streams requires --backend cuda")
    if args.cuda_fused_grid_y and args.backend != "cuda":
        parser.error("--cuda-fused-grid-y requires --backend cuda")
    if args.cuda_fused_grid_y and any(workers not in (1, 2, 4) for workers in args.workers):
        parser.error("--cuda-fused-grid-y capacities must be 1, 2, or 4")
    if args.cuda_lockstep_streams and args.backend != "cuda":
        parser.error("--cuda-lockstep-streams requires --backend cuda")
    if args.cuda_lockstep_streams and args.theta_file:
        parser.error("--cuda-lockstep-streams does not support --theta-file in this spike")
    if args.cuda_lockstep_streams and any(
        args.draws % workers for workers in args.workers if workers > 1
    ):
        parser.error("--draws must be divisible by every lockstep worker count")
    if args.output_root.exists():
        parser.error(f"--output-root already exists: {args.output_root}")
    for path in [args.binary, args.model, args.population, args.theta_file, args.params]:
        if path is not None and not path.exists():
            parser.error(f"path does not exist: {path}")
    args.binary = args.binary.resolve()
    args.model = args.model.resolve()
    args.population = args.population.resolve()
    args.output_root = args.output_root.resolve()
    args.theta_file = args.theta_file.resolve() if args.theta_file else None
    args.params = args.params.resolve() if args.params else None
    return args


def main() -> int:
    args = parse_args()
    args.output_root.mkdir(parents=True)
    document: dict[str, Any] = {
        "schema": SCHEMA,
        "repository_commit": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        "worktree_status": subprocess.check_output(
            ["git", "status", "--short"], text=True
        ).splitlines(),
        "binary": str(args.binary),
        "binary_sha256": sha256(args.binary),
        "model": str(args.model),
        "model_sha256": sha256(args.model),
        "population": str(args.population),
        "population_sha256": sha256(args.population),
        "backend": args.backend,
        "draws": None if args.theta_file else args.draws,
        "theta_file": str(args.theta_file) if args.theta_file else None,
        "ticks": args.ticks,
        "seed": args.seed,
        "noise": args.noise,
        "workers": args.workers,
        "repetitions": args.repetitions,
        "cpu_total_threads": args.cpu_total_threads,
        "supported_draw_workers": args.supported_draw_workers,
        "cuda_lockstep_streams": args.cuda_lockstep_streams,
        "cuda_free_streams": args.cuda_free_streams,
        "cuda_fused_grid_y": args.cuda_fused_grid_y,
        "enabled_features": args.enable,
        "runs": [],
    }
    summary_path = args.output_root / "summary.json"
    try:
        for repetition in range(args.repetitions):
            if args.cuda_fused_grid_y:
                reference = run_arm(args, 1, repetition, sequential_reference=True)
                document["runs"].append(reference)
                summary_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
                if reference["return_code"] != 0:
                    raise RuntimeError(
                        f"sequential reference repetition={repetition} failed; see {summary_path}"
                    )
            for workers in args.workers:
                record = run_arm(args, workers, repetition)
                document["runs"].append(record)
                summary_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
                if record["return_code"] != 0:
                    raise RuntimeError(
                        f"workers={workers} repetition={repetition} failed; see {summary_path}"
                    )

        references = {
            run["repetition"]: run
            for run in document["runs"]
            if (
                run["arm_kind"] == "sequential-reference"
                if args.cuda_fused_grid_y
                else run["workers"] == 1
            )
        }
        comparisons = []
        for run in document["runs"]:
            reference = references[run["repetition"]]
            comparison = compare_hashes(reference["output_hashes"], run["output_hashes"])
            comparisons.append(
                {
                    "repetition": run["repetition"],
                    "workers": run["workers"],
                    "arm_kind": run["arm_kind"],
                    **comparison,
                }
            )
            if not comparison["equal"]:
                raise RuntimeError(
                    f"workers={run['workers']} repetition={run['repetition']} changed outputs"
                )
        document["comparisons"] = comparisons

        reference_arm = args.output_root / (
            "sequential-reference" if args.cuda_fused_grid_y else "workers-1"
        ) / "rep-0"
        document["negative_control"] = negative_control(
            reference_arm / "output", args.output_root
        )
        document["passed"] = True
    except Exception as error:  # retain partial evidence on every failure
        document["passed"] = False
        document["error"] = str(error)
        summary_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
        print(error, file=sys.stderr)
        return 1

    summary_path.write_text(json.dumps(document, indent=2, sort_keys=True) + "\n")
    print(f"PASS: output trees identical; evidence: {summary_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

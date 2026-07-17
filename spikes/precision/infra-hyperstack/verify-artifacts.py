#!/usr/bin/env python3
"""Reject incomplete or cross-machine Hyperstack decision artifacts."""

from __future__ import annotations

import base64
import binascii
import hashlib
import json
import math
import re
import sys
from pathlib import Path

BEGIN = "<!-- sembla-precision-state-v1:begin -->"
END = "<!-- sembla-precision-state-v1:end -->"
STRATEGIES = ["f32", "double-single", "native f64 (wgpu)", "native f64 (CUDA)"]
EXPECTED_WORKLOAD = {
    "requested_rows": 26_000_000,
    "requested_groups": 1_300_000,
    "actual_rows": 26_000_000,
    "actual_groups": 1_300_000,
    "benchmark_tick": 7,
    "warmup_ticks": 10,
    "measured_ticks": 100,
    "beta": 0.35,
    "dt": 0.25,
}


def fail(message: str) -> None:
    raise SystemExit(f"artifact verification failed: {message}")


def embedded_state(path: Path) -> dict:
    document = path.read_text(encoding="utf-8")
    if document.count(BEGIN) != 1 or document.count(END) != 1:
        fail(f"{path.name} does not contain exactly one embedded state block")
    encoded = document.split(BEGIN, 1)[1].split(END, 1)[0].strip()
    match = re.fullmatch(r"```json\s*(.*?)\s*```", encoded, re.DOTALL)
    if not match:
        fail(f"{path.name} embedded state is not one JSON code fence")
    try:
        return json.loads(match.group(1))
    except json.JSONDecodeError as error:
        fail(f"{path.name} embedded state is invalid JSON: {error}")


def ed25519_fingerprint(path: Path) -> str:
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split()
        if "ssh-ed25519" not in fields:
            continue
        index = fields.index("ssh-ed25519")
        if index + 1 >= len(fields):
            break
        try:
            key_blob = base64.b64decode(fields[index + 1], validate=True)
        except (ValueError, binascii.Error):
            break
        digest = base64.b64encode(hashlib.sha256(key_blob).digest()).decode("ascii")
        return f"SHA256:{digest.rstrip('=')}"
    fail(f"{path.name} does not contain one valid ED25519 public key")


def exact_gpu_token(text: str, expected: str) -> bool:
    return re.search(
        rf"(?:^|[^A-Za-z0-9]){re.escape(expected)}(?:[^A-Za-z0-9]|$)",
        text,
        re.IGNORECASE,
    ) is not None


def require_count(metrics: dict, key: str, label: str) -> None:
    value = metrics.get(key)
    if not isinstance(value, int) or isinstance(value, bool) or value < 0:
        fail(f"{label} is missing valid non-negative diagnostic {key}")


def run_fingerprint(machine: dict) -> dict:
    hardware = machine["hardware"]
    fp64 = hardware["fp64"]
    infrastructure = machine["infrastructure"]
    workload = machine["workload"]
    return {
        "adapter_name": hardware.get("adapter_name"),
        "backend": hardware.get("backend"),
        "device_type": hardware.get("device_type"),
        "driver": hardware.get("driver"),
        "driver_info": hardware.get("driver_info"),
        "shader_f64": hardware.get("shader_f64"),
        "strict_math": hardware.get("strict_math"),
        "gpu_model": fp64.get("gpu_model"),
        "fp64_class": fp64.get("class"),
        "fp64_ratio": fp64.get("fp32_to_fp64_ratio"),
        "workload": workload,
        "repository_commit": infrastructure.get("repository-commit"),
        "provider": infrastructure.get("provider"),
        "region": infrastructure.get("hyperstack-region"),
        "environment": infrastructure.get("hyperstack-environment"),
        "flavor": infrastructure.get("hyperstack-flavor"),
        "image": infrastructure.get("hyperstack-image"),
        "expected_gpu": infrastructure.get("expected-gpu"),
        "nvidia_device": infrastructure.get("nvidia-device"),
        "requested_fp64_class": infrastructure.get("requested-fp64-class"),
        "strategy_availability": [
            (row.get("strategy"), row.get("status", {}).get("status"))
            for row in machine.get("strategies", [])
        ],
        "guard_availability": [
            (
                strategy,
                "unavailable"
                if machine.get("guards", {}).get(strategy, {}).get("status") == "unavailable"
                else "available",
            )
            for strategy in STRATEGIES
        ],
    }


def verify_run(path: Path, profile: dict, commit: str) -> tuple[dict, dict, str, str]:
    state = embedded_state(path)
    version = state.get("version")
    if not isinstance(version, int) or isinstance(version, bool) or version != 1:
        fail(f"{path.name} embedded state version is not supported version 1")
    machine = state.get("machines", {}).get("nvidia")
    if not isinstance(machine, dict):
        fail(f"{path.name} has no machines.nvidia state")

    hardware = machine.get("hardware", {})
    fp64 = hardware.get("fp64", {})
    infrastructure = machine.get("infrastructure", {})
    workload = machine.get("workload", {})
    expected_gpu = profile["expected_gpu"]

    if hardware.get("backend") != "Vulkan":
        fail(f"{path.name} backend is not Vulkan: {hardware.get('backend')!r}")
    if hardware.get("device_type") != "DiscreteGpu":
        fail(f"{path.name} did not use a discrete GPU: {hardware.get('device_type')!r}")
    if not exact_gpu_token(str(hardware.get("adapter_name", "")), expected_gpu):
        fail(f"{path.name} adapter does not contain exact {expected_gpu} token")
    if not hardware.get("driver") and not hardware.get("driver_info"):
        fail(f"{path.name} has no GPU driver provenance")
    if fp64.get("class") != "full-rate" or fp64.get("full_rate_extrapolation") is not True:
        fail(f"{path.name} runtime fp64 classification is not verified full-rate")
    if not exact_gpu_token(str(fp64.get("gpu_model", "")), expected_gpu):
        fail(f"{path.name} fp64 metadata does not contain exact {expected_gpu} token")
    if not isinstance(hardware.get("strict_math", {}).get("trustworthy"), bool):
        fail(f"{path.name} strict-math trust result is missing")

    for key, expected in EXPECTED_WORKLOAD.items():
        if workload.get(key) != expected:
            fail(f"{path.name} workload {key}={workload.get(key)!r}, expected {expected!r}")

    expected_infra = {
        "repository-commit": commit,
        "provider": "hyperstack",
        "hyperstack-region": profile["region"],
        "hyperstack-environment": profile["environment"],
        "hyperstack-flavor": profile["flavor"],
        "hyperstack-image": profile["image"],
        "expected-gpu": expected_gpu,
        "requested-fp64-class": "full-rate",
    }
    for key, expected in expected_infra.items():
        if infrastructure.get(key) != expected:
            fail(
                f"{path.name} infrastructure {key}={infrastructure.get(key)!r}, "
                f"expected {expected!r}"
            )
    if not exact_gpu_token(str(infrastructure.get("nvidia-device", "")), expected_gpu):
        fail(f"{path.name} nvidia-device does not contain exact {expected_gpu} token")

    guards = machine.get("guards")
    if not isinstance(guards, dict) or set(guards) != set(STRATEGIES):
        fail(f"{path.name} guard evidence does not contain exactly the four strategies")
    for strategy in STRATEGIES:
        guard = guards[strategy]
        status = guard.get("status") if isinstance(guard, dict) else None
        if status not in {"passed", "failed", "unavailable"}:
            fail(f"{path.name} strategy {strategy} has invalid guard status: {guard!r}")
        if status != "passed" and not str(guard.get("reason", "")).strip():
            fail(f"{path.name} strategy {strategy} guard has no failure/unavailable reason")

    rows = machine.get("strategies")
    if not isinstance(rows, list) or [row.get("strategy") for row in rows] != STRATEGIES:
        fail(f"{path.name} strategy rows are missing or out of order")
    for row in rows:
        strategy = row["strategy"]
        status = row.get("status", {})
        if status.get("status") != "answered":
            if status.get("status") != "unanswered" or not status.get("reason"):
                fail(f"{path.name} optional strategy {strategy} has invalid status: {status}")
            continue
        timing = status.get("timing", {})
        accuracy = status.get("accuracy", {})
        total_ms = timing.get("total_ms")
        if (
            not isinstance(total_ms, (int, float))
            or isinstance(total_ms, bool)
            or not math.isfinite(total_ms)
            or total_ms <= 0
        ):
            fail(f"{path.name} strategy {strategy} has invalid total_ms")
        for key in (
            "reduction_max_relative_error",
            "reduction_mean_relative_error",
            "winner_mismatch_fraction",
        ):
            value = accuracy.get(key)
            if (
                not isinstance(value, (int, float))
                or isinstance(value, bool)
                or not math.isfinite(value)
                or value < 0
            ):
                fail(f"{path.name} strategy {strategy} has invalid {key}")
        require_count(accuracy, "fired_mismatch_count", f"{path.name} strategy {strategy}")
        if strategy.startswith("native f64"):
            require_count(
                accuracy,
                "unexplained_arithmetic_mirror_difference_count",
                f"{path.name} strategy {strategy}",
            )
            verdict = str(row.get("verdict", ""))
            if "classified `full-rate`" not in verdict or not exact_gpu_token(verdict, expected_gpu):
                fail(f"{path.name} strategy {strategy} lacks full-rate runtime provenance")

    run_id = infrastructure.get("run-id")
    if not isinstance(run_id, str) or not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", run_id):
        fail(f"{path.name} has no valid infrastructure run-id")
    generated_at = machine.get("generated_at")
    if not isinstance(generated_at, str) or not re.fullmatch(
        r"unix-seconds:[0-9]+", generated_at
    ):
        fail(f"{path.name} has no valid generated_at identity")

    return state, run_fingerprint(machine), run_id, generated_at


def main() -> None:
    if len(sys.argv) != 2:
        fail("usage: verify-artifacts.py ARTIFACT_DIRECTORY")
    directory = Path(sys.argv[1])
    if not directory.is_dir():
        fail(f"not a directory: {directory}")

    profile_path = directory / "selected-profile.json"
    commit_path = directory / "repository-commit.txt"
    gpu_path = directory / "nvidia-smi-q.txt"
    bootstrap_path = directory / "bootstrap.log"
    bootstrap_diagnostics_path = directory / "bootstrap-diagnostics.log"
    trusted_fingerprint_path = directory / "trusted-ssh-host-fingerprint.txt"
    scanned_host_key_path = directory / "ssh-host-key.pub"
    self_test_key_path = directory / "ssh-self-test.pub"
    for path in (
        profile_path,
        commit_path,
        gpu_path,
        bootstrap_path,
        bootstrap_diagnostics_path,
        trusted_fingerprint_path,
        scanned_host_key_path,
        self_test_key_path,
    ):
        if not path.is_file() or path.stat().st_size == 0:
            fail(f"missing or empty {path.name}")

    bootstrap_log = bootstrap_path.read_text(encoding="utf-8", errors="replace")
    if "Sembla precision bootstrap ready" not in bootstrap_log:
        fail("bootstrap.log does not contain the ready marker")
    bootstrap_diagnostics = bootstrap_diagnostics_path.read_text(
        encoding="utf-8", errors="replace"
    ).lower()
    for required in (
        "passwordauthentication no",
        "kbdinteractiveauthentication no",
        "gssapiauthentication no",
        "logingracetime 30",
    ):
        if required not in bootstrap_diagnostics:
            fail(f"bootstrap-diagnostics.log does not prove {required!r}")

    trusted_fingerprint = trusted_fingerprint_path.read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"SHA256:[A-Za-z0-9+/]+={0,2}", trusted_fingerprint):
        fail("trusted-ssh-host-fingerprint.txt is not one SHA256 fingerprint")
    for path in (scanned_host_key_path, self_test_key_path):
        if ed25519_fingerprint(path) != trusted_fingerprint:
            fail(f"{path.name} does not match the independently trusted VNC fingerprint")

    profile = json.loads(profile_path.read_text(encoding="utf-8"))
    commit = commit_path.read_text(encoding="utf-8").strip()
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        fail("repository-commit.txt is not one lowercase 40-hex commit")
    if commit != profile.get("repository_ref"):
        fail("retrieved repository commit differs from selected_profile.repository_ref")

    fingerprints = []
    run_ids = []
    generated_at_values = []
    result_hashes = []
    for index in range(1, 4):
        result_path = directory / f"RESULTS.run-{index}.md"
        log_path = directory / f"run-{index}.log"
        if not result_path.is_file() or result_path.stat().st_size == 0:
            fail(f"missing or empty {result_path.name}")
        if not log_path.is_file() or log_path.stat().st_size == 0:
            fail(f"missing or empty {log_path.name}")
        _, fingerprint, run_id, generated_at = verify_run(result_path, profile, commit)
        result_sha256 = hashlib.sha256(result_path.read_bytes()).hexdigest()
        log = log_path.read_text(encoding="utf-8", errors="replace")
        start = re.findall(
            rf"^SEMBLA_RUN_START run_id={re.escape(run_id)} "
            rf"started_at=[^ ]+ repository_commit={commit}$",
            log,
            re.MULTILINE,
        )
        complete = re.findall(
            rf"^SEMBLA_RUN_COMPLETE run_id={re.escape(run_id)} "
            rf"result_sha256={result_sha256} repository_commit={commit}$",
            log,
            re.MULTILINE,
        )
        if len(start) != 1 or len(complete) != 1:
            fail(
                f"{log_path.name} is not the unique complete transcript for {result_path.name}"
            )
        fingerprints.append(fingerprint)
        run_ids.append(run_id)
        generated_at_values.append(generated_at)
        result_hashes.append(result_sha256)

    if len(set(run_ids)) != 3:
        fail(f"run identities are not distinct: {run_ids}")
    if len(set(generated_at_values)) != 3:
        fail(f"generated_at identities are not distinct: {generated_at_values}")
    if len(set(result_hashes)) != 3:
        fail("result files are not three distinct completed invocations")
    baseline = fingerprints[0]
    for index, fingerprint in enumerate(fingerprints[1:], start=2):
        if fingerprint != baseline:
            fail(f"run {index} machine/workload/provenance differs from run 1")

    print(
        "Artifact verification passed for three distinct, same-machine full-rate NVIDIA runs."
    )


if __name__ == "__main__":
    main()

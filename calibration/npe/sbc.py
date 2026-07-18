#!/usr/bin/env python3
"""Run the fixed simulation-based-calibration gate for the NPE reference."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any

KS_P_VALUE_THRESHOLD = 0.01


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def run_sbc_gate(args: argparse.Namespace) -> dict[str, Any]:
    for variable in (
        "OMP_NUM_THREADS",
        "MKL_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "NUMEXPR_NUM_THREADS",
    ):
        os.environ[variable] = str(args.threads)
    try:
        import torch
        from scipy.stats import kstest, uniform
        from sbi.diagnostics import run_sbc
    except ImportError as error:
        raise RuntimeError(
            "NPE dependencies unavailable; install the exact versions from "
            "calibration/npe/requirements.txt"
        ) from error

    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(args.threads)
    torch.use_deterministic_algorithms(True)
    payload = torch.load(args.model, map_location="cpu", weights_only=False)
    posterior = payload["posterior"]
    theta_sbc = payload["theta_sbc"]
    x_sbc = payload["x_sbc"]
    parameter_columns = tuple(payload["parameter_columns"])
    rank_seed = int(payload["sbc_seed"])
    num_posterior_samples = int(payload["sbc_posterior_samples"])
    if theta_sbc.shape[0] < 100:
        raise ValueError(
            f"SBC requires at least 100 rank statistics per parameter; got {theta_sbc.shape[0]}"
        )

    torch.manual_seed(rank_seed)
    ranks, _ = run_sbc(
        theta_sbc,
        x_sbc,
        posterior,
        num_posterior_samples=num_posterior_samples,
        num_workers=1,
        show_progress_bar=False,
        use_batched_sampling=True,
    )
    ranks_cpu = ranks.detach().cpu()

    per_parameter: dict[str, dict[str, Any]] = {}
    for index, name in enumerate(parameter_columns):
        parameter_ranks = ranks_cpu[:, index].numpy()
        p_value = float(
            kstest(
                parameter_ranks,
                uniform(loc=0, scale=num_posterior_samples).cdf,
            ).pvalue
        )
        per_parameter[name] = {
            "ks_p_value": p_value,
            "ks_threshold": KS_P_VALUE_THRESHOLD,
            "pass": p_value > KS_P_VALUE_THRESHOLD,
            "rank_count": int(parameter_ranks.shape[0]),
            "ranks": [int(value) for value in parameter_ranks],
        }

    sbc_pass = all(
        result["rank_count"] >= 100 and result["pass"]
        for result in per_parameter.values()
    )
    diagnostics_path = Path(args.diagnostics)
    diagnostics = json.loads(diagnostics_path.read_text(encoding="utf-8"))
    diagnostics["sbc"] = {
        "ks_threshold": KS_P_VALUE_THRESHOLD,
        "minimum_rank_statistics": 100,
        "num_posterior_samples_per_rank": num_posterior_samples,
        "parameters": per_parameter,
        "pass": sbc_pass,
        "status": "answered",
    }
    diagnostics["pass"] = bool(diagnostics["recovery"]["pass"] and sbc_pass)
    write_json(diagnostics_path, diagnostics)
    return diagnostics


def parser() -> argparse.ArgumentParser:
    artifacts = Path(__file__).resolve().parent / "artifacts" / "run"
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--model", default=artifacts / "posterior.pt", type=Path)
    result.add_argument(
        "--diagnostics", default=artifacts / "diagnostics.json", type=Path
    )
    result.add_argument("--threads", default=1, type=int)
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        diagnostics = run_sbc_gate(args)
    except (OSError, KeyError, RuntimeError, ValueError) as error:
        raise SystemExit(f"error: {error}") from error
    if not diagnostics["pass"]:
        raise SystemExit("error: recovery or SBC acceptance threshold failed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

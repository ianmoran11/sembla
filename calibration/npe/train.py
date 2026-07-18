#!/usr/bin/env python3
"""Train the quarantined, single-round Sembla NPE reference posterior."""

from __future__ import annotations

import argparse
import csv
import json
import os
import random
from pathlib import Path
from typing import Any

from contract import ContractError, PairsArtifact, load_pairs

MEAN_ABSOLUTE_TOLERANCES = {"beta": 0.25, "gamma": 0.05}


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def recovery_tolerances(parameters: tuple[str, ...]) -> dict[str, float]:
    missing = [name for name in parameters if name not in MEAN_ABSOLUTE_TOLERANCES]
    if missing:
        raise ValueError(
            "the reference recovery threshold is undefined for parameter(s): "
            + ", ".join(missing)
        )
    return {name: MEAN_ABSOLUTE_TOLERANCES[name] for name in parameters}


def validate_artifact_pair(
    training: PairsArtifact, observation: PairsArtifact
) -> None:
    if training.parameter_columns != observation.parameter_columns:
        raise ContractError("training and held-out parameter columns differ")
    if training.summary_columns != observation.summary_columns:
        raise ContractError("training and held-out summary columns differ")
    if training.metadata.get("ir_hash") != observation.metadata.get("ir_hash"):
        raise ContractError("training and held-out effective IR hashes differ")
    if len(observation.rows) != 1:
        raise ContractError(
            "held-out observation artifact must contain exactly one draw"
        )
    if training.metadata.get("seed") == observation.metadata.get("seed"):
        raise ContractError(
            "held-out observation must use a seed distinct from the training sweep"
        )


def _matrix(artifact: PairsArtifact, columns: tuple[str, ...]) -> list[list[float]]:
    return [[row[column] for column in columns] for row in artifact.rows]


def train(args: argparse.Namespace) -> dict[str, Any]:
    # Set process-level thread limits before importing numerical libraries.
    for variable in (
        "OMP_NUM_THREADS",
        "MKL_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "NUMEXPR_NUM_THREADS",
    ):
        os.environ[variable] = str(args.threads)

    training = load_pairs(args.pairs, args.pairs_meta)
    observation = load_pairs(args.observation, args.observation_meta)
    validate_artifact_pair(training, observation)
    if args.train_draws < 1 or args.sbc_draws < 100:
        raise ValueError("train_draws must be positive and sbc_draws must be at least 100")
    required_draws = args.train_draws + args.sbc_draws
    if len(training.rows) < required_draws:
        raise ValueError(
            f"training artifact has {len(training.rows)} rows; "
            f"{required_draws} are required for train and SBC splits"
        )

    try:
        import numpy as np
        import torch
        from torch.utils.tensorboard import SummaryWriter
        from sbi.inference import NPE
        from sbi.neural_nets import posterior_nn
        from sbi.utils import BoxUniform
    except ImportError as error:
        raise RuntimeError(
            "NPE dependencies unavailable; install the exact versions from "
            "calibration/npe/requirements.txt"
        ) from error

    random.seed(args.seed)
    np.random.seed(args.seed)
    torch.manual_seed(args.seed)
    torch.set_num_threads(args.threads)
    torch.set_num_interop_threads(args.threads)
    torch.use_deterministic_algorithms(True)

    parameter_columns = training.parameter_columns
    summary_columns = training.summary_columns
    theta_all = np.asarray(_matrix(training, parameter_columns), dtype=np.float32)
    x_all = np.asarray(_matrix(training, summary_columns), dtype=np.float32)
    theta_train = torch.as_tensor(theta_all[: args.train_draws])
    x_train = torch.as_tensor(x_all[: args.train_draws])
    theta_sbc = torch.as_tensor(theta_all[args.train_draws : required_draws])
    x_sbc = torch.as_tensor(x_all[args.train_draws : required_draws])
    observation_theta = np.asarray(
        _matrix(observation, parameter_columns)[0], dtype=np.float64
    )
    observation_x = torch.as_tensor(
        np.asarray(_matrix(observation, summary_columns)[0], dtype=np.float32)
    )

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)
    minimum = theta_train.amin(dim=0)
    maximum = theta_train.amax(dim=0)
    span = maximum - minimum
    margin = torch.maximum(
        span * 0.5,
        torch.maximum(minimum.abs() * 0.25, torch.full_like(span, 1e-4)),
    )
    prior = BoxUniform(low=minimum - margin, high=maximum + margin)
    density_builder = posterior_nn(
        model="nsf",
        hidden_features=args.hidden_features,
        num_transforms=args.num_transforms,
    )
    summary_writer = SummaryWriter(log_dir=str(output_dir / "training-log"))
    inference = NPE(
        prior=prior,
        density_estimator=density_builder,
        summary_writer=summary_writer,
        show_progress_bars=False,
    )
    try:
        density_estimator = inference.append_simulations(theta_train, x_train).train(
            training_batch_size=args.batch_size,
            learning_rate=args.learning_rate,
            validation_fraction=0.1,
            stop_after_epochs=args.stop_after_epochs,
            max_num_epochs=args.max_num_epochs,
            show_train_summary=False,
        )
    finally:
        summary_writer.close()
    posterior = inference.build_posterior(density_estimator)
    posterior.set_default_x(observation_x)

    # Reset the sampling stream independently of training's stopping behavior.
    torch.manual_seed(args.posterior_seed)
    posterior_samples = posterior.sample(
        (args.posterior_samples,), show_progress_bars=False
    ).detach().cpu().numpy()

    samples_path = output_dir / "posterior-samples.csv"
    with samples_path.open("w", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(parameter_columns)
        for row in posterior_samples:
            writer.writerow([format(float(value), ".17g") for value in row])

    tolerances = recovery_tolerances(parameter_columns)
    parameter_results: dict[str, dict[str, Any]] = {}
    for index, name in enumerate(parameter_columns):
        values = posterior_samples[:, index].astype(np.float64)
        mean = float(np.mean(values))
        q025, q50, q975 = [
            float(value) for value in np.quantile(values, [0.025, 0.5, 0.975])
        ]
        true_value = float(observation_theta[index])
        credible_interval_contains_true = q025 <= true_value <= q975
        mean_within_tolerance = abs(mean - true_value) <= tolerances[name]
        parameter_results[name] = {
            "credible_interval_95": [q025, q975],
            "credible_interval_contains_true": credible_interval_contains_true,
            "mean": mean,
            "mean_absolute_error": abs(mean - true_value),
            "mean_absolute_tolerance": tolerances[name],
            "mean_within_tolerance": mean_within_tolerance,
            "median": q50,
            "true": true_value,
        }
    recovery_pass = all(
        result["credible_interval_contains_true"] and result["mean_within_tolerance"]
        for result in parameter_results.values()
    )

    model_path = output_dir / "posterior.pt"
    torch.save(
        {
            "posterior": posterior,
            "theta_sbc": theta_sbc,
            "x_sbc": x_sbc,
            "parameter_columns": parameter_columns,
            "sbc_seed": args.sbc_seed,
            "sbc_posterior_samples": args.sbc_posterior_samples,
        },
        model_path,
    )

    diagnostics: dict[str, Any] = {
        "inputs": {
            "held_out": {
                "metadata_sha256": observation.metadata_sha256,
                "pairs_sha256": observation.pairs_sha256,
            },
            "training": {
                "metadata_sha256": training.metadata_sha256,
                "pairs_sha256": training.pairs_sha256,
            },
        },
        "model": training.metadata.get("model"),
        "parameter_results": parameter_results,
        "pass": False,
        "recovery": {
            "credible_interval": 0.95,
            "pass": recovery_pass,
        },
        "sbc": {
            "minimum_rank_statistics": 100,
            "pass": False,
            "status": "pending",
        },
        "seeds": {
            "numpy": args.seed,
            "posterior_sampling": args.posterior_seed,
            "sbc": args.sbc_seed,
            "torch": args.seed,
        },
        "training": {
            "batch_size": args.batch_size,
            "density_estimator": "nsf",
            "draws": args.train_draws,
            "hidden_features": args.hidden_features,
            "max_num_epochs": args.max_num_epochs,
            "num_transforms": args.num_transforms,
            "threads": args.threads,
        },
    }
    write_json(output_dir / "diagnostics.json", diagnostics)
    return diagnostics


def parser() -> argparse.ArgumentParser:
    root = Path(__file__).resolve().parent
    artifacts = root / "artifacts"
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--pairs", default=artifacts / "training-pairs.csv", type=Path)
    result.add_argument("--pairs-meta", type=Path)
    result.add_argument(
        "--observation", default=artifacts / "heldout-pairs.csv", type=Path
    )
    result.add_argument("--observation-meta", type=Path)
    result.add_argument("--output", default=artifacts / "run", type=Path)
    result.add_argument("--train-draws", default=2200, type=int)
    result.add_argument("--sbc-draws", default=100, type=int)
    result.add_argument("--posterior-samples", default=2000, type=int)
    result.add_argument("--sbc-posterior-samples", default=256, type=int)
    result.add_argument("--seed", default=1701, type=int)
    result.add_argument("--posterior-seed", default=1702, type=int)
    result.add_argument("--sbc-seed", default=1703, type=int)
    result.add_argument("--threads", default=1, type=int)
    result.add_argument("--batch-size", default=128, type=int)
    result.add_argument("--learning-rate", default=5e-4, type=float)
    result.add_argument("--hidden-features", default=32, type=int)
    result.add_argument("--num-transforms", default=3, type=int)
    result.add_argument("--stop-after-epochs", default=10, type=int)
    result.add_argument("--max-num-epochs", default=80, type=int)
    return result


def main() -> int:
    args = parser().parse_args()
    try:
        train(args)
    except (ContractError, RuntimeError, ValueError) as error:
        raise SystemExit(f"error: {error}") from error
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Run the predeclared PRD-0006 full/reduced sensitivity ensemble.

This is diagnostic prior-predictive measurement, not calibration or inference.
It invokes the public Sembla CLI, scores each run through ``data/abs/score.py``,
and emits aggregate sensitivity evidence plus the compact simulated vectors
needed to recompute it independently. Work caches are resumable and are never
committed.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import shutil
import subprocess
import sys
import tempfile


ROOT = pathlib.Path(__file__).resolve().parents[1]
ABS = ROOT / "data/abs"
sys.path.insert(0, str(ABS))

import canonical  # noqa: E402
import score  # noqa: E402
import targets  # noqa: E402


DEFAULT_PREDECLARATION = ABS / "targets/sensitivity/predeclaration.json"
DEFAULT_EVIDENCE = ABS / "targets/sensitivity/evidence.json"
DEFAULT_MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"
DEFAULT_PLAN = ROOT / "fixtures/australian-population/australian_population.hundredth.plan.json"
DEFAULT_STATE = ROOT / "fixtures/state/australian_population_2010_hundredth.state"
DEFAULT_PARAMS = ABS / "params/2010.json"
DEFAULT_TARGETS = ABS / "targets/2010.json"
DEFAULT_SEMBLA = ROOT / "target/release/sembla"


def _sha(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _load(path: pathlib.Path) -> dict:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError(f"{path} does not contain a JSON object")
    return payload


def _source_tree_hash() -> str:
    files = [ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    files.extend(sorted((ROOT / "crates").rglob("Cargo.toml")))
    files.extend(sorted((ROOT / "crates").rglob("*.rs")))
    digest = hashlib.sha256(b"sembla-sensitivity-source/v1\0")
    for path in files:
        relative = path.relative_to(ROOT).as_posix().encode()
        content = path.read_bytes()
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.hexdigest()


def _evidence_run(case: dict) -> dict:
    return {
        key: case[key] for key in (
            "case_id", "seed", "params_sha256", "results_sha256",
            "observation_sha256", "ir_sha256", "plan_semantic_sha256",
            "execution_contract_sha256",
        )
    } | {
        "observed_vectors": {
            recipe: case["vectors"][recipe]["observed"]
            for recipe in ("full", "reduced")
        }
    }


def _validate_predeclaration(pre: dict, paths: dict[str, pathlib.Path]) -> None:
    if pre.get("format") != "sembla.target-sensitivity-predeclaration/v1":
        raise ValueError("unknown sensitivity predeclaration format")
    expected_hashes = {
        "model_raw_sha256": _sha(paths["model"]),
        "plan_raw_sha256": _sha(paths["plan"]),
        "state_raw_sha256": _sha(paths["state"]),
        "params_raw_sha256": _sha(paths["params"]),
        "targets_domain_sha256": targets.target_hash(paths["targets"].read_bytes()),
    }
    for key, value in expected_hashes.items():
        if pre["inputs"].get(key) != value:
            raise ValueError(f"predeclared {key} does not match current input")
    if pre.get("full_dimension") != 1096 or pre.get("reduced_dimension") != 165:
        raise ValueError("predeclared target dimensions changed")
    if len(pre.get("base_draws", [])) != 3:
        raise ValueError("predeclaration must contain exactly three base draws")
    if pre.get("expected_run_count") != 108:
        raise ValueError("predeclared run count changed")


def _parameter_values(pre: dict, base: dict, name: str, sign: int) -> dict[str, float]:
    values = dict(base["free_values"])
    prior = pre["free_parameters"][name]
    latent = base["latent_z"][name] + sign * pre["perturbation_prior_sd"]
    values[name] = math.exp(prior["location"] + prior["spread"] * latent)
    return values


def _all_params(fixed: dict[str, float], free: dict[str, float]) -> dict[str, float]:
    result = dict(fixed)
    result.update(free)
    if len(result) != 377:
        raise ValueError(f"resolved sensitivity params contain {len(result)} values")
    return result


def _run_case(
    *, case_id: str, seed: int, params: dict[str, float], pre_hash: str,
    work: pathlib.Path, paths: dict[str, pathlib.Path], sembla: pathlib.Path,
    sembla_hash: str,
) -> dict:
    cache_dir = work / "cache"
    cache_dir.mkdir(parents=True, exist_ok=True)
    cache = cache_dir / f"{case_id}.json"
    params_bytes = (json.dumps(params, sort_keys=True, indent=2) + "\n").encode()
    params_hash = hashlib.sha256(params_bytes).hexdigest()
    if cache.is_file():
        payload = _load(cache)
        if payload.get("predeclaration_sha256") != pre_hash:
            raise ValueError(f"stale cache predeclaration for {case_id}")
        if payload.get("params_sha256") != params_hash or payload.get("seed") != seed:
            raise ValueError(f"stale cache coordinates for {case_id}")
        if payload.get("sembla_sha256") != sembla_hash:
            raise ValueError(f"stale cache executable for {case_id}")
        return payload

    case_dir = work / "runs" / case_id
    shutil.rmtree(case_dir, ignore_errors=True)
    case_dir.mkdir(parents=True)
    params_path = case_dir / "params.json"
    canonical.write_json(params_path, params)
    run_path = case_dir / "run.csv"
    command = [
        str(sembla), "run", str(paths["plan"]),
        "--population", str(paths["state"]),
        "--seed", str(seed), "--ticks", "12",
        "--params", str(params_path),
        "--enable", "grouped-observations",
        "--out", str(run_path),
    ]
    process = subprocess.run(command, text=True, capture_output=True, timeout=300)
    if process.returncode:
        raise RuntimeError(
            f"Sembla failed for {case_id}:\nstdout={process.stdout}\nstderr={process.stderr}"
        )
    manifest = _load(pathlib.Path(f"{run_path}.manifest.json"))
    scored = {}
    for recipe in ("full", "reduced"):
        report = score.score_run(
            run_path, paths["targets"], paths["model"],
            mode="fitting", recipe=recipe,
        )
        scored[recipe] = {
            "target_ids": report["summary_vector"]["target_ids"],
            "observed": report["summary_vector"]["observed"],
            "target": report["summary_vector"]["target"],
        }
    payload = {
        "case_id": case_id,
        "seed": seed,
        "params_sha256": params_hash,
        "predeclaration_sha256": pre_hash,
        "sembla_sha256": sembla_hash,
        "results_sha256": manifest["results_sha256"],
        "observation_sha256": manifest["observation_sha256"],
        "ir_sha256": manifest["ir_hash"],
        "plan_semantic_sha256": manifest["plan"]["plan_semantic_hash"]["digest"],
        "execution_contract_sha256": report["run"]["execution_contract_sha256"],
        "vectors": scored,
    }
    canonical.write_json(cache, payload)
    shutil.rmtree(case_dir)
    return payload


def _normalise(observed: list[float], target_values: list[float]) -> list[float]:
    return [
        (value - target) / max(abs(target), 1.0)
        for value, target in zip(observed, target_values, strict=True)
    ]


def _sample_std(values: list[float]) -> float:
    if len(values) < 2:
        return 0.0
    mean = sum(values) / len(values)
    return math.sqrt(sum((value - mean) ** 2 for value in values) / (len(values) - 1))


def _noise_rms(vectors: list[list[float]]) -> float:
    if len(vectors) < 2:
        raise ValueError("noise estimate needs at least two vectors")
    dimensions = len(vectors[0])
    variances = []
    for index in range(dimensions):
        std = _sample_std([vector[index] for vector in vectors])
        variances.append(std * std)
    return math.sqrt(sum(variances) / dimensions)


def _rms(values: list[float]) -> float:
    return math.sqrt(sum(value * value for value in values) / len(values)) if values else 0.0


def _effective_rank_and_correlation(vectors: list[list[float]]) -> tuple[float, float]:
    if len(vectors) < 2:
        return 0.0, 0.0
    dimensions = len(vectors[0])
    means = [sum(vector[j] for vector in vectors) / len(vectors) for j in range(dimensions)]
    centered = [[vector[j] - means[j] for j in range(dimensions)] for vector in vectors]
    gram = [
        [sum(a * b for a, b in zip(left, right, strict=True)) for right in centered]
        for left in centered
    ]
    denominator = len(vectors) - 1
    trace = sum(gram[i][i] for i in range(len(gram))) / denominator
    trace_square = sum(value * value for row in gram for value in row) / (denominator ** 2)
    effective_rank = (trace * trace / trace_square) if trace_square else 0.0
    correlations = []
    for i in range(len(gram)):
        for j in range(i + 1, len(gram)):
            norm = math.sqrt(gram[i][i] * gram[j][j])
            if norm:
                correlations.append(abs(gram[i][j] / norm))
    return effective_rank, (sum(correlations) / len(correlations) if correlations else 0.0)


def measure(
    predeclaration: pathlib.Path,
    evidence: pathlib.Path,
    work: pathlib.Path,
    sembla: pathlib.Path,
) -> dict:
    paths = {
        "model": DEFAULT_MODEL,
        "plan": DEFAULT_PLAN,
        "state": DEFAULT_STATE,
        "params": DEFAULT_PARAMS,
        "targets": DEFAULT_TARGETS,
    }
    pre_bytes = predeclaration.read_bytes()
    pre_hash = hashlib.sha256(pre_bytes).hexdigest()
    pre = json.loads(pre_bytes)
    _validate_predeclaration(pre, paths)
    fixed = _load(paths["params"])
    free_names = pre["free_parameter_order"]
    sembla_hash = _sha(sembla)
    source_tree_hash = _source_tree_hash()
    version = subprocess.run(
        [str(sembla), "--version"], text=True, capture_output=True, timeout=30
    )
    if version.returncode:
        raise RuntimeError(version.stderr)

    cases: dict[str, dict] = {}
    run_log = []
    for base in pre["base_draws"]:
        for name in free_names:
            for sign, suffix in ((-1, "minus"), (1, "plus")):
                case_id = f"base_{base['index']}.{name}.{suffix}"
                free = _parameter_values(pre, base, name, sign)
                case = _run_case(
                    case_id=case_id,
                    seed=pre["paired_seeds"][base["index"]],
                    params=_all_params(fixed, free),
                    pre_hash=pre_hash,
                    work=work,
                    paths=paths,
                    sembla=sembla,
                    sembla_hash=sembla_hash,
                )
                cases[case_id] = case
                run_log.append(_evidence_run(case))
                print(f"completed {len(run_log)}/{pre['expected_run_count']}: {case_id}", flush=True)

    median_free = {
        name: math.exp(pre["free_parameters"][name]["location"])
        for name in free_names
    }
    noise_cases = []
    for seed in pre["noise_seeds"]:
        case_id = f"noise.{seed}"
        case = _run_case(
            case_id=case_id,
            seed=seed,
            params=_all_params(fixed, median_free),
            pre_hash=pre_hash,
            work=work,
            paths=paths,
            sembla=sembla,
            sembla_hash=sembla_hash,
        )
        cases[case_id] = case
        noise_cases.append(case)
        run_log.append(_evidence_run(case))
        print(f"completed {len(run_log)}/{pre['expected_run_count']}: {case_id}", flush=True)

    analysis = {}
    normalised = {"full": [], "reduced": []}
    for recipe in normalised:
        for case in cases.values():
            vector = case["vectors"][recipe]
            normalised[recipe].append(_normalise(vector["observed"], vector["target"]))
        noise_vectors = [
            _normalise(case["vectors"][recipe]["observed"], case["vectors"][recipe]["target"])
            for case in noise_cases
        ]
        effective_rank, mean_correlation = _effective_rank_and_correlation(normalised[recipe])
        analysis[recipe] = {
            "dimension": len(noise_cases[0]["vectors"][recipe]["observed"]),
            "noise_rms": _noise_rms(noise_vectors),
            "effective_rank": effective_rank,
            "mean_absolute_draw_correlation": mean_correlation,
        }

    parameter_rows = []
    failed = []
    for name in free_names:
        effects = {"full": [], "reduced": []}
        for base in pre["base_draws"]:
            minus = cases[f"base_{base['index']}.{name}.minus"]
            plus = cases[f"base_{base['index']}.{name}.plus"]
            for recipe in effects:
                minus_vector = _normalise(
                    minus["vectors"][recipe]["observed"], minus["vectors"][recipe]["target"]
                )
                plus_vector = _normalise(
                    plus["vectors"][recipe]["observed"], plus["vectors"][recipe]["target"]
                )
                effects[recipe].extend(
                    (right - left) / (2 * pre["perturbation_prior_sd"])
                    for left, right in zip(minus_vector, plus_vector, strict=True)
                )
        full_effect = _rms(effects["full"])
        reduced_effect = _rms(effects["reduced"])
        full_ratio = full_effect / analysis["full"]["noise_rms"] if analysis["full"]["noise_rms"] else math.inf
        reduced_ratio = reduced_effect / analysis["reduced"]["noise_rms"] if analysis["reduced"]["noise_rms"] else math.inf
        retained = reduced_effect / full_effect if full_effect else 0.0
        passed = (
            reduced_ratio >= pre["sensitivity_gate"]["minimum_effect_to_noise"]
            and retained >= pre["sensitivity_gate"]["minimum_retained_effect_ratio"]
        )
        if not passed:
            failed.append(name)
        parameter_rows.append({
            "parameter": name,
            "full_effect_rms": full_effect,
            "full_effect_to_noise": full_ratio,
            "reduced_effect_rms": reduced_effect,
            "reduced_effect_to_noise": reduced_ratio,
            "retained_effect_ratio": retained,
            "reduced_gate_pass": passed,
        })

    first_case = next(iter(cases.values()))
    vector_contract = {
        recipe: {
            "target_ids": first_case["vectors"][recipe]["target_ids"],
            "target": first_case["vectors"][recipe]["target"],
        }
        for recipe in ("full", "reduced")
    }
    for case in cases.values():
        for recipe in vector_contract:
            if case["vectors"][recipe]["target_ids"] != vector_contract[recipe]["target_ids"]:
                raise ValueError(f"{recipe} target order changed between sensitivity runs")
            if case["vectors"][recipe]["target"] != vector_contract[recipe]["target"]:
                raise ValueError(f"{recipe} target values changed between sensitivity runs")
    identities = {
        key: {case[key] for case in cases.values()}
        for key in ("ir_sha256", "plan_semantic_sha256", "execution_contract_sha256")
    }
    if any(len(values) != 1 for values in identities.values()):
        raise ValueError("execution identity changed within sensitivity ensemble")

    evidence_payload = {
        "format": "sembla.target-sensitivity-evidence/v1",
        "predeclaration_path": predeclaration.name,
        "predeclaration_sha256": pre_hash,
        "execution": {
            "sembla_version": version.stdout.strip(),
            "sembla_binary_raw_sha256": sembla_hash,
            "rust_source_tree_sha256": source_tree_hash,
            "source_tree_hash_domain": "sembla-sensitivity-source/v1",
            "measurement_script_raw_sha256": _sha(pathlib.Path(__file__)),
            "scorer_raw_sha256": _sha(ABS / "score.py"),
            "python_version": sys.version,
            "ir_sha256": next(iter(identities["ir_sha256"])),
            "plan_semantic_sha256": next(iter(identities["plan_semantic_sha256"])),
            "execution_contract_sha256": next(iter(identities["execution_contract_sha256"])),
        },
        "run_count": len(run_log),
        "vector_contract": vector_contract,
        "analysis": analysis,
        "parameter_sensitivity": parameter_rows,
        "failed_parameters": failed,
        "recommendation": "reduced" if not failed else "full",
        "recommendation_rule": (
            "recommend reduced only when every free parameter passes the "
            "predeclared effect-to-noise and retained-effect gates"
        ),
        "runs": run_log,
        "chronology_note": (
            "The predeclaration was written before the first ensemble run in the "
            "implementation session. It and this evidence remain in one uncommitted "
            "series, so Git history alone does not timestamp their order."
        ),
    }
    canonical.write_json(evidence, evidence_payload)
    return evidence_payload


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--predeclaration", type=pathlib.Path, default=DEFAULT_PREDECLARATION)
    parser.add_argument("--evidence", type=pathlib.Path, default=DEFAULT_EVIDENCE)
    parser.add_argument("--sembla", type=pathlib.Path, default=DEFAULT_SEMBLA)
    parser.add_argument("--work", type=pathlib.Path)
    args = parser.parse_args(argv)
    if args.work is None:
        with tempfile.TemporaryDirectory(prefix="sembla-target-sensitivity-") as directory:
            payload = measure(
                args.predeclaration, args.evidence, pathlib.Path(directory), args.sembla
            )
    else:
        args.work.mkdir(parents=True, exist_ok=True)
        payload = measure(args.predeclaration, args.evidence, args.work, args.sembla)
    print(
        f"recommendation={payload['recommendation']} "
        f"failed={','.join(payload['failed_parameters']) or 'none'}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

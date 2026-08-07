#!/usr/bin/env python3
"""Score a Sembla run against a canonical ``sembla.targets/v1`` ledger."""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import json
import math
import pathlib
from typing import Iterable

import canonical
import targets as targets_module


REPORT_FORMAT = "sembla.target-score/v1"
SIZE_BINS = (
    ("lt_1k", 0, 1_000),
    ("1k_to_10k", 1_000, 10_000),
    ("10k_to_100k", 10_000, 100_000),
    ("gte_100k", 100_000, None),
)


@dataclass(frozen=True)
class GroupedRow:
    tick: int
    keys: tuple[object, ...]
    count: int


class GroupedStore:
    def __init__(self, run_path: pathlib.Path, model: dict, manifest: dict):
        self.run_path = pathlib.Path(run_path)
        self.model = model
        self.manifest = manifest
        grouped, attrs = targets_module._grouped_contract(model)
        self.grouped_contract = grouped
        self.attrs = attrs
        self.key_types = {
            view: {key: attrs[key] for key in keys}
            for view, keys in grouped.items()
        }
        self.cache: dict[str, tuple[GroupedRow, ...]] = {}
        manifest_rows = manifest.get("grouped_outputs")
        if not isinstance(manifest_rows, list):
            raise ValueError("run manifest grouped_outputs is not a list")
        self.manifest_outputs = {}
        for row in manifest_rows:
            if not isinstance(row, dict) or set(row) != {"view", "algorithm", "sha256"}:
                raise ValueError("run manifest has malformed grouped output metadata")
            view = row.get("view")
            if not isinstance(view, str) or view in self.manifest_outputs:
                raise ValueError(f"run manifest repeats grouped output {view!r}")
            self.manifest_outputs[view] = row
        if set(self.manifest_outputs) != set(self.grouped_contract):
            missing = sorted(set(self.grouped_contract) - set(self.manifest_outputs))
            extra = sorted(set(self.manifest_outputs) - set(self.grouped_contract))
            raise ValueError(
                f"run manifest grouped-output inventory changed; missing={missing!r}, "
                f"extra={extra!r}"
            )

    def _path(self, view: str) -> pathlib.Path:
        text = str(self.run_path)
        stem = text[:-4] if text.endswith(".csv") else text
        return pathlib.Path(f"{stem}.grouped.{view}.csv")

    def rows(self, view: str) -> tuple[GroupedRow, ...]:
        if view in self.cache:
            return self.cache[view]
        if view not in self.grouped_contract:
            raise ValueError(f"model has no grouped observation {view!r}")
        if view not in self.manifest_outputs:
            raise ValueError(f"run manifest has no grouped output for {view!r}")
        path = self._path(view)
        if not path.is_file():
            raise ValueError(f"missing grouped output {path}")
        manifest_row = self.manifest_outputs[view]
        if manifest_row.get("algorithm") != "sha256":
            raise ValueError(f"grouped output {view!r} has unknown hash algorithm")
        actual_hash = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual_hash != manifest_row.get("sha256"):
            raise ValueError(f"grouped output {view!r} hash mismatch")

        key_order = self.grouped_contract[view]
        expected_header = ["tick", *key_order, "count"]
        parsed: list[GroupedRow] = []
        seen = set()
        with path.open(newline="", encoding="utf-8") as source:
            reader = csv.DictReader(source)
            if reader.fieldnames != expected_header:
                raise ValueError(
                    f"grouped output {view!r} header {reader.fieldnames!r}; "
                    f"expected {expected_header!r}"
                )
            for source_row in reader:
                try:
                    tick = int(source_row["tick"])
                    count = int(source_row["count"])
                except ValueError as error:
                    raise ValueError(f"grouped output {view!r} has non-integer count/tick") from error
                if not 0 <= tick < self.manifest["ticks"] or count <= 0:
                    raise ValueError(f"grouped output {view!r} has invalid row {source_row!r}")
                keys = []
                for key in key_order:
                    raw = source_row[key]
                    key_type = self.key_types[view][key]
                    if key_type.get("kind") == "enum":
                        if raw not in key_type.get("variants", []):
                            raise ValueError(
                                f"grouped output {view!r} has unknown {key} variant {raw!r}"
                            )
                        keys.append(raw)
                    elif key_type.get("kind") == "int":
                        try:
                            parsed_key = int(raw)
                        except ValueError as error:
                            raise ValueError(
                                f"grouped output {view!r} has non-integer key {key!r}"
                            ) from error
                        if parsed_key < 0:
                            raise ValueError(
                                f"grouped output {view!r} has negative key {key!r}"
                            )
                        keys.append(parsed_key)
                    else:
                        raise ValueError(
                            f"grouped output {view!r} uses unsupported grouped type "
                            f"for {key!r}"
                        )
                identity = (tick, *keys)
                if identity in seen:
                    raise ValueError(f"grouped output {view!r} has duplicate row {identity!r}")
                seen.add(identity)
                parsed.append(GroupedRow(tick=tick, keys=tuple(keys), count=count))
        result = tuple(parsed)
        self.cache[view] = result
        return result

    def validate_all(self) -> None:
        """Hash, parse, and type-check every declared grouped output."""
        for view in sorted(self.grouped_contract):
            self.rows(view)


def _load_json(path: pathlib.Path) -> dict:
    try:
        payload = json.loads(pathlib.Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ValueError(f"cannot read JSON {path}: {error}") from error
    if not isinstance(payload, dict):
        raise ValueError(f"JSON root in {path} is not an object")
    return payload


def _read_scalar_ticks(path: pathlib.Path, expected_ticks: int) -> None:
    with pathlib.Path(path).open(newline="", encoding="utf-8") as source:
        lines = [line for line in source if not line.startswith("#")]
    reader = csv.DictReader(lines)
    if reader.fieldnames is None or "tick" not in reader.fieldnames:
        raise ValueError("scalar run CSV has no tick column")
    ticks = []
    for row in reader:
        try:
            ticks.append(int(row["tick"]))
        except ValueError as error:
            raise ValueError("scalar run CSV has a non-integer tick") from error
    if ticks != list(range(expected_ticks)):
        raise ValueError(
            f"scalar run ticks changed: got {ticks!r}, expected {list(range(expected_ticks))!r}"
        )


def _manifest_path(run_path: pathlib.Path) -> pathlib.Path:
    return pathlib.Path(f"{run_path}.manifest.json")


def _sha256_file(path: pathlib.Path, label: str) -> str:
    try:
        return hashlib.sha256(pathlib.Path(path).read_bytes()).hexdigest()
    except OSError as error:
        raise ValueError(f"cannot read {label} {path}: {error}") from error


def _validate_execution_contract(
    path: pathlib.Path,
    artifact: dict,
    model_bytes: bytes,
    manifest: dict,
) -> tuple[dict, str]:
    contract_path = pathlib.Path(path)
    contract_bytes = contract_path.read_bytes()
    contract = _load_json(contract_path)
    if set(contract) != {"format", "model", "plan", "provenance"}:
        raise ValueError("execution contract schema changed")
    if contract.get("format") != targets_module.EXECUTION_FORMAT:
        raise ValueError("execution contract format changed")
    if contract != targets_module.execution_contract():
        raise ValueError("execution contract does not match the reviewed model/plan identity")
    model_contract = contract.get("model")
    if not isinstance(model_contract, dict) or set(model_contract) != {
        "name", "raw_sha256", "ir_hash"
    }:
        raise ValueError("execution contract model identity changed")
    model_raw = hashlib.sha256(model_bytes).hexdigest()
    if model_contract.get("name") != artifact["model"]["name"] or model_contract.get(
        "raw_sha256"
    ) != model_raw or model_raw != artifact["model"]["sha256"]:
        raise ValueError("execution contract model bytes do not match targets")
    expected_ir = model_contract.get("ir_hash")
    if not isinstance(expected_ir, dict) or set(expected_ir) != {"algorithm", "digest"}:
        raise ValueError("execution contract IR hash is malformed")
    if manifest.get("ir_hash_algorithm") != expected_ir.get("algorithm") or manifest.get(
        "ir_hash"
    ) != expected_ir.get("digest"):
        raise ValueError("run manifest IR identity does not match execution contract")

    plan_contract = contract.get("plan")
    if not isinstance(plan_contract, dict) or set(plan_contract) != {
        "raw_sha256", "schema", "identity_scheme", "origin", "enabled_features",
        "semantic_hash",
    }:
        raise ValueError("execution contract plan identity changed")
    expected_plan = {
        "enabled_features": plan_contract["enabled_features"],
        "identity_scheme": plan_contract["identity_scheme"],
        "origin": plan_contract["origin"],
        "plan_schema": plan_contract["schema"],
        "plan_semantic_hash": plan_contract["semantic_hash"],
    }
    if manifest.get("plan") != expected_plan:
        raise ValueError("run manifest plan identity does not match execution contract")
    if manifest.get("enabled_features") != plan_contract["enabled_features"]:
        raise ValueError("run feature set does not match execution contract")
    return contract, hashlib.sha256(contract_bytes).hexdigest()


def _validate_run(
    run_path: pathlib.Path,
    artifact: dict,
    model: dict,
    model_bytes: bytes,
    execution_contract_path: pathlib.Path,
) -> tuple[dict, dict, str]:
    if hashlib.sha256(model_bytes).hexdigest() != artifact["model"]["sha256"]:
        raise ValueError("scorer model bytes do not match target artifact")
    targets_module.validate_artifact(artifact, model, model_bytes=model_bytes)
    manifest = _load_json(_manifest_path(run_path))
    if manifest.get("manifest_kind") != "run":
        raise ValueError("scorer requires a run manifest")
    if manifest.get("model") != artifact["model"]["name"]:
        raise ValueError("run manifest model does not match targets")
    if manifest.get("ticks") != 12:
        raise ValueError("annual target scoring requires exactly 12 ticks")
    if manifest.get("dt") != 1.0:
        raise ValueError("annual target scoring requires dt=1")
    contract, contract_hash = _validate_execution_contract(
        execution_contract_path, artifact, model_bytes, manifest
    )

    if manifest.get("results_hash_algorithm") != "sha256":
        raise ValueError("run manifest results hash algorithm changed")
    actual_results_hash = _sha256_file(run_path, "scalar run CSV")
    if manifest.get("results_sha256") != actual_results_hash:
        raise ValueError("scalar run CSV results hash mismatch")
    if manifest.get("observation_hash_algorithm") != "sha256":
        raise ValueError("run manifest observation hash algorithm changed")
    summaries_path = pathlib.Path(f"{run_path}.summaries.csv")
    actual_observation_hash = _sha256_file(summaries_path, "summary observation CSV")
    if manifest.get("observation_sha256") != actual_observation_hash:
        raise ValueError("summary observation CSV hash mismatch")
    _read_scalar_ticks(run_path, 12)
    return manifest, contract, contract_hash


def _selector_match(selector: dict, value: object) -> bool:
    operation, expected = next(iter(selector["match"].items()))
    if operation == "eq":
        return value == expected
    if operation == "any":
        return expected is True
    if operation == "gte":
        return isinstance(value, int) and value >= expected
    if operation == "exclude":
        return value not in expected
    raise ValueError(f"unknown selector operation {operation!r}")


def _rows_matching(
    store: GroupedStore,
    view: str,
    selectors: list[dict],
    time: dict,
) -> Iterable[GroupedRow]:
    key_order = store.grouped_contract[view]
    if tuple(selector["key"] for selector in selectors) != key_order:
        raise ValueError(f"selector order changed for {view!r}")
    if "tick" in time:
        allowed_ticks = {time["tick"]}
    else:
        period = time["period"]
        allowed_ticks = set(range(period["start_tick"], period["end_tick"] + 1))
    for row in store.rows(view):
        if row.tick not in allowed_ticks:
            continue
        if all(
            _selector_match(selector, value)
            for selector, value in zip(selectors, row.keys, strict=True)
        ):
            yield row


def _sum_count(store: GroupedStore, view: str, selectors: list[dict], time: dict) -> int:
    return sum(row.count for row in _rows_matching(store, view, selectors, time))


def _target_number(value: dict) -> float:
    if value.get("kind") == "count":
        return float(value["count"])
    if value.get("kind") == "ratio":
        denominator = value["denominator"]
        if denominator <= 0:
            raise ValueError("target ratio denominator must be positive")
        return value["numerator"] / denominator
    raise ValueError(f"unknown target value kind {value.get('kind')!r}")


def observe_target(target: dict, store: GroupedStore, scale_factor: int) -> float:
    observation = target["observation"]
    view = observation["name"]
    operation = target["aggregation"]["operation"]
    time = target["time"]
    if operation in {"last_count", "sum_count"}:
        count = _sum_count(store, view, observation["keys"], time)
        return float(count * scale_factor)
    if operation == "ratio_of_sums":
        numerator = _sum_count(store, view, observation["numerator_keys"], time)
        denominator = _sum_count(store, view, observation["denominator_keys"], time)
        if denominator == 0:
            raise ValueError(f"observed ratio denominator is zero for {target['id']!r}")
        return numerator / denominator
    if operation == "weighted_ratio":
        selectors = observation["keys"]
        key_order = store.grouped_contract[view]
        weight_key = target["aggregation"]["weight_key"]
        try:
            weight_index = key_order.index(weight_key)
        except ValueError as error:
            raise ValueError(f"weight key {weight_key!r} is not grouped by {view!r}") from error
        cap = target["aggregation"]["cap"]
        power = target["aggregation"]["power"]
        denominator = 0
        numerator = 0
        for row in _rows_matching(store, view, selectors, time):
            band = row.keys[weight_index]
            if not isinstance(band, int):
                raise ValueError("weighted grouped key is not an integer band")
            weight = min(band, cap) ** power
            numerator += row.count * weight
            denominator += row.count
        if denominator == 0:
            raise ValueError(f"observed weighted denominator is zero for {target['id']!r}")
        return numerator / denominator
    raise ValueError(f"unknown aggregation operation {operation!r}")


def _metrics(rows: list[dict]) -> dict:
    if not rows:
        return {"count": 0, "mae": None, "rmse": None, "maximum_absolute_error": None}
    errors = [row["signed_error"] for row in rows]
    return {
        "count": len(rows),
        "mae": sum(abs(error) for error in errors) / len(errors),
        "rmse": math.sqrt(sum(error * error for error in errors) / len(errors)),
        "maximum_absolute_error": max(abs(error) for error in errors),
    }


def _size_bin(target: dict) -> str:
    if target["value"]["kind"] != "count":
        return "non_count"
    value = target["value"]["count"]
    for name, lower, upper in SIZE_BINS:
        if value >= lower and (upper is None or value < upper):
            return name
    raise AssertionError("population-size bins are exhaustive")


def score_run(
    run_path: pathlib.Path,
    targets_path: pathlib.Path,
    model_path: pathlib.Path,
    *,
    mode: str,
    recipe: str = "full",
    requested_target_ids: list[str] | None = None,
    execution_contract_path: pathlib.Path | None = None,
) -> dict:
    """Score one run and return a canonical-JSON-compatible report object."""
    if mode not in {"fitting", "evaluation"}:
        raise ValueError("score mode must be 'fitting' or 'evaluation'")
    run_path = pathlib.Path(run_path)
    targets_path = pathlib.Path(targets_path)
    model_path = pathlib.Path(model_path)
    artifact = _load_json(targets_path)
    model_bytes = model_path.read_bytes()
    model = json.loads(model_bytes)
    if execution_contract_path is None:
        execution_contract_path = targets_path.parent / "execution.json"
    manifest, execution_contract, execution_contract_hash = _validate_run(
        run_path,
        artifact,
        model,
        model_bytes,
        pathlib.Path(execution_contract_path),
    )
    by_id = {target["id"]: target for target in artifact["targets"]}

    if requested_target_ids is not None:
        selected_ids = requested_target_ids
    elif mode == "fitting":
        try:
            selected_ids = artifact["projections"][recipe]["target_ids"]
        except KeyError as error:
            raise ValueError(f"unknown fitted projection {recipe!r}") from error
    else:
        selected_ids = [target["id"] for target in artifact["targets"]]

    if len(selected_ids) != len(set(selected_ids)):
        raise ValueError("requested target IDs contain duplicates")
    selected = []
    for target_id in selected_ids:
        target = by_id.get(target_id)
        if target is None:
            raise ValueError(f"requested unknown target {target_id!r}")
        if mode == "fitting" and target["role"] != "fitted":
            raise ValueError(f"fitting mode refuses heldout target {target_id!r}")
        selected.append(target)

    store = GroupedStore(run_path, model, manifest)
    store.validate_all()
    residuals = []
    for target in selected:
        observed = observe_target(target, store, artifact["scale"]["factor"])
        expected = _target_number(target["value"])
        signed_error = observed - expected
        residuals.append({
            "id": target["id"],
            "family": target["family"],
            "role": target["role"],
            "state": target.get("state"),
            "population_size_bin": _size_bin(target),
            "observed": observed,
            "target": expected,
            "signed_error": signed_error,
            "absolute_error": abs(signed_error),
        })

    families = sorted({row["family"] for row in residuals})
    roles = sorted({row["role"] for row in residuals})
    states = sorted({row["state"] for row in residuals if row["state"] is not None})
    bins = [name for name, _lower, _upper in SIZE_BINS] + ["non_count"]
    report = {
        "format": REPORT_FORMAT,
        "targets": {
            "path": targets_path.name,
            "sha256": targets_module.target_hash(targets_path.read_bytes()),
            "run_year": artifact["run_year"],
            "variant": artifact["variant"],
        },
        "run": {
            "model": manifest["model"],
            "seed": manifest["seed"],
            "ticks": manifest["ticks"],
            "results_sha256": manifest["results_sha256"],
            "observation_sha256": manifest["observation_sha256"],
            "ir_sha256": execution_contract["model"]["ir_hash"]["digest"],
            "plan_semantic_sha256": execution_contract["plan"]["semantic_hash"]["digest"],
            "execution_contract_sha256": execution_contract_hash,
        },
        "mode": mode,
        "recipe": recipe if mode == "fitting" else None,
        "residuals": residuals,
        "metrics": {
            "overall": _metrics(residuals),
            "by_family": {
                family: _metrics([row for row in residuals if row["family"] == family])
                for family in families
            },
            "by_role": {
                role: _metrics([row for row in residuals if row["role"] == role])
                for role in roles
            },
            "by_population_size": {
                size_bin: _metrics([
                    row for row in residuals if row["population_size_bin"] == size_bin
                ])
                for size_bin in bins
            },
            "by_state": {
                state: _metrics([row for row in residuals if row["state"] == state])
                for state in states
            },
        },
        "summary_vector": {
            "target_ids": selected_ids,
            "observed": [row["observed"] for row in residuals],
            "target": [row["target"] for row in residuals],
        },
        "diagnostics": artifact["diagnostics"],
    }
    return report


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=pathlib.Path, required=True)
    parser.add_argument("--targets", type=pathlib.Path, required=True)
    parser.add_argument("--model", type=pathlib.Path, required=True)
    parser.add_argument("--mode", choices=("fitting", "evaluation"), required=True)
    parser.add_argument("--recipe", choices=("full", "reduced"), default="full")
    parser.add_argument("--target-id", action="append", dest="target_ids")
    parser.add_argument(
        "--execution-contract",
        type=pathlib.Path,
        help="defaults to execution.json beside the target ledger",
    )
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args(argv)
    report = score_run(
        args.run,
        args.targets,
        args.model,
        mode=args.mode,
        recipe=args.recipe,
        requested_target_ids=args.target_ids,
        execution_contract_path=args.execution_contract,
    )
    canonical.write_json(args.out, report)
    print(
        f"scored {len(report['residuals'])} targets; "
        f"MAE={report['metrics']['overall']['mae']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

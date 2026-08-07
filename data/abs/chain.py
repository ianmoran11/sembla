#!/usr/bin/env python3
"""Run, verify, and summarize the Australian annual population chain.

The module is deliberately standard-library-only and talks to Sembla only
through its public CLI and canonical files.  A chain is not a checkpointed run:
each run year is an independent twelve-tick execution linked to the next by a
hashed ``sembla.state/v1`` artifact.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import pathlib
import shutil
import subprocess
import sys
from typing import Iterable

import canonical
import score
import targets as targets_module


HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]
DEFAULT_PARAMS = HERE / "params"
DEFAULT_TARGETS = HERE / "targets"
DEFAULT_MODEL = (
    ROOT / "fixtures/australian-population/australian_population.hundredth.json"
)
DEFAULT_PLAN = (
    ROOT / "fixtures/australian-population/australian_population.hundredth.plan.json"
)
DEFAULT_STATE = (
    ROOT / "fixtures/state/australian_population_2010_hundredth.state"
)
DEFAULT_SEMBLA = ROOT / "target/release/sembla"

CHAIN_FORMAT = "sembla.australian-population-chain/v1"
SEED_COORDINATE_FORMAT = "sembla.australian-population-seed-coordinate/v1"
SEED_DOMAIN = b"sembla.australian-population-seed/v1\0"
REPRODUCTION_FORMAT = "sembla.australian-population-chain-reproduction/v1"
MIDDLE_REPRODUCTION_FORMAT = (
    "sembla.australian-population-middle-year-reproduction/v1"
)
EVIDENCE_FORMAT = "sembla.australian-population-baseline-evidence/v1"
RUN_YEARS = tuple(range(2010, 2025))
SCALES = ("full", "tenth", "hundredth")
STATES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")
FEATURE = "grouped-observations"


class ChainError(ValueError):
    """A deterministic chain contract failed."""


def _load_json(path: pathlib.Path) -> dict:
    path = pathlib.Path(path)
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ChainError(f"cannot read JSON {path}: {error}") from error
    if not isinstance(payload, dict):
        raise ChainError(f"JSON root in {path} is not an object")
    return payload


def _sha256(path: pathlib.Path) -> str:
    digest = hashlib.sha256()
    try:
        with pathlib.Path(path).open("rb") as source:
            for block in iter(lambda: source.read(1024 * 1024), b""):
                digest.update(block)
    except OSError as error:
        raise ChainError(f"cannot hash {path}: {error}") from error
    return digest.hexdigest()


def _raw_record(path: pathlib.Path, display: str) -> dict:
    path = pathlib.Path(path)
    return {
        "path": display,
        "bytes": path.stat().st_size,
        "raw_sha256": _sha256(path),
    }


def _repository_display(path: pathlib.Path) -> str:
    path = pathlib.Path(path).resolve()
    try:
        return path.relative_to(ROOT.resolve()).as_posix()
    except ValueError:
        return path.name


def _is_sha256(value: object) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(character in "0123456789abcdef" for character in value)
    )


def seed_coordinate(
    *,
    model_identity: str,
    scale: str,
    run_year: int,
    params_sha256: str,
    replica_index: int,
) -> dict:
    """Return the frozen, position-independent annual seed coordinate."""
    if not _is_sha256(model_identity):
        raise ChainError("model identity must be a lowercase SHA-256 digest")
    if scale not in SCALES:
        raise ChainError(f"unknown scale {scale!r}")
    if run_year not in RUN_YEARS:
        raise ChainError(f"run year {run_year} is outside 2010..2024")
    if not _is_sha256(params_sha256):
        raise ChainError("parameter identity must be a lowercase SHA-256 digest")
    if type(replica_index) is not int or replica_index < 0:
        raise ChainError("replica index must be a non-negative integer")
    return {
        "format": SEED_COORDINATE_FORMAT,
        "model_identity": model_identity,
        "params_raw_sha256": params_sha256,
        "replica_index": replica_index,
        "run_year": run_year,
        "scale": scale,
    }


def derive_seed(**coordinate_values) -> dict:
    """Derive a u64 seed from a canonical semantic coordinate."""
    coordinate = seed_coordinate(**coordinate_values)
    encoded = json.dumps(
        coordinate,
        sort_keys=True,
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    digest = hashlib.sha256(SEED_DOMAIN + encoded).digest()
    return {
        "coordinate": coordinate,
        "domain": "sembla.australian-population-seed/v1",
        "sha256": digest.hex(),
        "seed": int.from_bytes(digest[:8], "big"),
    }


def _state_hash(sembla: pathlib.Path, path: pathlib.Path) -> dict:
    process = subprocess.run(
        [str(sembla), "state-hash", str(path)],
        text=True,
        capture_output=True,
    )
    if process.returncode:
        raise ChainError(
            f"state-hash failed for {path}: {process.stdout}{process.stderr}"
        )
    fields = process.stdout.strip().split()
    if (
        len(fields) != 4
        or fields[:3] != ["state", "sha256", "sembla.state-artifact/v1"]
        or not _is_sha256(fields[3])
    ):
        raise ChainError(f"unexpected state-hash output for {path}: {process.stdout!r}")
    return {
        "format": "sembla.state/v1",
        "hash": {
            "algorithm": "sha256",
            "domain": "sembla.state-artifact/v1",
            "digest": fields[3],
        },
    }


def _grouped_path(run_path: pathlib.Path, view: str) -> pathlib.Path:
    text = str(run_path)
    stem = text[:-4] if text.endswith(".csv") else text
    return pathlib.Path(f"{stem}.grouped.{view}.csv")


def _contested_transition_names(model: dict) -> tuple[str, ...]:
    boxes = model.get("boxes")
    if not isinstance(boxes, list) or len(boxes) != 1:
        raise ChainError("capacity inspection requires the one-box population model")
    names = []
    resources = set()
    for transition in boxes[0].get("transitions", []):
        contests = transition.get("contests", [])
        if not contests:
            continue
        if len(contests) != 1:
            raise ChainError("population transition has more than one contested resource")
        resource = contests[0].get("resource", {})
        if resource.get("kind") != "self_attr":
            raise ChainError("population contest resource is not a self attribute")
        resources.add(resource.get("name"))
        names.append(transition["name"])
    if resources != {"slot_resource"} or not names:
        raise ChainError(
            f"population contest contract changed: resources={sorted(resources)!r}"
        )
    return tuple(names)


def inspect_capacity(run_path: pathlib.Path, model: dict) -> dict:
    """Return vacancy minima and the strict per-tick saturation diagnostic."""
    contested = _contested_transition_names(model)
    try:
        with pathlib.Path(run_path).open(newline="", encoding="utf-8") as source:
            lines = [line for line in source if not line.startswith("#")]
    except OSError as error:
        raise ChainError(f"cannot read scalar run output {run_path}: {error}") from error
    reader = csv.DictReader(lines)
    required = {
        "tick",
        "vacant_birth_slots",
        "vacant_overseas_slots",
        "deferred_total",
        *(f"fired_{name}" for name in contested),
    }
    if reader.fieldnames is None or not required.issubset(reader.fieldnames):
        missing = sorted(required - set(reader.fieldnames or []))
        raise ChainError(f"scalar run output is missing capacity columns: {missing!r}")
    minimum_birth = None
    minimum_overseas = None
    maximum = None
    row_count = 0
    for row in reader:
        row_count += 1
        try:
            tick = int(row["tick"])
            vacant_birth = int(row["vacant_birth_slots"])
            vacant_overseas = int(row["vacant_overseas_slots"])
            deferred = int(row["deferred_total"])
            fired = sum(int(row[f"fired_{name}"]) for name in contested)
        except ValueError as error:
            raise ChainError("scalar capacity row contains a non-integer") from error
        if min(vacant_birth, vacant_overseas, deferred, fired) < 0:
            raise ChainError("scalar capacity row contains a negative count")
        minimum_birth = (
            vacant_birth if minimum_birth is None else min(minimum_birth, vacant_birth)
        )
        minimum_overseas = (
            vacant_overseas
            if minimum_overseas is None
            else min(minimum_overseas, vacant_overseas)
        )
        candidate = (deferred, fired, tick)
        if maximum is None:
            maximum = candidate
        else:
            left_deferred, left_fired, _left_tick = maximum
            # Compare ratios exactly; positive/zero is infinite and zero/zero is zero.
            if fired == 0:
                greater = deferred > 0 and not (
                    left_fired == 0 and left_deferred > 0
                )
            elif left_fired == 0:
                greater = left_deferred == 0 and deferred > 0
            else:
                greater = deferred * left_fired > left_deferred * fired
            if greater:
                maximum = candidate
    if row_count != 12:
        raise ChainError(f"annual capacity inspection saw {row_count} ticks, expected 12")
    assert minimum_birth is not None and minimum_overseas is not None and maximum is not None
    deferred, fired, tick = maximum
    saturated = deferred > 0 and (fired == 0 or deferred * 10 > fired)
    zero_vacancy = minimum_birth == 0 or minimum_overseas == 0
    ratio = math.inf if fired == 0 and deferred else (deferred / fired if fired else 0.0)
    return {
        "minimum_vacant_birth_slots": minimum_birth,
        "minimum_vacant_overseas_slots": minimum_overseas,
        "maximum_saturation": {
            "tick": tick,
            "deferred": deferred,
            "fired_on_slot_resource": fired,
            "deferred_to_fired_ratio": ratio,
            "strictly_exceeds_ten_percent": saturated,
        },
        "zero_vacancy_margin": zero_vacancy,
        "valid_calibration_evidence": not saturated and not zero_vacancy,
    }


def require_capacity(capacity: dict, label: str) -> None:
    if capacity["maximum_saturation"]["strictly_exceeds_ten_percent"]:
        maximum = capacity["maximum_saturation"]
        raise ChainError(
            f"{label}: saturation at tick {maximum['tick']}: "
            f"{maximum['deferred']} deferred exceeds 10% of "
            f"{maximum['fired_on_slot_resource']} fired"
        )
    if capacity["minimum_vacant_birth_slots"] == 0:
        raise ChainError(f"{label}: minimum vacant birth slots reached zero")
    if capacity["minimum_vacant_overseas_slots"] == 0:
        raise ChainError(f"{label}: minimum vacant overseas slots reached zero")


def _expected_plan_tuple(execution: dict) -> dict:
    plan = execution["plan"]
    return {
        "enabled_features": plan["enabled_features"],
        "identity_scheme": plan["identity_scheme"],
        "origin": plan["origin"],
        "plan_schema": plan["schema"],
        "plan_semantic_hash": plan["semantic_hash"],
    }


def _require_file(path: pathlib.Path, label: str) -> pathlib.Path:
    path = pathlib.Path(path)
    if not path.is_file():
        raise ChainError(f"missing {label}: {path}")
    return path


def _resolve_defaults(scale: str, args) -> tuple[pathlib.Path, pathlib.Path, pathlib.Path]:
    if scale == "hundredth":
        model = pathlib.Path(args.model or DEFAULT_MODEL)
        plan = pathlib.Path(args.plan or DEFAULT_PLAN)
        state = pathlib.Path(args.initial_state or DEFAULT_STATE)
    else:
        model = pathlib.Path(
            args.model or HERE / "generated" / f"australian_population_{scale}.json"
        )
        plan = pathlib.Path(
            args.plan or HERE / "generated" / f"australian_population_{scale}.plan.json"
        )
        state = pathlib.Path(
            args.initial_state
            or HERE / "generated" / f"australian_population_2010_{scale}.state"
        )
    return model, plan, state


def _validate_year_range(start_year: int, end_year: int) -> None:
    if start_year not in RUN_YEARS or end_year not in RUN_YEARS:
        raise ChainError("start/end run years must lie in 2010..2024")
    if start_year > end_year:
        raise ChainError("start run year must not exceed end run year")


def _relative_or_absolute(path: pathlib.Path) -> str:
    path = pathlib.Path(path).resolve()
    try:
        return path.relative_to(ROOT.resolve()).as_posix()
    except ValueError:
        return str(path)


def _resolve_recorded_path(text: str) -> pathlib.Path:
    path = pathlib.Path(text)
    return path if path.is_absolute() else ROOT / path


def _run_paths(out_dir: pathlib.Path, year: int) -> dict[str, pathlib.Path]:
    run = out_dir / f"{year}.csv"
    return {
        "run": run,
        "manifest": pathlib.Path(f"{run}.manifest.json"),
        "summaries": pathlib.Path(f"{run}.summaries.csv"),
        "state": out_dir / f"{year + 1}.state",
        "score": out_dir / f"{year}.score.json",
    }


def _binary_version(sembla: pathlib.Path) -> str:
    process = subprocess.run(
        [str(sembla), "--version"], text=True, capture_output=True
    )
    if process.returncode:
        raise ChainError(f"cannot read Sembla version: {process.stderr}")
    return process.stdout.strip()


def _build_report(
    *,
    out_dir: pathlib.Path,
    scale: str,
    start_year: int,
    end_year: int,
    replica_index: int,
    backend: str,
    enabled_features: list[str],
    params_dir: pathlib.Path,
    targets_dir: pathlib.Path,
    model_path: pathlib.Path,
    plan_path: pathlib.Path,
    initial_state: pathlib.Path,
    sembla: pathlib.Path,
) -> dict:
    model_bytes = model_path.read_bytes()
    model = json.loads(model_bytes)
    execution_path = targets_dir / "execution.json"
    execution = _load_json(execution_path)
    if execution.get("format") != targets_module.EXECUTION_FORMAT:
        raise ChainError("target execution contract format changed")
    if execution["model"]["raw_sha256"] != hashlib.sha256(model_bytes).hexdigest():
        raise ChainError("chain model bytes do not match target execution contract")
    if execution["plan"]["raw_sha256"] != _sha256(plan_path):
        raise ChainError("chain plan bytes do not match target execution contract")
    if enabled_features != execution["plan"]["enabled_features"]:
        raise ChainError("chain feature set does not match target execution contract")
    model_identity = execution["model"]["ir_hash"]["digest"]
    expected_plan = _expected_plan_tuple(execution)
    expected_backend = "cpu-oracle" if backend == "cpu" else "cuda"
    state_hash_cache = {}

    def state_hash(path: pathlib.Path) -> dict:
        key = pathlib.Path(path).resolve()
        if key not in state_hash_cache:
            state_hash_cache[key] = _state_hash(sembla, key)
        return state_hash_cache[key]

    links = []
    prior_output_hash = None
    for year in range(start_year, end_year + 1):
        paths = _run_paths(out_dir, year)
        for label, path in paths.items():
            _require_file(path, f"{year} {label}")
        params_path = _require_file(params_dir / f"{year}.json", f"{year} parameters")
        target_path = _require_file(targets_dir / f"{year}.json", f"{year} targets")
        input_state = initial_state if year == start_year else out_dir / f"{year}.state"
        _require_file(input_state, f"{year} initial state")
        params_hash = _sha256(params_path)
        seed = derive_seed(
            model_identity=model_identity,
            scale=scale,
            run_year=year,
            params_sha256=params_hash,
            replica_index=replica_index,
        )
        manifest = _load_json(paths["manifest"])
        if manifest.get("manifest_kind") != "run":
            raise ChainError(f"{year}: manifest is not a run manifest")
        if manifest.get("model") != execution["model"]["name"]:
            raise ChainError(f"{year}: manifest model changed")
        if manifest.get("ticks") != 12 or manifest.get("dt") != 1.0:
            raise ChainError(f"{year}: annual tick contract changed")
        if manifest.get("seed") != seed["seed"]:
            raise ChainError(f"{year}: manifest seed does not match semantic seed")
        if manifest.get("ir_hash_algorithm") != "sha256" or manifest.get(
            "ir_hash"
        ) != model_identity:
            raise ChainError(f"{year}: manifest IR identity changed")
        if manifest.get("plan") != expected_plan:
            raise ChainError(f"{year}: manifest plan identity changed")
        if manifest.get("enabled_features") != enabled_features:
            raise ChainError(f"{year}: manifest feature set changed")
        backend_identity = manifest.get("backend_identity", {})
        if backend_identity.get("backend") != expected_backend:
            raise ChainError(f"{year}: manifest backend identity changed")

        expected_input_tuple = state_hash(input_state)
        expected_output_tuple = state_hash(paths["state"])
        if manifest.get("initial_state") != expected_input_tuple:
            raise ChainError(f"{year}: initial-state tuple does not match input artifact")
        if manifest.get("exported_state") != expected_output_tuple:
            raise ChainError(f"{year}: exported-state tuple does not match output artifact")
        if prior_output_hash is not None and expected_input_tuple["hash"] != prior_output_hash:
            raise ChainError(f"{year}: chain-link state hash mismatch")
        prior_output_hash = expected_output_tuple["hash"]
        if manifest.get("population_hash_algorithm") != "sha256" or manifest.get(
            "population_sha256"
        ) != _sha256(input_state):
            raise ChainError(f"{year}: initial-state raw digest mismatch")
        if manifest.get("population_source") != input_state.name:
            raise ChainError(f"{year}: initial-state source name mismatch")

        params = _load_json(params_path)
        if manifest.get("resolved_theta") != params:
            raise ChainError(f"{year}: resolved theta does not match parameter file")
        if manifest.get("results_hash_algorithm") != "sha256" or manifest.get(
            "results_sha256"
        ) != _sha256(paths["run"]):
            raise ChainError(f"{year}: scalar-results digest mismatch")
        if manifest.get("observation_hash_algorithm") != "sha256" or manifest.get(
            "observation_sha256"
        ) != _sha256(paths["summaries"]):
            raise ChainError(f"{year}: summary-observation digest mismatch")
        grouped_records = []
        grouped_manifest = manifest.get("grouped_outputs")
        if not isinstance(grouped_manifest, list):
            raise ChainError(f"{year}: grouped-output manifest changed")
        seen_views = set()
        for grouped in grouped_manifest:
            view = grouped.get("view")
            if not isinstance(view, str) or view in seen_views:
                raise ChainError(f"{year}: duplicate or invalid grouped view {view!r}")
            seen_views.add(view)
            grouped_path = _require_file(
                _grouped_path(paths["run"], view), f"{year} grouped {view}"
            )
            if grouped.get("algorithm") != "sha256" or grouped.get(
                "sha256"
            ) != _sha256(grouped_path):
                raise ChainError(f"{year}: grouped output {view!r} digest mismatch")
            grouped_records.append(
                _raw_record(grouped_path, grouped_path.relative_to(out_dir).as_posix())
                | {"view": view}
            )

        target_bytes = target_path.read_bytes()
        target = json.loads(target_bytes)
        if target.get("run_year") != year or target.get("scale", {}).get("name") != scale:
            raise ChainError(f"{year}: target run year or scale changed")
        score_report = _load_json(paths["score"])
        if score_report.get("format") != score.REPORT_FORMAT:
            raise ChainError(f"{year}: score report format changed")
        if score_report.get("mode") != "evaluation":
            raise ChainError(f"{year}: baseline score is not an evaluation report")
        if score_report.get("targets", {}).get("sha256") != targets_module.target_hash(
            target_bytes
        ):
            raise ChainError(f"{year}: score target digest mismatch")
        if score_report.get("run", {}).get("results_sha256") != manifest[
            "results_sha256"
        ]:
            raise ChainError(f"{year}: score/run results digest mismatch")
        expected_score = score.score_run(
            paths["run"], target_path, model_path, mode="evaluation"
        )
        if score_report != expected_score:
            raise ChainError(f"{year}: score report does not recompute from run bytes")

        capacity = inspect_capacity(paths["run"], model)
        require_capacity(capacity, str(year))
        links.append(
            {
                "link": f"{year}-{year + 1}",
                "run_year": year,
                "seed": seed,
                "parameters": _raw_record(
                    params_path, _relative_or_absolute(params_path)
                ),
                "targets": _raw_record(
                    target_path, _relative_or_absolute(target_path)
                )
                | {"domain_sha256": targets_module.target_hash(target_bytes)},
                "input_state": _raw_record(
                    input_state,
                    (
                        _relative_or_absolute(input_state)
                        if year == start_year
                        else input_state.relative_to(out_dir).as_posix()
                    ),
                )
                | {"state_artifact": expected_input_tuple},
                "output_state": _raw_record(
                    paths["state"], paths["state"].relative_to(out_dir).as_posix()
                )
                | {"state_artifact": expected_output_tuple},
                "results": _raw_record(
                    paths["run"], paths["run"].relative_to(out_dir).as_posix()
                ),
                "summaries": _raw_record(
                    paths["summaries"],
                    paths["summaries"].relative_to(out_dir).as_posix(),
                ),
                "manifest": _raw_record(
                    paths["manifest"],
                    paths["manifest"].relative_to(out_dir).as_posix(),
                ),
                "grouped_outputs": grouped_records,
                "score": _raw_record(
                    paths["score"], paths["score"].relative_to(out_dir).as_posix()
                ),
                "capacity": capacity,
            }
        )

    return {
        "format": CHAIN_FORMAT,
        "status": "uncalibrated_baseline",
        "scale": scale,
        "start_run_year": start_year,
        "end_run_year": end_year,
        "start_boundary": f"{start_year}-06-30",
        "end_boundary": f"{end_year + 1}-06-30",
        "replica_index": replica_index,
        "backend": backend,
        "enabled_features": enabled_features,
        "seed_domain": "sembla.australian-population-seed/v1",
        "model": _raw_record(model_path, _relative_or_absolute(model_path))
        | {"ir_sha256": model_identity},
        "plan": _raw_record(plan_path, _relative_or_absolute(plan_path))
        | {"semantic_hash": execution["plan"]["semantic_hash"]},
        "initial_state": _relative_or_absolute(initial_state),
        "parameter_directory": _relative_or_absolute(params_dir),
        "target_directory": _relative_or_absolute(targets_dir),
        "execution_contract": _raw_record(
            execution_path, _relative_or_absolute(execution_path)
        ),
        "sembla": {
            "path": _repository_display(sembla),
            "version": _binary_version(sembla),
            "raw_sha256": _sha256(sembla),
        },
        "link_count": len(links),
        "links": links,
    }


def verify_chain(
    out_dir: pathlib.Path,
    *,
    sembla_override: pathlib.Path | None = None,
) -> dict:
    """Rebuild the chain report from bytes and require an exact match."""
    out_dir = pathlib.Path(out_dir)
    report_path = _require_file(out_dir / "chain-report.json", "chain report")
    report_bytes = report_path.read_bytes()
    report = json.loads(report_bytes)
    canonical_bytes = (
        json.dumps(report, sort_keys=True, indent=2, ensure_ascii=False) + "\n"
    ).encode("utf-8")
    if report_bytes != canonical_bytes:
        raise ChainError("chain report is not canonical JSON")
    if report.get("format") != CHAIN_FORMAT:
        raise ChainError("chain report format changed")
    # Never execute a path supplied by an untrusted report. The caller chooses
    # the verifier binary explicitly, or accepts the repository default.
    sembla = pathlib.Path(sembla_override or DEFAULT_SEMBLA)
    _require_file(sembla, "trusted Sembla verifier binary")
    expected = _build_report(
        out_dir=out_dir,
        scale=report["scale"],
        start_year=report["start_run_year"],
        end_year=report["end_run_year"],
        replica_index=report["replica_index"],
        backend=report["backend"],
        enabled_features=report["enabled_features"],
        params_dir=_resolve_recorded_path(report["parameter_directory"]),
        targets_dir=_resolve_recorded_path(report["target_directory"]),
        model_path=_resolve_recorded_path(report["model"]["path"]),
        plan_path=_resolve_recorded_path(report["plan"]["path"]),
        initial_state=_resolve_recorded_path(report["initial_state"]),
        sembla=sembla,
    )
    if expected != report:
        raise ChainError("chain report does not match current chain bytes")
    return report


def run_chain(args) -> dict:
    _validate_year_range(args.start_year, args.end_year)
    out_dir = pathlib.Path(args.out)
    if out_dir.exists() and any(out_dir.iterdir()):
        raise ChainError(
            f"output directory must be absent or empty (no checkpoint/resume): {out_dir}"
        )
    out_dir.mkdir(parents=True, exist_ok=True)
    params_dir = _require_file(args.params_dir / f"{args.start_year}.json", "first parameter file").parent
    targets_dir = _require_file(args.targets_dir / f"{args.start_year}.json", "first target ledger").parent
    model_path, plan_path, default_state = _resolve_defaults(args.scale, args)
    model_path = _require_file(model_path, "model")
    plan_path = _require_file(plan_path, "plan")
    sembla = _require_file(args.sembla, "Sembla binary")
    if args.start_year != 2010 and args.initial_state is None:
        raise ChainError("a subset beginning after 2010 requires --initial-state")
    initial_state = _require_file(default_state, "initial state")
    enabled_features = args.enable or []
    if FEATURE not in enabled_features:
        raise ChainError(
            "Australian target scoring requires --enable grouped-observations"
        )
    if len(enabled_features) != len(set(enabled_features)):
        raise ChainError("enabled features contain duplicates")

    execution = _load_json(targets_dir / "execution.json")
    model_identity = execution.get("model", {}).get("ir_hash", {}).get("digest")
    if not _is_sha256(model_identity):
        raise ChainError("target execution contract has no model identity")

    for year in range(args.start_year, args.end_year + 1):
        params_path = _require_file(
            params_dir / f"{year}.json", f"{year} parameter file"
        )
        target_path = _require_file(
            targets_dir / f"{year}.json", f"{year} target ledger"
        )
        target = _load_json(target_path)
        if target.get("run_year") != year or target.get("scale", {}).get("name") != args.scale:
            raise ChainError(f"{year}: target run year or scale does not match request")
        input_state = initial_state if year == args.start_year else out_dir / f"{year}.state"
        _require_file(input_state, f"{year} input state")
        paths = _run_paths(out_dir, year)
        seed = derive_seed(
            model_identity=model_identity,
            scale=args.scale,
            run_year=year,
            params_sha256=_sha256(params_path),
            replica_index=args.replica_index,
        )
        command = [
            str(sembla),
            "run",
            str(plan_path),
            "--population",
            str(input_state),
            "--seed",
            str(seed["seed"]),
            "--ticks",
            "12",
            "--params",
            str(params_path),
            "--backend",
            args.backend,
            "--out",
            str(paths["run"]),
            "--export-state",
            str(paths["state"]),
        ]
        for feature in enabled_features:
            command.extend(["--enable", feature])
        process = subprocess.run(command, text=True, capture_output=True)
        if process.returncode:
            raise ChainError(
                f"{year}: Sembla run failed\nstdout={process.stdout}\nstderr={process.stderr}"
            )
        if "deferred exceeds 10%" in process.stderr:
            raise ChainError(f"{year}: runtime emitted a saturation warning")
        capacity = inspect_capacity(paths["run"], json.loads(model_path.read_bytes()))
        require_capacity(capacity, str(year))
        report = score.score_run(
            paths["run"],
            target_path,
            model_path,
            mode="evaluation",
        )
        canonical.write_json(paths["score"], report)
        print(
            f"year={year} seed={seed['seed']} "
            f"population_score_mae={report['metrics']['overall']['mae']}",
            flush=True,
        )

    report = _build_report(
        out_dir=out_dir,
        scale=args.scale,
        start_year=args.start_year,
        end_year=args.end_year,
        replica_index=args.replica_index,
        backend=args.backend,
        enabled_features=enabled_features,
        params_dir=params_dir,
        targets_dir=targets_dir,
        model_path=model_path,
        plan_path=plan_path,
        initial_state=initial_state,
        sembla=sembla,
    )
    canonical.write_json(out_dir / "chain-report.json", report)
    verify_chain(out_dir, sembla_override=sembla)
    print(f"verified {len(report['links'])} chain links", flush=True)
    return report


def _inventory(directory: pathlib.Path) -> dict[str, dict]:
    directory = pathlib.Path(directory)
    result = {}
    for path in sorted(directory.rglob("*")):
        if path.is_file():
            relative = path.relative_to(directory).as_posix()
            result[relative] = {
                "bytes": path.stat().st_size,
                "raw_sha256": _sha256(path),
            }
    return result


def compare_chains(left: pathlib.Path, right: pathlib.Path) -> dict:
    left_inventory = _inventory(left)
    right_inventory = _inventory(right)
    if left_inventory != right_inventory:
        missing = sorted(set(left_inventory) - set(right_inventory))
        extra = sorted(set(right_inventory) - set(left_inventory))
        changed = sorted(
            path
            for path in set(left_inventory) & set(right_inventory)
            if left_inventory[path] != right_inventory[path]
        )
        raise ChainError(
            f"chain outputs differ: missing={missing!r}, extra={extra!r}, "
            f"changed={changed!r}"
        )
    return {
        "format": REPRODUCTION_FORMAT,
        "byte_identical": True,
        "file_count": len(left_inventory),
        "files": [
            {"path": path, **metadata}
            for path, metadata in left_inventory.items()
        ],
    }


def compare_middle_year(
    chain_dir: pathlib.Path,
    isolated_dir: pathlib.Path,
    year: int,
) -> dict:
    if year not in RUN_YEARS:
        raise ChainError("middle-year comparison year is outside 2010..2024")
    chain_report = _load_json(pathlib.Path(chain_dir) / "chain-report.json")
    if chain_report.get("format") != CHAIN_FORMAT:
        raise ChainError("middle-year comparison requires a verified chain report")
    if not chain_report["start_run_year"] < year < chain_report["end_run_year"]:
        raise ChainError("middle-year comparison requires an interior run year")
    run_name = f"{year}.csv"
    manifest = _load_json(pathlib.Path(chain_dir) / f"{run_name}.manifest.json")
    grouped = manifest.get("grouped_outputs")
    if not isinstance(grouped, list):
        raise ChainError(f"full chain has no grouped-output inventory for {year}")
    expected_names = {
        run_name,
        f"{run_name}.manifest.json",
        f"{run_name}.summaries.csv",
        f"{year}.score.json",
        f"{year + 1}.state",
        *(f"{year}.grouped.{row['view']}.csv" for row in grouped),
    }
    records = []
    for name in sorted(expected_names):
        left = _require_file(pathlib.Path(chain_dir) / name, f"full-chain {name}")
        right = _require_file(pathlib.Path(isolated_dir) / name, f"isolated {name}")
        left_record = {"bytes": left.stat().st_size, "raw_sha256": _sha256(left)}
        right_record = {"bytes": right.stat().st_size, "raw_sha256": _sha256(right)}
        if left_record != right_record:
            raise ChainError(f"isolated middle-year output changed: {name}")
        records.append({"path": name, **left_record})
    return {
        "format": MIDDLE_REPRODUCTION_FORMAT,
        "byte_identical": True,
        "run_year": year,
        "file_count": len(records),
        "files": records,
    }


def _family_metrics(rows: list[dict]) -> dict:
    target = sum(row["target"] for row in rows)
    observed = sum(row["observed"] for row in rows)
    absolute = sum(row["absolute_error"] for row in rows)
    return {
        "target": target,
        "observed": observed,
        "signed_error": observed - target,
        "wape": absolute / target if target else None,
    }


def _baseline_rows(report: dict, chain_dir: pathlib.Path):
    drift = []
    components = []
    score_reports = {}
    families = (
        "births_state",
        "derived_deaths_state_total",
        "overseas_arrivals_state",
        "overseas_departures_state",
        "interstate_od",
    )
    for link in report["links"]:
        year = link["run_year"]
        score_report = _load_json(chain_dir / f"{year}.score.json")
        score_reports[year] = score_report
        stock = [
            row for row in score_report["residuals"]
            if row["family"] == "derived_stock_state_total"
        ]
        if len(stock) != 8:
            raise ChainError(f"{year}: score report lacks eight state totals")
        for row in stock:
            drift.append(
                (
                    year,
                    year + 1,
                    row["state"],
                    row["target"],
                    row["observed"],
                    row["signed_error"],
                    row["signed_error"] / row["target"],
                )
            )
        target_total = sum(row["target"] for row in stock)
        observed_total = sum(row["observed"] for row in stock)
        drift.append(
            (
                year,
                year + 1,
                "eight_state_total",
                target_total,
                observed_total,
                observed_total - target_total,
                (observed_total - target_total) / target_total,
            )
        )
        for family in families:
            rows = [row for row in score_report["residuals"] if row["family"] == family]
            metrics = _family_metrics(rows)
            components.append(
                (
                    year,
                    family,
                    metrics["target"],
                    metrics["observed"],
                    metrics["signed_error"],
                    metrics["wape"],
                )
            )
    return drift, components, score_reports


def write_evidence(
    *,
    chain_dir: pathlib.Path,
    evidence_dir: pathlib.Path,
    reproduction_chain: pathlib.Path,
    middle_chain: pathlib.Path,
    middle_run_year: int,
    sembla_override: pathlib.Path | None,
) -> dict:
    report = verify_chain(chain_dir, sembla_override=sembla_override)
    if (
        report["scale"] != "hundredth"
        or report["start_run_year"] != 2010
        or report["end_run_year"] != 2024
        or report["link_count"] != 15
        or [link["run_year"] for link in report["links"]] != list(RUN_YEARS)
    ):
        raise ChainError(
            "baseline evidence requires the complete hundredth-scale 2010..2024 chain"
        )
    repeated_report = verify_chain(
        reproduction_chain, sembla_override=sembla_override
    )
    if repeated_report != report:
        raise ChainError("repeated full chain report differs from the primary chain")
    reproduction = compare_chains(chain_dir, reproduction_chain)

    isolated_report = verify_chain(middle_chain, sembla_override=sembla_override)
    if (
        not report["start_run_year"] < middle_run_year < report["end_run_year"]
        or isolated_report["start_run_year"] != middle_run_year
        or isolated_report["end_run_year"] != middle_run_year
        or isolated_report["link_count"] != 1
        or isolated_report["scale"] != report["scale"]
        or isolated_report["model"] != report["model"]
        or isolated_report["plan"] != report["plan"]
        or isolated_report["replica_index"] != report["replica_index"]
    ):
        raise ChainError("isolated reproduction is not the requested interior run year")
    primary_link = next(
        link for link in report["links"] if link["run_year"] == middle_run_year
    )
    isolated_link = isolated_report["links"][0]
    if (
        primary_link["input_state"]["raw_sha256"]
        != isolated_link["input_state"]["raw_sha256"]
        or primary_link["input_state"]["state_artifact"]
        != isolated_link["input_state"]["state_artifact"]
    ):
        raise ChainError("isolated middle year did not use the same boundary state")
    middle = compare_middle_year(chain_dir, middle_chain, middle_run_year)

    evidence_dir = pathlib.Path(evidence_dir)
    if evidence_dir.exists() and any(evidence_dir.iterdir()):
        raise ChainError(f"evidence directory is not empty: {evidence_dir}")
    evidence_dir.mkdir(parents=True, exist_ok=True)
    residual_dir = evidence_dir / "residuals"
    residual_dir.mkdir()
    shutil.copyfile(chain_dir / "chain-report.json", evidence_dir / "chain-report.json")
    canonical.write_json(evidence_dir / "reproduction.json", reproduction)
    canonical.write_json(evidence_dir / "middle-year-reproduction.json", middle)
    for link in report["links"]:
        year = link["run_year"]
        shutil.copyfile(
            chain_dir / f"{year}.score.json",
            residual_dir / f"{year}.json",
        )

    drift, components, score_reports = _baseline_rows(report, chain_dir)
    canonical.write_csv(
        evidence_dir / "erp-drift.csv",
        [
            "run_year",
            "boundary_year",
            "state",
            "target",
            "simulated",
            "signed_error",
            "relative_error",
        ],
        drift,
        sort=False,
    )
    canonical.write_csv(
        evidence_dir / "component-error.csv",
        ["run_year", "family", "target", "simulated", "signed_error", "wape"],
        components,
        sort=False,
    )
    final_year = report["end_run_year"]
    final_rows = [row for row in drift if row[0] == final_year and row[2] in STATES]
    canonical.write_csv(
        evidence_dir / "final-population-by-state.csv",
        ["state", "target", "simulated", "signed_error", "relative_error"],
        [(row[2], row[3], row[4], row[5], row[6]) for row in final_rows],
        sort=False,
    )

    national_rows = [row for row in drift if row[2] == "eight_state_total"]
    final_national = next(row for row in national_rows if row[0] == final_year)
    maximum_drift = max(national_rows, key=lambda row: abs(row[6]))
    minimum_birth = min(
        link["capacity"]["minimum_vacant_birth_slots"] for link in report["links"]
    )
    minimum_overseas = min(
        link["capacity"]["minimum_vacant_overseas_slots"] for link in report["links"]
    )
    maximum_saturation = max(
        report["links"],
        key=lambda link: link["capacity"]["maximum_saturation"][
            "deferred_to_fired_ratio"
        ],
    )
    summary = {
        "format": EVIDENCE_FORMAT,
        "status": "uncalibrated_baseline",
        "chain_report_raw_sha256": _sha256(evidence_dir / "chain-report.json"),
        "reproduction_raw_sha256": _sha256(evidence_dir / "reproduction.json"),
        "middle_year_reproduction_raw_sha256": _sha256(
            evidence_dir / "middle-year-reproduction.json"
        ),
        "run_years": [link["run_year"] for link in report["links"]],
        "final_boundary_year": final_year + 1,
        "final_eight_state_erp": {
            "target": final_national[3],
            "simulated": final_national[4],
            "signed_error": final_national[5],
            "relative_error": final_national[6],
        },
        "maximum_absolute_national_relative_drift": {
            "run_year": maximum_drift[0],
            "boundary_year": maximum_drift[1],
            "relative_error": maximum_drift[6],
        },
        "capacity": {
            "minimum_vacant_birth_slots": minimum_birth,
            "minimum_vacant_overseas_slots": minimum_overseas,
            "maximum_deferred_to_fired_ratio": maximum_saturation["capacity"][
                "maximum_saturation"
            ]["deferred_to_fired_ratio"],
            "maximum_ratio_run_year": maximum_saturation["run_year"],
        },
        "residual_report_count": len(score_reports),
    }
    canonical.write_json(evidence_dir / "summary.json", summary)

    component_by_year = {}
    for row in components:
        component_by_year.setdefault(row[0], {})[row[1]] = row
    lines = [
        "# Australian population: uncalibrated 2010–2025 baseline",
        "",
        "This is the PRD 0007 baseline, not a calibrated forecast. The seventeen",
        "free migration entries in every annual parameter file remain their shared",
        "pre-calibration defaults; PRD 0008 is responsible for fitting them. Only",
        "the fixed ABS-derived fertility, mortality and overseas rates vary annually.",
        "",
        "## Reproduction and chain identity",
        "",
        f"- Scale: `{report['scale']}`; replica index: `{report['replica_index']}`.",
        f"- Run years: {report['start_run_year']}–{report['end_run_year']} (boundaries "
        f"{report['start_boundary']} to {report['end_boundary']}).",
        f"- Sembla: `{report['sembla']['version']}`, raw SHA-256 "
        f"`{report['sembla']['raw_sha256']}`.",
        f"- Chain report SHA-256: `{summary['chain_report_raw_sha256']}`.",
        f"- All {reproduction['file_count']} generated chain files reproduced byte for byte.",
        f"- Run year {middle['run_year']} reproduced byte for byte in isolation "
        f"({middle['file_count']} files).",
        "- Every input/output state tuple, raw state byte hash, model/plan identity,",
        "  annual parameter bytes, scalar/summary/grouped output and score report is",
        "  checked by `scripts/verify-population-chain.sh`.",
        "",
        "```bash",
        "cargo build --release --locked",
        "scripts/run-australian-population.sh --scale hundredth \\",
        "  --start-year 2010 --end-year 2024 \\",
        "  --params-dir data/abs/params --targets-dir data/abs/targets \\",
        "  --out /tmp/australian-baseline --backend cpu \\",
        "  --enable grouped-observations",
        "scripts/verify-population-chain.sh --out /tmp/australian-baseline",
        "scripts/run-australian-population.sh --scale hundredth \\",
        "  --start-year 2010 --end-year 2024 \\",
        "  --params-dir data/abs/params --targets-dir data/abs/targets \\",
        "  --out /tmp/australian-baseline-repeat --backend cpu \\",
        "  --enable grouped-observations",
        "scripts/run-australian-population.sh --scale hundredth \\",
        "  --start-year 2017 --end-year 2017 \\",
        "  --initial-state /tmp/australian-baseline/2017.state \\",
        "  --params-dir data/abs/params --targets-dir data/abs/targets \\",
        "  --out /tmp/australian-baseline-2017 --backend cpu \\",
        "  --enable grouped-observations",
        "python3 data/abs/chain.py evidence \\",
        "  --chain /tmp/australian-baseline \\",
        "  --reproduction-chain /tmp/australian-baseline-repeat \\",
        "  --middle-chain /tmp/australian-baseline-2017 --middle-run-year 2017 \\",
        "  --out docs/evidence/australian-population/baseline-2026-08-06",
        "```", 
        "",
        "The driver refuses a non-empty output directory. This is deliberate: a",
        "subset is a new annual run with an explicit input state, not checkpoint",
        "resume. Seeds are SHA-256-derived from model identity, scale, run year,",
        "annual parameter-file digest and replica index; list position and output",
        "path never enter the coordinate.",
        "",
        "## Eight-state ERP drift",
        "",
        "| boundary | target | simulated | signed error | relative error |",
        "|---:|---:|---:|---:|---:|",
    ]
    for row in national_rows:
        lines.append(
            f"| {row[1]} | {int(row[3]):,} | {int(row[4]):,} | "
            f"{int(row[5]):+,} | {row[6] * 100:+.3f}% |"
        )
    lines.extend(
        [
            "",
            f"The terminal 30 June {final_year + 1} eight-state population is "
            f"{int(final_national[4]):,} against {int(final_national[3]):,}: "
            f"{final_national[6] * 100:+.3f}% drift. The largest absolute national "
            f"relative drift is {maximum_drift[6] * 100:+.3f}% at the "
            f"{maximum_drift[1]} boundary.",
            "",
            f"### Final 30 June {final_year + 1} population by state",
            "",
            "| state | target | simulated | signed error | relative error |",
            "|---|---:|---:|---:|---:|",
            *[
                f"| {row[2]} | {int(row[3]):,} | {int(row[4]):,} | "
                f"{int(row[5]):+,} | {row[6] * 100:+.3f}% |"
                for row in final_rows
            ],
            "",
            "## Annual component errors",
            "",
            "WAPE is total absolute cell error divided by the published family total.",
            "The interstate result is expected to be poor before PRD 0008 because",
            "`interstate_base`, push/pull factors, `peak_months` and `k` are not fitted.",
            "",
            "| run year | births WAPE | deaths WAPE | arrivals WAPE | departures WAPE | O-D WAPE |",
            "|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for year in range(report["start_run_year"], report["end_run_year"] + 1):
        values = component_by_year[year]
        lines.append(
            f"| {year} | {values['births_state'][5] * 100:.3f}% | "
            f"{values['derived_deaths_state_total'][5] * 100:.3f}% | "
            f"{values['overseas_arrivals_state'][5] * 100:.3f}% | "
            f"{values['overseas_departures_state'][5] * 100:.3f}% | "
            f"{values['interstate_od'][5] * 100:.3f}% |"
        )
    lines.extend(
        [
            "",
            "## Capacity and interpretation",
            "",
            f"- Minimum vacant birth slots: {minimum_birth:,}.",
            f"- Minimum vacant overseas slots: {minimum_overseas:,}.",
            f"- Maximum deferred/fired ratio: "
            f"{summary['capacity']['maximum_deferred_to_fired_ratio'] * 100:.6f}% "
            f"in run year {summary['capacity']['maximum_ratio_run_year']}.",
            "- No year crossed the strict 10% saturation threshold or reached a zero",
            "  vacancy margin; either condition makes the driver fail.",
            "",
            "Birth/death registration years, financial-year migration and 30 June ERP",
            "stocks do not form an exact accounting identity. Other Territories remain",
            "outside the eight-state model. Residuals are therefore evidence, not values",
            "to be silently reconciled. Detailed per-year reports are under `residuals/`.",
            "",
            "## Chained versus continuous runs",
            "",
            "A chained 12+12 pair is intentionally not bitwise equal to one continuous",
            "24-tick run because the second annual window restarts tick coordinates and",
            "has its own semantic seed and theta. This is asserted by",
            "`crates/sembla-cli/tests/chained_runs.rs`; annual state artifacts are chain",
            "links, not hidden checkpoints.",
        ]
    )
    (evidence_dir / "README.md").write_text(
        "\n".join(lines) + "\n", encoding="utf-8", newline=""
    )
    return summary


def _add_shared_run_arguments(parser: argparse.ArgumentParser) -> None:
    parser.add_argument("--scale", choices=SCALES, required=True)
    parser.add_argument("--start-year", type=int, required=True)
    parser.add_argument("--end-year", type=int, required=True)
    parser.add_argument("--params-dir", type=pathlib.Path, required=True)
    parser.add_argument("--targets-dir", type=pathlib.Path, required=True)
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--backend", choices=("cpu", "cuda"), required=True)
    parser.add_argument("--replica-index", type=int, default=0)
    parser.add_argument("--enable", action="append", choices=(FEATURE,))
    parser.add_argument("--sembla", type=pathlib.Path, default=DEFAULT_SEMBLA)
    parser.add_argument("--model", type=pathlib.Path)
    parser.add_argument("--plan", type=pathlib.Path)
    parser.add_argument("--initial-state", type=pathlib.Path)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    seed_parser = subparsers.add_parser("seed", help="derive one semantic annual seed")
    seed_parser.add_argument("--model-identity", required=True)
    seed_parser.add_argument("--scale", choices=SCALES, required=True)
    seed_parser.add_argument("--run-year", type=int, required=True)
    seed_parser.add_argument("--params", type=pathlib.Path, required=True)
    seed_parser.add_argument("--replica-index", type=int, default=0)

    run_parser = subparsers.add_parser("run", help="run and verify an annual chain")
    _add_shared_run_arguments(run_parser)

    verify_parser = subparsers.add_parser("verify", help="verify an existing chain")
    verify_parser.add_argument("--out", type=pathlib.Path, required=True)
    verify_parser.add_argument("--sembla", type=pathlib.Path)

    capacity_parser = subparsers.add_parser(
        "check-capacity", help="fail on saturation or zero vacancy"
    )
    capacity_parser.add_argument("--run", type=pathlib.Path, required=True)
    capacity_parser.add_argument("--model", type=pathlib.Path, required=True)

    compare_parser = subparsers.add_parser(
        "compare", help="prove two complete chain directories are byte-identical"
    )
    compare_parser.add_argument("--left", type=pathlib.Path, required=True)
    compare_parser.add_argument("--right", type=pathlib.Path, required=True)
    compare_parser.add_argument("--out", type=pathlib.Path, required=True)

    middle_parser = subparsers.add_parser(
        "compare-middle", help="prove one isolated run year is byte-identical"
    )
    middle_parser.add_argument("--chain", type=pathlib.Path, required=True)
    middle_parser.add_argument("--isolated", type=pathlib.Path, required=True)
    middle_parser.add_argument("--run-year", type=int, required=True)
    middle_parser.add_argument("--out", type=pathlib.Path, required=True)

    evidence_parser = subparsers.add_parser(
        "evidence", help="write the committed baseline evidence package"
    )
    evidence_parser.add_argument("--chain", type=pathlib.Path, required=True)
    evidence_parser.add_argument("--out", type=pathlib.Path, required=True)
    evidence_parser.add_argument(
        "--reproduction-chain", type=pathlib.Path, required=True
    )
    evidence_parser.add_argument("--middle-chain", type=pathlib.Path, required=True)
    evidence_parser.add_argument("--middle-run-year", type=int, required=True)
    evidence_parser.add_argument("--sembla", type=pathlib.Path)

    args = parser.parse_args(argv)
    try:
        if args.command == "seed":
            result = derive_seed(
                model_identity=args.model_identity,
                scale=args.scale,
                run_year=args.run_year,
                params_sha256=_sha256(args.params),
                replica_index=args.replica_index,
            )
            print(json.dumps(result, sort_keys=True, separators=(",", ":")))
        elif args.command == "run":
            run_chain(args)
        elif args.command == "verify":
            report = verify_chain(args.out, sembla_override=args.sembla)
            print(f"verified {report['link_count']} chain links")
        elif args.command == "check-capacity":
            capacity = inspect_capacity(args.run, _load_json(args.model))
            require_capacity(capacity, str(args.run))
            print(json.dumps(capacity, sort_keys=True, separators=(",", ":")))
        elif args.command == "compare":
            proof = compare_chains(args.left, args.right)
            canonical.write_json(args.out, proof)
            print(f"verified {proof['file_count']} byte-identical files")
        elif args.command == "compare-middle":
            proof = compare_middle_year(args.chain, args.isolated, args.run_year)
            canonical.write_json(args.out, proof)
            print(f"verified isolated run year {args.run_year}")
        elif args.command == "evidence":
            summary = write_evidence(
                chain_dir=args.chain,
                evidence_dir=args.out,
                reproduction_chain=args.reproduction_chain,
                middle_chain=args.middle_chain,
                middle_run_year=args.middle_run_year,
                sembla_override=args.sembla,
            )
            print(
                f"wrote baseline evidence; final relative drift="
                f"{summary['final_eight_state_erp']['relative_error']}"
            )
        else:  # pragma: no cover - argparse enforces commands.
            raise AssertionError(args.command)
    except (ChainError, OSError, KeyError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

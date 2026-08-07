#!/usr/bin/env python3
"""Build canonical ``sembla.targets/v1`` Australian target ledgers.

The builder is offline and standard-library only.  It maps committed ABS
extract cells to exact model grouped observations, records fitted/held-out
roles and full/reduced projections, validates every selector against the
exported model, and writes domain-separated hashes to ``targets/index.json``.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
import hashlib
import json
import pathlib
from typing import Iterable

import canonical


HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"
SOURCES = HERE / "sources.json"
TARGETS = HERE / "targets"
ROOT = HERE.parents[1]
DEFAULT_MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"
DEFAULT_PLAN = ROOT / "fixtures/australian-population/australian_population.hundredth.plan.json"

FORMAT = "sembla.targets/v1"
INDEX_FORMAT = "sembla.targets-index/v1"
EXECUTION_FORMAT = "sembla.targets-execution/v1"
# These public-run identities are checked against an actual run manifest by the
# Rust integration test.  Raw-byte guards make model/plan regeneration fail
# loudly until all four identities are reviewed together.
EXECUTION_MODEL_RAW_SHA256 = "6f307418a367d44d6cde56c18c57d6873981a7187bc17fa94ca3c5f9110ed904"
EXECUTION_PLAN_RAW_SHA256 = "6f05d870681f35950deab4c74a8a9e816b43625a744f9e8cf2d5aa96a2ef18fe"
EXECUTION_IR_SHA256 = "3d249dbc8c39249b234daa6d3af7a44bae77acf154a830b1b81180afbcd0f62e"
EXECUTION_PLAN_SEMANTIC_SHA256 = "0e24d88398abaaaae7842d21aacf8c7dd9ae5f882c0f6d39c97d03c7aa74e5bf"
HASH_DOMAIN = b"sembla.targets/v1\0"
GEOGRAPHY_VERSION = "australian_states_and_territories/v1"
SCALE_NAME = "hundredth"
SCALE_FACTOR = 100
RUN_YEARS = tuple(range(2010, 2025))
STATES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")
SEXES = ("male", "female")
MODEL_AGE_BANDS = (
    ("00_04", tuple(range(0, 5)), 0),
    ("05_09", tuple(range(5, 10)), 1),
    ("10_14", tuple(range(10, 15)), 2),
    ("15_19", tuple(range(15, 20)), 3),
    ("20_24", tuple(range(20, 25)), 4),
    ("25_29", tuple(range(25, 30)), 5),
    ("30_34", tuple(range(30, 35)), 6),
    ("35_39", tuple(range(35, 40)), 7),
    ("40_44", tuple(range(40, 45)), 8),
    ("45_49", tuple(range(45, 50)), 9),
    ("50_54", tuple(range(50, 55)), 10),
    ("55_59", tuple(range(55, 60)), 11),
    ("60_64", tuple(range(60, 65)), 12),
    ("65_69", tuple(range(65, 70)), 13),
    ("70_74", tuple(range(70, 75)), 14),
    ("75_79", tuple(range(75, 80)), 15),
    ("80_84", tuple(range(80, 85)), 16),
    ("85_89", tuple(range(85, 90)), 17),
    ("90_94", tuple(range(90, 95)), 18),
    ("95_99", tuple(range(95, 100)), 19),
    ("100_plus", (100,), 20),
)
INTERSTATE_AGE_BANDS = (
    ("00_04", "0-4", 0),
    ("05_09", "5-9", 1),
    ("10_14", "10-14", 2),
    ("15_19", "15-19", 3),
    ("20_24", "20-24", 4),
    ("25_29", "25-29", 5),
    ("30_34", "30-34", 6),
    ("35_39", "35-39", 7),
    ("40_44", "40-44", 8),
    ("45_49", "45-49", 9),
    ("50_54", "50-54", 10),
    ("55_59", "55-59", 11),
    ("60_64", "60-64", 12),
    ("65_69", "65-69", 13),
    ("70_74", "70-74", 14),
    ("75_plus", "75+", 15),
)

EXPECTED_HEADERS = {
    "erp_state_age_sex.csv": ["year", "state", "sex", "age", "persons"],
    "births_state.csv": ["year", "state", "births"],
    "deaths_state_age_sex.csv": ["year", "state", "sex", "age_band", "deaths"],
    "overseas_margins.csv": ["run_year", "state", "arrivals", "departures"],
    "interstate_flows.csv": ["year", "origin", "destination", "persons"],
    "interstate_state_age_sex.csv": [
        "year", "state", "sex", "age_band", "arrivals", "departures"
    ],
    "interstate_margins.csv": ["run_year", "state", "arrivals", "departures"],
}

REQUIRED_GROUPED_VIEWS = {
    "population_cells": ("area", "sex", "age_months"),
    "population_single_year_cells": ("area", "sex", "age_months"),
    "births_cells": ("area", "sex"),
    "deaths_state_age_cells": ("area", "sex", "event_age_months"),
    "overseas_arrival_cells": ("area", "sex"),
    "overseas_departure_cells": ("area", "sex"),
    "interstate_flows": ("prev_area", "area"),
    "interstate_age_sex_flows": (
        "prev_area", "area", "sex", "event_age_months"
    ),
}


@dataclass(frozen=True)
class Inputs:
    erp: dict[tuple[int, str, str, int], int]
    births: dict[tuple[int, str], int]
    deaths: dict[tuple[int, str, str, str], int]
    overseas: dict[tuple[int, str], tuple[int, int]]
    interstate_od: dict[tuple[int, str, str], int]
    interstate_detail: dict[tuple[int, str, str, str], tuple[int, int]]
    interstate_margins: dict[tuple[int, str], tuple[int, int]]
    sources: dict[str, dict[str, object]]
    sources_sha256: str


def _read_rows(name: str, extracts: pathlib.Path = EXTRACTS) -> list[dict[str, str]]:
    with (pathlib.Path(extracts) / name).open(newline="", encoding="utf-8") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames != EXPECTED_HEADERS[name]:
            raise ValueError(
                f"{name} schema changed: {reader.fieldnames!r}; "
                f"expected {EXPECTED_HEADERS[name]!r}"
            )
        return list(reader)


def _integer(text: str, label: str) -> int:
    try:
        value = int(text)
    except ValueError as error:
        raise ValueError(f"{label} is not an integer: {text!r}") from error
    if value < 0:
        raise ValueError(f"{label} is negative: {value}")
    return value


def _insert(mapping: dict, key, value, label: str) -> None:
    if key in mapping:
        raise ValueError(f"{label} contains duplicate cell {key!r}")
    mapping[key] = value


def _require_grid(label: str, actual: set, expected: set) -> None:
    if actual != expected:
        raise ValueError(
            f"{label} grid changed: missing={sorted(expected - actual)[:10]!r}, "
            f"extra={sorted(actual - expected)[:10]!r}"
        )


def load_inputs(
    extracts: pathlib.Path = EXTRACTS,
    sources_path: pathlib.Path = SOURCES,
) -> Inputs:
    """Load every source grid used by target construction."""
    erp = {}
    for row in _read_rows("erp_state_age_sex.csv", extracts):
        key = (int(row["year"]), row["state"], row["sex"], int(row["age"]))
        _insert(erp, key, _integer(row["persons"], f"ERP {key}"), "ERP")
    _require_grid(
        "ERP",
        set(erp),
        {
            (year, state, sex, age)
            for year in range(2010, 2026)
            for state in STATES
            for sex in SEXES
            for age in range(101)
        },
    )

    births = {}
    for row in _read_rows("births_state.csv", extracts):
        key = (int(row["year"]), row["state"])
        _insert(births, key, _integer(row["births"], f"births {key}"), "births")
    _require_grid(
        "births", set(births),
        {(year, state) for year in RUN_YEARS for state in STATES},
    )

    deaths = {}
    death_bands = {label.replace("_", "-") for label, _ages, _index in MODEL_AGE_BANDS}
    death_bands = {
        "0-4", "5-9", "10-14", "15-19", "20-24", "25-29", "30-34",
        "35-39", "40-44", "45-49", "50-54", "55-59", "60-64", "65-69",
        "70-74", "75-79", "80-84", "85-89", "90-94", "95-99", "100+",
        "not_stated",
    }
    for row in _read_rows("deaths_state_age_sex.csv", extracts):
        key = (int(row["year"]), row["state"], row["sex"], row["age_band"])
        _insert(deaths, key, _integer(row["deaths"], f"deaths {key}"), "deaths")
    _require_grid(
        "deaths", set(deaths),
        {
            (year, state, sex, band)
            for year in RUN_YEARS for state in STATES for sex in SEXES
            for band in death_bands
        },
    )

    overseas = {}
    for row in _read_rows("overseas_margins.csv", extracts):
        key = (int(row["run_year"]), row["state"])
        value = (
            _integer(row["arrivals"], f"overseas arrivals {key}"),
            _integer(row["departures"], f"overseas departures {key}"),
        )
        _insert(overseas, key, value, "overseas margins")
    _require_grid(
        "overseas margins", set(overseas),
        {(year, state) for year in RUN_YEARS for state in STATES},
    )

    interstate_od = {}
    for row in _read_rows("interstate_flows.csv", extracts):
        key = (int(row["year"]), row["origin"], row["destination"])
        _insert(
            interstate_od, key, _integer(row["persons"], f"interstate O-D {key}"),
            "interstate O-D",
        )
    _require_grid(
        "interstate O-D", set(interstate_od),
        {
            (year, origin, destination)
            for year in RUN_YEARS for origin in STATES for destination in STATES
            if origin != destination
        },
    )

    interstate_detail = {}
    detail_bands = {source for _name, source, _index in INTERSTATE_AGE_BANDS}
    for row in _read_rows("interstate_state_age_sex.csv", extracts):
        key = (int(row["year"]), row["state"], row["sex"], row["age_band"])
        value = (
            _integer(row["arrivals"], f"interstate detail arrivals {key}"),
            _integer(row["departures"], f"interstate detail departures {key}"),
        )
        _insert(interstate_detail, key, value, "interstate detail")
    _require_grid(
        "interstate detail", set(interstate_detail),
        {
            (year, state, sex, band)
            for year in RUN_YEARS for state in STATES for sex in SEXES
            for band in detail_bands
        },
    )

    interstate_margins = {}
    for row in _read_rows("interstate_margins.csv", extracts):
        key = (int(row["run_year"]), row["state"])
        value = (
            _integer(row["arrivals"], f"interstate margin arrivals {key}"),
            _integer(row["departures"], f"interstate margin departures {key}"),
        )
        _insert(interstate_margins, key, value, "interstate margins")
    _require_grid(
        "interstate margins", set(interstate_margins),
        {(year, state) for year in RUN_YEARS for state in STATES},
    )

    sources_path = pathlib.Path(sources_path)
    manifest = json.loads(sources_path.read_text(encoding="utf-8"))
    if manifest.get("format") != "sembla.abs-sources/v1":
        raise ValueError("sources.json format changed")
    sources = manifest.get("sources")
    if not isinstance(sources, dict):
        raise ValueError("sources.json has no source map")

    return Inputs(
        erp=erp,
        births=births,
        deaths=deaths,
        overseas=overseas,
        interstate_od=interstate_od,
        interstate_detail=interstate_detail,
        interstate_margins=interstate_margins,
        sources=sources,
        sources_sha256=hashlib.sha256(sources_path.read_bytes()).hexdigest(),
    )


def _source(inputs: Inputs, source_ids: str | tuple[str, ...], period: str) -> dict:
    ids = (source_ids,) if isinstance(source_ids, str) else source_ids
    records = [inputs.sources[source_id] for source_id in ids]
    release_dates = {record["release_date"] for record in records}
    releases = {record["release"] for record in records}
    if len(release_dates) != 1 or len(releases) != 1:
        raise ValueError(f"source releases do not align for {ids!r}")
    result = {
        "id": ids[0] if len(ids) == 1 else None,
        "ids": list(ids) if len(ids) != 1 else None,
        "release": records[0]["release"],
        "release_date": records[0]["release_date"],
        "reference_period": period,
    }
    return {key: value for key, value in result.items() if value is not None}


def _match_eq(key: str, value: str | int) -> dict:
    return {"key": key, "match": {"eq": value}}


def _match_any(key: str) -> dict:
    return {"key": key, "match": {"any": True}}


def _match_gte(key: str, value: int) -> dict:
    return {"key": key, "match": {"gte": value}}


def _match_exclude(key: str, values: Iterable[str]) -> dict:
    return {"key": key, "match": {"exclude": list(values)}}


def _band_match(key: str, index: int, terminal: int) -> dict:
    return _match_gte(key, index) if index == terminal else _match_eq(key, index)


def _count_value(value: int) -> dict:
    return {"kind": "count", "count": value}


def _ratio_value(numerator: int, denominator: int) -> dict:
    if denominator <= 0:
        raise ValueError("ratio denominator must be positive")
    return {"kind": "ratio", "numerator": numerator, "denominator": denominator}


def _count_discretisation(value: int) -> dict:
    remainder = value % SCALE_FACTOR
    return {
        "kind": "count_lattice",
        "scale_factor": SCALE_FACTOR,
        "lattice_quantum": SCALE_FACTOR,
        "minimum_attainable_absolute_error": min(
            remainder, (SCALE_FACTOR - remainder) % SCALE_FACTOR
        ),
    }


def _ratio_discretisation() -> dict:
    return {
        "kind": "denominator_dependent_ratio",
        "model_count_quantum": SCALE_FACTOR,
        "fixed_absolute_floor": None,
    }


def _count_observation(name: str, keys: list[dict]) -> dict:
    return {"name": name, "kind": "grouped_view", "keys": keys}


def _ratio_observation(name: str, numerator_keys: list[dict], denominator_keys: list[dict]) -> dict:
    return {
        "name": name,
        "kind": "grouped_view",
        "numerator_keys": numerator_keys,
        "denominator_keys": denominator_keys,
    }


def _role(variant: str, states: Iterable[str], structural_holdout: bool = False) -> tuple[str, str]:
    state_set = set(states)
    if structural_holdout:
        return "heldout", "single_year_age_structural_holdout"
    if variant == "spatial_holdout_nt" and "nt" in state_set:
        return "heldout", "spatial_transfer_nt"
    return "fitted", "annual_calibration_control"


def _target(
    *, target_id: str, family: str, observation: dict, time: dict,
    value: dict, source: dict, role: str, role_reason: str,
    aggregation: dict, discretisation: dict, projections: tuple[str, ...],
    state: str | None = None, source_rounding_resolution: int | None = None,
) -> dict:
    target = {
        "id": target_id,
        "family": family,
        "observation": observation,
        "time": time,
        "value": value,
        "source": source,
        "role": role,
        "role_reason": role_reason,
        "aggregation": aggregation,
        "discretisation": discretisation,
        "projection_membership": list(projections),
    }
    if state is not None:
        target["state"] = state
    if source_rounding_resolution is not None:
        target["source_rounding_resolution"] = source_rounding_resolution
    return target


def _death_source_band(label: str) -> str:
    if label == "100_plus":
        return "100+"
    left, right = label.split("_")
    return f"{int(left)}-{int(right)}"


def _direction_keys(
    state: str,
    direction: str,
    sex: str | None = None,
    age_index: int | None = None,
) -> list[dict]:
    keys = [
        _match_eq("prev_area", state) if direction == "departures" else _match_any("prev_area"),
        _match_eq("area", state) if direction == "arrivals" else _match_any("area"),
        _match_eq("sex", sex) if sex is not None else _match_any("sex"),
    ]
    if age_index is None:
        keys.append(_match_any("event_age_months"))
    else:
        keys.append(_band_match("event_age_months", age_index, 15))
    return keys


def _erp_sum(inputs: Inputs, year: int, states: Iterable[str], sexes: Iterable[str], ages: Iterable[int]) -> int:
    return sum(
        inputs.erp[(year, state, sex, age)]
        for state in states for sex in sexes for age in ages
    )


def build_artifact(
    year: int,
    model_path: pathlib.Path = DEFAULT_MODEL,
    inputs: Inputs | None = None,
    variant: str = "standard",
) -> dict:
    """Build and validate one annual target ledger."""
    if year not in RUN_YEARS:
        raise ValueError(f"unsupported run year {year}; expected 2010..2024")
    if variant not in {"standard", "spatial_holdout_nt"}:
        raise ValueError(f"unknown target variant {variant!r}")
    if variant == "spatial_holdout_nt" and year != 2010:
        raise ValueError("the spatial_holdout_nt variant is frozen to 2010")
    inputs = inputs or load_inputs()
    model_path = pathlib.Path(model_path)
    model_bytes = model_path.read_bytes()
    model = json.loads(model_bytes)
    end_year = year + 1
    targets: list[dict] = []
    projection_ids = {"full": [], "reduced": []}

    def add(target: dict) -> None:
        if any(previous["id"] == target["id"] for previous in targets):
            raise AssertionError(f"duplicate target ID {target['id']}")
        targets.append(target)
        if target["role"] == "fitted":
            for projection in target["projection_membership"]:
                projection_ids[projection].append(target["id"])

    stock_time = {"tick": 11, "boundary": f"{end_year}-06-30"}
    flow_time = {"period": {"start_tick": 0, "end_tick": 11}}

    # Raw full-projection five-year stock controls.
    for state in STATES:
        for sex in SEXES:
            for label, ages, band_index in MODEL_AGE_BANDS:
                value = _erp_sum(inputs, end_year, (state,), (sex,), ages)
                role, reason = _role(variant, (state,))
                add(_target(
                    target_id=f"stock.five_year.{state}.{sex}.{label}",
                    family="stock_five_year",
                    observation=_count_observation(
                        "population_cells",
                        [_match_eq("area", state), _match_eq("sex", sex),
                         _band_match("age_months", band_index, 20)],
                    ),
                    time=stock_time,
                    value=_count_value(value),
                    source=_source(inputs, f"erp_{state}", f"30 June {end_year}"),
                    role=role,
                    role_reason=reason,
                    aggregation={"operation": "last_count"},
                    discretisation=_count_discretisation(value),
                    projections=("full",),
                    state=state,
                ))

    # Structural single-year holdout, including an honest 100+ selector.
    for state in STATES:
        for sex in SEXES:
            for age in range(101):
                label = f"{age:03d}" if age < 100 else "100_plus"
                value = inputs.erp[(end_year, state, sex, age)]
                role, reason = _role(variant, (state,), structural_holdout=True)
                add(_target(
                    target_id=f"stock.single_year.{state}.{sex}.{label}",
                    family="stock_single_year",
                    observation=_count_observation(
                        "population_single_year_cells",
                        [_match_eq("area", state), _match_eq("sex", sex),
                         _band_match("age_months", age, 100)],
                    ),
                    time=stock_time,
                    value=_count_value(value),
                    source=_source(inputs, f"erp_{state}", f"30 June {end_year}"),
                    role=role,
                    role_reason=reason,
                    aggregation={"operation": "last_count"},
                    discretisation=_count_discretisation(value),
                    projections=(),
                    state=state,
                ))

    # Mandatory state birth controls.
    for state in STATES:
        value = inputs.births[(year, state)]
        role, reason = _role(variant, (state,))
        add(_target(
            target_id=f"flow.births.{state}", family="births_state",
            observation=_count_observation(
                "births_cells", [_match_eq("area", state), _match_any("sex")]
            ),
            time=flow_time, value=_count_value(value),
            source=_source(inputs, "births_state", f"calendar registration year {year}"),
            role=role, role_reason=reason,
            aggregation={"operation": "sum_count"},
            discretisation=_count_discretisation(value),
            projections=("full", "reduced"), state=state,
        ))

    # State by age death controls; sex is summed, not discarded upstream.
    for state in STATES:
        for label, _ages, band_index in MODEL_AGE_BANDS:
            source_band = _death_source_band(label)
            value = sum(inputs.deaths[(year, state, sex, source_band)] for sex in SEXES)
            role, reason = _role(variant, (state,))
            add(_target(
                target_id=f"flow.deaths.{state}.{label}",
                family="deaths_state_age",
                observation=_count_observation(
                    "deaths_state_age_cells",
                    [_match_eq("area", state), _match_any("sex"),
                     _band_match("event_age_months", band_index, 20)],
                ),
                time=flow_time, value=_count_value(value),
                source=_source(inputs, "deaths_state_age_sex", f"calendar registration year {year}"),
                role=role, role_reason=reason,
                aggregation={"operation": "sum_count"},
                discretisation=_count_discretisation(value),
                projections=("full",), state=state,
            ))

    # Overseas gross flows by state.
    for state in STATES:
        arrivals, departures = inputs.overseas[(year, state)]
        role, reason = _role(variant, (state,))
        for direction, value, observation, source_id in (
            ("arrivals", arrivals, "overseas_arrival_cells", "overseas_arrivals"),
            ("departures", departures, "overseas_departure_cells", "overseas_departures"),
        ):
            add(_target(
                target_id=f"flow.overseas_{direction}.{state}",
                family=f"overseas_{direction}_state",
                observation=_count_observation(
                    observation, [_match_eq("area", state), _match_any("sex")]
                ),
                time=flow_time, value=_count_value(value),
                source=_source(inputs, source_id, f"financial year {year}-{str(year + 1)[-2:]}"),
                role=role, role_reason=reason,
                aggregation={"operation": "sum_count"},
                discretisation=_count_discretisation(value),
                projections=("full", "reduced"), state=state,
            ))

    # All 56 O-D cells remain in both projections.
    for origin in STATES:
        for destination in STATES:
            if origin == destination:
                continue
            value = inputs.interstate_od[(year, origin, destination)]
            role, reason = _role(variant, (origin, destination))
            add(_target(
                target_id=f"flow.interstate_od.{origin}.{destination}",
                family="interstate_od",
                observation=_count_observation(
                    "interstate_flows",
                    [_match_eq("prev_area", origin), _match_eq("area", destination)],
                ),
                time=flow_time, value=_count_value(value),
                source=_source(inputs, "interstate_od", f"July {year} to June {year + 1}"),
                role=role, role_reason=reason,
                aggregation={"operation": "sum_count"},
                discretisation=_count_discretisation(value),
                projections=("full", "reduced"), state=origin,
            ))

    # Raw state/direction/sex/age compositions for the full projection.
    detail_denominators = {}
    for state in STATES:
        for direction, tuple_index in (("arrivals", 0), ("departures", 1)):
            denominator = sum(
                inputs.interstate_detail[(year, state, sex, source_band)][tuple_index]
                for sex in SEXES for _label, source_band, _index in INTERSTATE_AGE_BANDS
            )
            detail_denominators[(state, direction)] = denominator
            for sex in SEXES:
                for label, source_band, age_index in INTERSTATE_AGE_BANDS:
                    numerator = inputs.interstate_detail[
                        (year, state, sex, source_band)
                    ][tuple_index]
                    role, reason = _role(variant, (state,))
                    numerator_keys = _direction_keys(state, direction, sex, age_index)
                    denominator_keys = _direction_keys(state, direction)
                    add(_target(
                        target_id=(
                            f"flow.interstate_composition.{direction}.{state}.{sex}.{label}"
                        ),
                        family="interstate_age_sex_composition",
                        observation=_ratio_observation(
                            "interstate_age_sex_flows", numerator_keys, denominator_keys
                        ),
                        time=flow_time,
                        value=_ratio_value(numerator, denominator),
                        source=_source(
                            inputs, "interstate_age_sex",
                            f"financial year {year}-{str(year + 1)[-2:]}",
                        ),
                        role=role, role_reason=reason,
                        aggregation={"operation": "ratio_of_sums"},
                        discretisation=_ratio_discretisation(),
                        projections=("full",), state=state,
                        source_rounding_resolution=10,
                    ))

    # Reduced derived stock state totals.
    training_states = tuple(
        state for state in STATES if not (variant == "spatial_holdout_nt" and state == "nt")
    )
    for state in STATES:
        value = _erp_sum(inputs, end_year, (state,), SEXES, range(101))
        role, reason = _role(variant, (state,))
        add(_target(
            target_id=f"derived.stock_state_total.{state}",
            family="derived_stock_state_total",
            observation=_count_observation(
                "population_cells",
                [_match_eq("area", state), _match_any("sex"), _match_any("age_months")],
            ),
            time=stock_time, value=_count_value(value),
            source=_source(inputs, f"erp_{state}", f"30 June {end_year}"),
            role=role, role_reason=reason,
            aggregation={"operation": "last_count"},
            discretisation=_count_discretisation(value),
            projections=("reduced",), state=state,
        ))

    # Reduced training-geography national age profile.
    erp_ids = tuple(f"erp_{state}" for state in training_states)
    for label, ages, band_index in MODEL_AGE_BANDS:
        value = _erp_sum(inputs, end_year, training_states, SEXES, ages)
        area_selector = (
            _match_exclude("area", ("nt",))
            if variant == "spatial_holdout_nt" else _match_any("area")
        )
        add(_target(
            target_id=f"derived.stock_training_age.{label}",
            family="derived_stock_training_age",
            observation=_count_observation(
                "population_cells",
                [area_selector, _match_any("sex"),
                 _band_match("age_months", band_index, 20)],
            ),
            time=stock_time, value=_count_value(value),
            source=_source(inputs, erp_ids, f"30 June {end_year}"),
            role="fitted", role_reason="training_geography_aggregate",
            aggregation={"operation": "last_count"},
            discretisation=_count_discretisation(value),
            projections=("reduced",),
        ))

    # Reduced death state totals retain not-stated registrations honestly.
    for state in STATES:
        value = sum(
            inputs.deaths[(year, state, sex, band)]
            for sex in SEXES
            for band in (
                "0-4", "5-9", "10-14", "15-19", "20-24", "25-29",
                "30-34", "35-39", "40-44", "45-49", "50-54", "55-59",
                "60-64", "65-69", "70-74", "75-79", "80-84", "85-89",
                "90-94", "95-99", "100+", "not_stated",
            )
        )
        role, reason = _role(variant, (state,))
        add(_target(
            target_id=f"derived.deaths_state_total.{state}",
            family="derived_deaths_state_total",
            observation=_count_observation(
                "deaths_state_age_cells",
                [_match_eq("area", state), _match_any("sex"),
                 _match_any("event_age_months")],
            ),
            time=flow_time, value=_count_value(value),
            source=_source(inputs, "deaths_state_age_sex", f"calendar registration year {year}"),
            role=role, role_reason=reason,
            aggregation={"operation": "sum_count"},
            discretisation=_count_discretisation(value),
            projections=("reduced",), state=state,
        ))

    # Reduced female-share and first/second collapsed-age-ordinal moments.
    for state in STATES:
        for direction, tuple_index in (("arrivals", 0), ("departures", 1)):
            denominator = detail_denominators[(state, direction)]
            role, reason = _role(variant, (state,))
            female = sum(
                inputs.interstate_detail[(year, state, "female", source_band)][tuple_index]
                for _label, source_band, _index in INTERSTATE_AGE_BANDS
            )
            add(_target(
                target_id=f"derived.interstate_moment.{direction}.{state}.female_share",
                family="derived_interstate_composition_moment",
                observation=_ratio_observation(
                    "interstate_age_sex_flows",
                    _direction_keys(state, direction, "female"),
                    _direction_keys(state, direction),
                ),
                time=flow_time, value=_ratio_value(female, denominator),
                source=_source(inputs, "interstate_age_sex", f"financial year {year}-{str(year + 1)[-2:]}"),
                role=role, role_reason=reason,
                aggregation={"operation": "ratio_of_sums"},
                discretisation=_ratio_discretisation(),
                projections=("reduced",), state=state,
                source_rounding_resolution=10,
            ))
            for power, moment_name in ((1, "age_band_mean"), (2, "age_band_second_moment")):
                numerator = sum(
                    inputs.interstate_detail[(year, state, sex, source_band)][tuple_index]
                    * (age_index ** power)
                    for sex in SEXES
                    for _label, source_band, age_index in INTERSTATE_AGE_BANDS
                )
                add(_target(
                    target_id=f"derived.interstate_moment.{direction}.{state}.{moment_name}",
                    family="derived_interstate_composition_moment",
                    observation=_count_observation(
                        "interstate_age_sex_flows", _direction_keys(state, direction)
                    ),
                    time=flow_time, value=_ratio_value(numerator, denominator),
                    source=_source(inputs, "interstate_age_sex", f"financial year {year}-{str(year + 1)[-2:]}"),
                    role=role, role_reason=reason,
                    aggregation={
                        "operation": "weighted_ratio",
                        "weight_key": "event_age_months",
                        "cap": 15,
                        "power": power,
                    },
                    discretisation=_ratio_discretisation(),
                    projections=("reduced",), state=state,
                    source_rounding_resolution=10,
                ))

    od_arrivals = {state: 0 for state in STATES}
    od_departures = {state: 0 for state in STATES}
    for origin in STATES:
        for destination in STATES:
            if origin == destination:
                continue
            count = inputs.interstate_od[(year, origin, destination)]
            od_departures[origin] += count
            od_arrivals[destination] += count
    margin_rows = []
    maximum_margin_difference = 0
    for state in STATES:
        published_arrivals, published_departures = inputs.interstate_margins[(year, state)]
        arrival_error = od_arrivals[state] - published_arrivals
        departure_error = od_departures[state] - published_departures
        maximum_margin_difference = max(
            maximum_margin_difference, abs(arrival_error), abs(departure_error)
        )
        margin_rows.append({
            "state": state,
            "od_arrivals": od_arrivals[state],
            "published_arrivals": published_arrivals,
            "arrival_signed_difference": arrival_error,
            "od_departures": od_departures[state],
            "published_departures": published_departures,
            "departure_signed_difference": departure_error,
        })
    not_stated = {
        state: sum(inputs.deaths[(year, state, sex, "not_stated")] for sex in SEXES)
        for state in STATES
    }
    projection_sets = {name: set(ids) for name, ids in projection_ids.items()}
    for target in targets:
        target["projection_membership"] = [
            name for name in ("full", "reduced")
            if target["id"] in projection_sets[name]
        ]

    artifact = {
        "format": FORMAT,
        "model": {
            "name": "australian_population",
            "sha256": hashlib.sha256(model_bytes).hexdigest(),
            "required_feature": "grouped-observations",
        },
        "geography": {
            "version": GEOGRAPHY_VERSION,
            "attribute": "area",
            "variants": list(STATES),
        },
        "scale": {
            "name": SCALE_NAME,
            "factor": SCALE_FACTOR,
            "model_rows": 352_460,
        },
        "run_year": year,
        "stock_boundary_year": end_year,
        "covered_period": {
            "start": f"{year}-06-30",
            "end": f"{end_year}-06-30",
            "flow_ticks": [0, 11],
            "stock_tick": 11,
        },
        "variant": variant,
        "spatial_holdout": ["nt"] if variant == "spatial_holdout_nt" else [],
        "sources_sha256": inputs.sources_sha256,
        "source_alignment": {
            "births_and_deaths": "calendar_registration_year_proxy",
            "migration": "financial_year",
            "stocks": "30_june_end_boundary",
        },
        "projections": {
            name: {"target_ids": ids, "dimension": len(ids)}
            for name, ids in projection_ids.items()
        },
        "targets": targets,
        "diagnostics": {
            "death_not_stated_by_state": not_stated,
            "interstate_all_age_margin_comparison": margin_rows,
            "maximum_absolute_interstate_margin_difference": maximum_margin_difference,
            "material_2020_conflict": year == 2020,
            "raw_margins_are_fitted": False,
        },
    }
    expected_dimensions = (
        (1096, 165) if variant == "standard" else (952, 140)
    )
    actual_dimensions = (
        artifact["projections"]["full"]["dimension"],
        artifact["projections"]["reduced"]["dimension"],
    )
    if len(targets) != 2797 or actual_dimensions != expected_dimensions:
        raise AssertionError(
            f"target inventory changed: targets={len(targets)}, "
            f"dimensions={actual_dimensions}, expected={expected_dimensions}"
        )
    validate_artifact(artifact, model, model_bytes=model_bytes)
    return artifact


def _grouped_contract(model: dict) -> tuple[dict[str, tuple[str, ...]], dict[str, dict]]:
    boxes = model.get("boxes")
    if not isinstance(boxes, list) or len(boxes) != 1:
        raise ValueError("model must contain exactly one box")
    box = boxes[0]
    grouped = {}
    for view in box.get("grouped_views", []):
        grouped[view["name"]] = tuple(key["attr"] for key in view["keys"])
    tables = box.get("tables", [])
    people = next((table for table in tables if table.get("name") == "person_slot"), None)
    if people is None:
        raise ValueError("model has no person_slot table")
    attrs = {attribute["name"]: attribute["ty"] for attribute in people["attrs"]}
    return grouped, attrs


def _selector_sets(observation: dict) -> list[list[dict]]:
    if "keys" in observation:
        return [observation["keys"]]
    if "numerator_keys" in observation and "denominator_keys" in observation:
        return [observation["numerator_keys"], observation["denominator_keys"]]
    raise ValueError("grouped observation has no selector set")


def validate_artifact(artifact: dict, model: dict, model_bytes: bytes | None = None) -> None:
    """Strictly validate a target artifact before any field drives scoring."""
    top_keys = {
        "covered_period", "diagnostics", "format", "geography", "model",
        "projections", "run_year", "scale", "source_alignment", "sources_sha256",
        "spatial_holdout", "stock_boundary_year", "targets", "variant",
    }
    if set(artifact) != top_keys or artifact.get("format") != FORMAT:
        raise ValueError("target artifact schema or format changed")
    run_year = artifact.get("run_year")
    if type(run_year) is not int or run_year not in RUN_YEARS:
        raise ValueError("target run year is outside the supported ledger range")
    if artifact.get("stock_boundary_year") != run_year + 1:
        raise ValueError("target stock boundary is not the following year")
    variant = artifact.get("variant")
    expected_holdout = [] if variant == "standard" else ["nt"]
    if variant not in {"standard", "spatial_holdout_nt"}:
        raise ValueError("unknown target variant")
    if artifact.get("spatial_holdout") != expected_holdout:
        raise ValueError("target spatial holdout does not match its variant")
    expected_period = {
        "start": f"{run_year}-06-30",
        "end": f"{run_year + 1}-06-30",
        "flow_ticks": [0, 11],
        "stock_tick": 11,
    }
    if artifact.get("covered_period") != expected_period:
        raise ValueError("target covered period changed")
    if artifact.get("source_alignment") != {
        "stocks": "30_june_end_boundary",
        "births_and_deaths": "calendar_registration_year_proxy",
        "migration": "financial_year",
    }:
        raise ValueError("target source-period alignment changed")
    if artifact.get("geography") != {
        "version": GEOGRAPHY_VERSION,
        "attribute": "area",
        "variants": list(STATES),
    }:
        raise ValueError("target geography contract changed")
    source_hash = artifact.get("sources_sha256")
    if not isinstance(source_hash, str) or len(source_hash) != 64 or any(
        character not in "0123456789abcdef" for character in source_hash
    ):
        raise ValueError("target sources hash is malformed")
    sources_bytes = SOURCES.read_bytes()
    manifest_payload = json.loads(sources_bytes)
    if manifest_payload.get("format") != "sembla.abs-sources/v1":
        raise ValueError("sources.json format changed")
    source_manifest = manifest_payload.get("sources")
    if not isinstance(source_manifest, dict):
        raise ValueError("sources.json has no source map")
    if source_hash != hashlib.sha256(sources_bytes).hexdigest():
        raise ValueError("target sources hash does not match sources.json")

    model_header = artifact.get("model")
    if not isinstance(model_header, dict) or set(model_header) != {
        "name", "required_feature", "sha256"
    }:
        raise ValueError("target model header changed")
    if model_header.get("name") != model.get("name"):
        raise ValueError("target/model name mismatch")
    if model_header.get("required_feature") != "grouped-observations":
        raise ValueError("target required feature changed")
    if model_bytes is not None:
        expected = hashlib.sha256(model_bytes).hexdigest()
        if model_header.get("sha256") != expected:
            raise ValueError("target/model byte digest mismatch")

    grouped, attrs = _grouped_contract(model)
    for name, expected_keys in REQUIRED_GROUPED_VIEWS.items():
        if grouped.get(name) != expected_keys:
            raise ValueError(
                f"model grouped view {name!r} changed: {grouped.get(name)!r}; "
                f"expected {expected_keys!r}"
            )
    area_variants = attrs["area"].get("variants")
    if area_variants != list(STATES):
        raise ValueError("model area enum changed")
    people = next(
        table for table in model["boxes"][0]["tables"]
        if table["name"] == "person_slot"
    )
    if artifact.get("scale") != {
        "name": SCALE_NAME,
        "factor": SCALE_FACTOR,
        "model_rows": people["size_hint"],
    }:
        raise ValueError("target scale contract changed")
    if not isinstance(artifact.get("diagnostics"), dict):
        raise ValueError("target diagnostics must be an object")

    ledger_targets = artifact.get("targets")
    if not isinstance(ledger_targets, list):
        raise ValueError("target artifact has no ordered target list")
    roles = {}
    membership = {}
    for target in ledger_targets:
        if not isinstance(target, dict):
            raise ValueError("target entry is not an object")
        target_id = target.get("id")
        if not isinstance(target_id, str) or not target_id or target_id in roles:
            raise ValueError(f"duplicate or invalid target ID {target_id!r}")
        family = target.get("family")
        if not isinstance(family, str) or not family:
            raise ValueError(f"target {target_id!r} has invalid family")
        role = target.get("role")
        if role not in {"fitted", "heldout"}:
            raise ValueError(f"target {target_id!r} has invalid role {role!r}")
        if not isinstance(target.get("role_reason"), str) or not target["role_reason"]:
            raise ValueError(f"target {target_id!r} has no role reason")
        roles[target_id] = role
        projected = target.get("projection_membership")
        if not isinstance(projected, list) or len(projected) != len(set(projected)):
            raise ValueError(f"target {target_id!r} has invalid projection membership")
        if any(name not in {"full", "reduced"} for name in projected):
            raise ValueError(f"target {target_id!r} names an unknown projection")
        membership[target_id] = projected
        state = target.get("state")
        if state is not None and state not in STATES:
            raise ValueError(f"target {target_id!r} has unknown state {state!r}")

        source = target.get("source")
        if not isinstance(source, dict):
            raise ValueError(f"target {target_id!r} has invalid source")
        single_source_families = {
            "births_state": "births_state",
            "deaths_state_age": "deaths_state_age_sex",
            "derived_deaths_state_total": "deaths_state_age_sex",
            "overseas_arrivals_state": "overseas_arrivals",
            "overseas_departures_state": "overseas_departures",
            "interstate_od": "interstate_od",
            "interstate_age_sex_composition": "interstate_age_sex",
            "derived_interstate_composition_moment": "interstate_age_sex",
        }
        if family in {"stock_five_year", "stock_single_year", "derived_stock_state_total"}:
            if state is None:
                raise ValueError(f"target {target_id!r} stock source has no state")
            expected_source_ids = (f"erp_{state}",)
            expected_reference_period = f"30 June {run_year + 1}"
        elif family == "derived_stock_training_age":
            expected_source_ids = tuple(
                f"erp_{area}" for area in STATES if area not in expected_holdout
            )
            expected_reference_period = f"30 June {run_year + 1}"
        elif family in single_source_families:
            expected_source_ids = (single_source_families[family],)
            if family in {"births_state", "deaths_state_age", "derived_deaths_state_total"}:
                expected_reference_period = f"calendar registration year {run_year}"
            elif family == "interstate_od":
                expected_reference_period = f"July {run_year} to June {run_year + 1}"
            else:
                expected_reference_period = (
                    f"financial year {run_year}-{str(run_year + 1)[-2:]}"
                )
        else:
            raise ValueError(f"target {target_id!r} has unknown family {family!r}")

        expected_source_keys = {
            "release", "release_date", "reference_period",
            "id" if len(expected_source_ids) == 1 else "ids",
        }
        if set(source) != expected_source_keys:
            raise ValueError(f"target {target_id!r} source schema changed")
        actual_source_ids = (
            (source.get("id"),)
            if len(expected_source_ids) == 1
            else tuple(source.get("ids", []))
        )
        if actual_source_ids != expected_source_ids:
            raise ValueError(
                f"target {target_id!r} source IDs {actual_source_ids!r} do not "
                f"match {expected_source_ids!r}"
            )
        try:
            source_records = [source_manifest[source_id] for source_id in expected_source_ids]
        except KeyError as error:
            raise ValueError(f"target {target_id!r} source is absent from sources.json") from error
        for field in ("release", "release_date"):
            expected_values = {record.get(field) for record in source_records}
            if len(expected_values) != 1 or source.get(field) != next(iter(expected_values)):
                raise ValueError(
                    f"target {target_id!r} source {field} does not match sources.json"
                )
        if source.get("reference_period") != expected_reference_period:
            raise ValueError(f"target {target_id!r} source reference period changed")
        expected_resolution = (
            10 if family in {
                "interstate_age_sex_composition",
                "derived_interstate_composition_moment",
            } else None
        )
        if expected_resolution is None:
            if "source_rounding_resolution" in target:
                raise ValueError(f"target {target_id!r} has invalid source resolution")
        elif target.get("source_rounding_resolution") != expected_resolution:
            raise ValueError(f"target {target_id!r} has invalid source resolution")

        observation = target.get("observation", {})
        name = observation.get("name")
        if observation.get("kind") != "grouped_view" or name not in grouped:
            raise ValueError(f"target {target_id!r} names unknown observation {name!r}")
        expected_keys = grouped[name]
        for selectors in _selector_sets(observation):
            if not isinstance(selectors, list):
                raise ValueError(f"target {target_id!r} selectors are not ordered")
            actual_keys = tuple(selector.get("key") for selector in selectors)
            if actual_keys != expected_keys:
                raise ValueError(
                    f"target {target_id!r} selector keys {actual_keys!r} do not "
                    f"match {name!r} keys {expected_keys!r}"
                )
            for selector in selectors:
                key = selector["key"]
                match = selector.get("match", {})
                if not isinstance(match, dict) or len(match) != 1:
                    raise ValueError(f"target {target_id!r} has ambiguous selector")
                operation, value = next(iter(match.items()))
                attr_type = attrs[key]
                if operation == "eq" and attr_type.get("kind") == "enum":
                    if value not in attr_type["variants"]:
                        raise ValueError(
                            f"target {target_id!r} uses unknown {key} variant {value!r}"
                        )
                elif operation == "exclude" and attr_type.get("kind") == "enum":
                    if not isinstance(value, list) or not value or len(value) != len(set(value)) or any(
                        variant not in attr_type["variants"] for variant in value
                    ):
                        raise ValueError(f"target {target_id!r} has invalid exclusion")
                elif operation == "any" and value is True:
                    pass
                elif operation in {"eq", "gte"} and attr_type.get("kind") == "int":
                    if type(value) is not int or value < 0:
                        raise ValueError(f"target {target_id!r} has invalid band selector")
                else:
                    raise ValueError(f"target {target_id!r} has invalid selector {selector!r}")

        aggregation = target.get("aggregation")
        if not isinstance(aggregation, dict):
            raise ValueError(f"target {target_id!r} has invalid aggregation")
        operation = aggregation.get("operation")
        value = target.get("value")
        time = target.get("time")
        if operation == "last_count":
            if set(aggregation) != {"operation"} or time != {
                "tick": 11, "boundary": f"{run_year + 1}-06-30"
            }:
                raise ValueError(f"target {target_id!r} has invalid stock timing")
            expected_kind = "count"
        elif operation == "sum_count":
            if set(aggregation) != {"operation"} or time != {
                "period": {"start_tick": 0, "end_tick": 11}
            }:
                raise ValueError(f"target {target_id!r} has invalid flow timing")
            expected_kind = "count"
        elif operation == "ratio_of_sums":
            if set(aggregation) != {"operation"} or time != {
                "period": {"start_tick": 0, "end_tick": 11}
            }:
                raise ValueError(f"target {target_id!r} has invalid ratio timing")
            expected_kind = "ratio"
        elif operation == "weighted_ratio":
            if set(aggregation) != {"operation", "weight_key", "cap", "power"}:
                raise ValueError(f"target {target_id!r} has invalid weighted aggregation")
            if aggregation["weight_key"] not in expected_keys:
                raise ValueError(f"target {target_id!r} weights an unknown key")
            if type(aggregation["cap"]) is not int or aggregation["cap"] < 0:
                raise ValueError(f"target {target_id!r} has invalid weight cap")
            if aggregation["power"] not in {1, 2} or time != {
                "period": {"start_tick": 0, "end_tick": 11}
            }:
                raise ValueError(f"target {target_id!r} has invalid weighted timing")
            expected_kind = "ratio"
        else:
            raise ValueError(f"target {target_id!r} has unknown aggregation {operation!r}")

        expected_observation_keys = (
            {"kind", "name", "numerator_keys", "denominator_keys"}
            if operation == "ratio_of_sums"
            else {"kind", "name", "keys"}
        )
        if set(observation) != expected_observation_keys:
            raise ValueError(f"target {target_id!r} observation shape is inconsistent")
        if not isinstance(value, dict) or value.get("kind") != expected_kind:
            raise ValueError(f"target {target_id!r} value/aggregation mismatch")
        discretisation = target.get("discretisation")
        if expected_kind == "count":
            count = value.get("count")
            if set(value) != {"kind", "count"} or type(count) is not int or count < 0:
                raise ValueError(f"target {target_id!r} has invalid count")
            remainder = count % SCALE_FACTOR
            expected_floor = min(remainder, (SCALE_FACTOR - remainder) % SCALE_FACTOR)
            if discretisation != {
                "kind": "count_lattice",
                "scale_factor": SCALE_FACTOR,
                "lattice_quantum": SCALE_FACTOR,
                "minimum_attainable_absolute_error": expected_floor,
            }:
                raise ValueError(f"target {target_id!r} has invalid count discretisation")
        else:
            numerator = value.get("numerator")
            denominator = value.get("denominator")
            if set(value) != {"kind", "numerator", "denominator"} or type(numerator) is not int or numerator < 0 or type(denominator) is not int or denominator <= 0:
                raise ValueError(f"target {target_id!r} has invalid ratio")
            if discretisation != {
                "kind": "denominator_dependent_ratio",
                "model_count_quantum": SCALE_FACTOR,
                "fixed_absolute_floor": None,
            }:
                raise ValueError(f"target {target_id!r} has invalid ratio discretisation")

    projections = artifact.get("projections")
    if not isinstance(projections, dict) or set(projections) != {"full", "reduced"}:
        raise ValueError("target projections changed")
    projection_sets = {}
    for projection_name in ("full", "reduced"):
        projection = projections[projection_name]
        if not isinstance(projection, dict) or set(projection) != {"target_ids", "dimension"}:
            raise ValueError(f"projection {projection_name!r} schema changed")
        projection_ids = projection.get("target_ids")
        if not isinstance(projection_ids, list) or len(projection_ids) != projection.get("dimension"):
            raise ValueError(f"projection {projection_name!r} dimension mismatch")
        if len(projection_ids) != len(set(projection_ids)):
            raise ValueError(f"projection {projection_name!r} repeats target IDs")
        for target_id in projection_ids:
            if target_id not in roles:
                raise ValueError(f"projection {projection_name!r} names unknown target {target_id!r}")
            if roles[target_id] != "fitted":
                raise ValueError(f"projection {projection_name!r} includes heldout {target_id!r}")
        projection_sets[projection_name] = set(projection_ids)
    for target_id, role in roles.items():
        expected_membership = [
            name for name in ("full", "reduced")
            if target_id in projection_sets[name]
        ]
        if membership[target_id] != expected_membership:
            raise ValueError(f"target {target_id!r} projection membership is inconsistent")
        if (role == "fitted") != bool(expected_membership):
            raise ValueError(f"target {target_id!r} role has no consistent projection")


def target_hash(content: bytes) -> str:
    return hashlib.sha256(HASH_DOMAIN + content).hexdigest()


def _artifact_filename(year: int, variant: str) -> str:
    return f"{year}.json" if variant == "standard" else f"{year}.{variant}.json"


def execution_contract(
    model_path: pathlib.Path = DEFAULT_MODEL,
    plan_path: pathlib.Path = DEFAULT_PLAN,
) -> dict:
    """Return the reviewed public-run identity expected by the scorer."""
    model_path = pathlib.Path(model_path)
    plan_path = pathlib.Path(plan_path)
    model_bytes = model_path.read_bytes()
    plan_bytes = plan_path.read_bytes()
    model_raw = hashlib.sha256(model_bytes).hexdigest()
    plan_raw = hashlib.sha256(plan_bytes).hexdigest()
    if model_raw != EXECUTION_MODEL_RAW_SHA256:
        raise ValueError("execution-contract model bytes changed; review run identity")
    if plan_raw != EXECUTION_PLAN_RAW_SHA256:
        raise ValueError("execution-contract plan bytes changed; review run identity")
    model = json.loads(model_bytes)
    plan = json.loads(plan_bytes)
    if plan.get("model", {}).get("name") != model.get("name"):
        raise ValueError("execution-contract plan/model name changed")
    enabled = plan.get("identity", {}).get("enabled_features")
    if enabled != ["grouped-observations"]:
        raise ValueError("execution-contract plan features changed")
    return {
        "format": EXECUTION_FORMAT,
        "model": {
            "name": model.get("name"),
            "raw_sha256": model_raw,
            "ir_hash": {
                "algorithm": "sha256",
                "digest": EXECUTION_IR_SHA256,
            },
        },
        "plan": {
            "raw_sha256": plan_raw,
            "schema": plan.get("schema_version"),
            "identity_scheme": plan.get("identity_scheme"),
            "origin": plan.get("origin"),
            "enabled_features": enabled,
            "semantic_hash": {
                "algorithm": "sha256",
                "domain": "sembla.plan-core/v1",
                "digest": EXECUTION_PLAN_SEMANTIC_SHA256,
            },
        },
        "provenance": (
            "Observed from the public run manifest for the exact model/plan bytes; "
            "cross-checked by crates/sembla-cli/tests/australian_population.rs"
        ),
    }


def write_all(
    out_dir: pathlib.Path = TARGETS,
    model_path: pathlib.Path = DEFAULT_MODEL,
    inputs: Inputs | None = None,
    plan_path: pathlib.Path = DEFAULT_PLAN,
) -> dict:
    """Write all ledgers, execution identity, and their canonical index."""
    out_dir = pathlib.Path(out_dir)
    model_path = pathlib.Path(model_path)
    inputs = inputs or load_inputs()
    entries = []
    requested = [(year, "standard") for year in RUN_YEARS] + [(2010, "spatial_holdout_nt")]
    for year, variant in requested:
        artifact = build_artifact(year, model_path, inputs, variant)
        filename = _artifact_filename(year, variant)
        path = out_dir / filename
        canonical.write_json(path, artifact)
        content = path.read_bytes()
        role_counts = {
            role: sum(target["role"] == role for target in artifact["targets"])
            for role in ("fitted", "heldout")
        }
        entries.append({
            "path": filename,
            "run_year": year,
            "variant": variant,
            "bytes": len(content),
            "raw_sha256": hashlib.sha256(content).hexdigest(),
            "target_sha256": target_hash(content),
            "target_count": len(artifact["targets"]),
            "role_counts": role_counts,
            "projection_dimensions": {
                name: projection["dimension"]
                for name, projection in artifact["projections"].items()
            },
        })
    contract_path = out_dir / "execution.json"
    canonical.write_json(contract_path, execution_contract(model_path, plan_path))
    contract_bytes = contract_path.read_bytes()
    index = {
        "format": INDEX_FORMAT,
        "target_format": FORMAT,
        "hash_domain": "sembla.targets/v1\\0",
        "model_sha256": hashlib.sha256(model_path.read_bytes()).hexdigest(),
        "sources_sha256": inputs.sources_sha256,
        "execution_contract": {
            "path": contract_path.name,
            "bytes": len(contract_bytes),
            "raw_sha256": hashlib.sha256(contract_bytes).hexdigest(),
        },
        "entries": entries,
    }
    canonical.write_json(out_dir / "index.json", index)
    return index


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=pathlib.Path, default=TARGETS)
    parser.add_argument("--model", type=pathlib.Path, default=DEFAULT_MODEL)
    parser.add_argument("--plan", type=pathlib.Path, default=DEFAULT_PLAN)
    args = parser.parse_args(argv)
    index = write_all(args.out, args.model, plan_path=args.plan)
    print(
        f"wrote {len(index['entries'])} target ledgers and index to {args.out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

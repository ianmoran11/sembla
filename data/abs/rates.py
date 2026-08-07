#!/usr/bin/env python3
"""Derive annual Australian-population parameters from committed ABS extracts.

This module is deliberately offline and standard-library only.  It converts
published rates and flows into the model's monthly hazards, writes complete
2010--2024 parameter maps, records the fixed/free contract, and renders the
human-readable rate evidence report.
"""

from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from decimal import Decimal, localcontext
import hashlib
import json
import math
import pathlib
from typing import Iterable

import build_state
import canonical


HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"
PARAMS = HERE / "params"
REPORT = EXTRACTS / "rates.md"
FIDELITY_EVIDENCE = PARAMS / "fidelity-2010.json"
FIDELITY_PREDECLARATION = PARAMS / "fidelity-2010-predeclaration.json"
FIDELITY_PREDECLARATION_SHA256 = (
    "87f442ea1b97b90f718e0bf4205497aad03c2d24de0aa7470a09e160f391e18b"
)
SOURCES = HERE / "sources.json"

RUN_YEARS = tuple(range(2010, 2025))
ERP_YEARS = tuple(range(2010, 2026))
MODEL_STATES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")
SEXES = ("male", "female")
AGE_BANDS = (
    ("0-4", "00_04"),
    ("5-9", "05_09"),
    ("10-14", "10_14"),
    ("15-19", "15_19"),
    ("20-24", "20_24"),
    ("25-29", "25_29"),
    ("30-34", "30_34"),
    ("35-39", "35_39"),
    ("40-44", "40_44"),
    ("45-49", "45_49"),
    ("50-54", "50_54"),
    ("55-59", "55_59"),
    ("60-64", "60_64"),
    ("65-69", "65_69"),
    ("70-74", "70_74"),
    ("75-79", "75_79"),
    ("80-84", "80_84"),
    ("85-89", "85_89"),
    ("90-94", "90_94"),
    ("95-99", "95_99"),
    ("100+", "100_plus"),
)
SOURCE_AGE_BANDS = tuple(source for source, _model in AGE_BANDS)
MODEL_AGE_BANDS = tuple(model for _source, model in AGE_BANDS)
AGE_BAND_TO_MODEL = dict(AGE_BANDS)
DEATH_AGE_BANDS = SOURCE_AGE_BANDS + ("not_stated",)

MORTALITY_DIVISOR = Decimal(12_000)
DIRECT_LOG_NORMAL_SPREAD = 0.5
ZERO_MORTALITY_NORMAL_SPREAD = float(Decimal("0.05") / MORTALITY_DIVISOR)
FALLBACK_CELLS = frozenset(
    {
        (2010, "nt", "female", "100+"),
        (2011, "nt", "female", "100+"),
    }
)

FREE_DEFAULTS = {
    "interstate_base": 0.0001,
    "push_vic": 1.0,
    "pull_vic": 1.0,
    "push_qld": 1.0,
    "pull_qld": 1.0,
    "push_sa": 1.0,
    "pull_sa": 1.0,
    "push_wa": 1.0,
    "pull_wa": 1.0,
    "push_tas": 1.0,
    "pull_tas": 1.0,
    "push_nt": 1.0,
    "pull_nt": 1.0,
    "push_act": 1.0,
    "pull_act": 1.0,
    "peak_months": 360.0,
    "k": 0.00001,
}

EXPECTED_HEADERS = {
    "births_state.csv": ["year", "state", "births"],
    "births_sex.csv": ["year", "sex", "births"],
    "mortality_rates_state_age_sex.csv": [
        "year", "state", "sex", "age_band", "rate_per_1000", "status"
    ],
    "mortality_rates_national_age_sex.csv": [
        "year", "sex", "age_band", "rate_per_1000"
    ],
    "deaths_state_age_sex.csv": [
        "year", "state", "sex", "age_band", "deaths"
    ],
    "overseas_margins.csv": ["run_year", "state", "arrivals", "departures"],
    "erp_state_age_sex.csv": ["year", "state", "sex", "age", "persons"],
    "components_national.csv": [
        "run_year", "births", "deaths", "overseas_arrivals",
        "overseas_departures"
    ],
    "life_tables_state_age_sex.csv": [
        "period_start", "period_end", "state", "sex", "age", "qx"
    ],
}


@dataclass(frozen=True)
class Inputs:
    births: dict[tuple[int, str], int]
    national_births: dict[int, int]
    mortality: dict[tuple[int, str, str, str], Decimal | None]
    mortality_status: dict[tuple[int, str, str, str], str]
    national_mortality: dict[tuple[int, str, str], Decimal]
    deaths: dict[tuple[int, str, str, str], int]
    overseas: dict[tuple[int, str], tuple[int, int]]
    erp: dict[tuple[int, str], int]
    components: dict[int, dict[str, int]]
    life_tables: tuple[tuple[int, int, str, str, int, Decimal], ...]
    sources: dict[str, dict[str, object]]


@dataclass(frozen=True)
class EntryCalculation:
    stream: str
    year: int
    state: str
    start_vacancies: int
    target_entries: int
    end_vacancies: int
    average_expected_vacancies: float
    monthly_probability: float
    monthly_hazard: float


@dataclass(frozen=True)
class LifeTableMetric:
    period_start: int
    period_end: int
    cells: int
    mean_signed_error: float
    mean_absolute_error: float
    root_mean_square_error: float
    maximum_absolute_error: float


@dataclass(frozen=True)
class RateArtifacts:
    inputs: Inputs
    annual_params: dict[int, dict[str, float]]
    entry_calculations: tuple[EntryCalculation, ...]
    fallback_provenance: tuple[dict[str, object], ...]
    life_table_metrics: tuple[LifeTableMetric, ...]
    birth_capacities: dict[str, int]
    overseas_capacities: dict[str, int]


def _rows(extracts: pathlib.Path, name: str) -> list[dict[str, str]]:
    path = extracts / name
    with path.open(newline="", encoding="utf-8") as source:
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
        raise ValueError(f"{label} must be non-negative, got {value}")
    return value


def _decimal(text: str, label: str) -> Decimal:
    try:
        value = Decimal(text)
    except Exception as error:
        raise ValueError(f"{label} is not a decimal: {text!r}") from error
    if not value.is_finite() or value < 0:
        raise ValueError(f"{label} must be finite and non-negative, got {text!r}")
    return value


def _insert_unique(mapping: dict, key, value, label: str) -> None:
    if key in mapping:
        raise ValueError(f"{label} contains duplicate cell {key!r}")
    mapping[key] = value


def _require_grid(label: str, actual: set, expected: set) -> None:
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        raise ValueError(
            f"{label} grid changed: missing={missing[:10]!r}, "
            f"extra={extra[:10]!r}"
        )


def load_inputs(extracts: pathlib.Path = EXTRACTS, sources_path: pathlib.Path = SOURCES) -> Inputs:
    """Load and validate the complete fixed input grids."""
    extracts = pathlib.Path(extracts)

    births: dict[tuple[int, str], int] = {}
    for row in _rows(extracts, "births_state.csv"):
        key = (int(row["year"]), row["state"])
        _insert_unique(births, key, _integer(row["births"], f"births {key}"), "births")
    _require_grid(
        "births",
        set(births),
        {(year, state) for year in RUN_YEARS for state in MODEL_STATES},
    )

    births_by_sex: dict[tuple[int, str], int] = {}
    for row in _rows(extracts, "births_sex.csv"):
        key = (int(row["year"]), row["sex"])
        _insert_unique(
            births_by_sex, key, _integer(row["births"], f"births_sex {key}"),
            "births_sex",
        )
    _require_grid(
        "births_sex",
        set(births_by_sex),
        {(year, sex) for year in RUN_YEARS for sex in SEXES},
    )
    national_births = {
        year: sum(births_by_sex[(year, sex)] for sex in SEXES)
        for year in RUN_YEARS
    }

    mortality: dict[tuple[int, str, str, str], Decimal | None] = {}
    mortality_status: dict[tuple[int, str, str, str], str] = {}
    for row in _rows(extracts, "mortality_rates_state_age_sex.csv"):
        key = (int(row["year"]), row["state"], row["sex"], row["age_band"])
        if key in mortality:
            raise ValueError(f"mortality contains duplicate cell {key!r}")
        status = row["status"]
        value_text = row["rate_per_1000"]
        if status == "published":
            if not value_text:
                raise ValueError(f"published mortality cell {key!r} is blank")
            value = _decimal(value_text, f"mortality {key}")
        elif status == "not_published_zero_exposure":
            if key not in FALLBACK_CELLS:
                raise ValueError(f"unexpected mortality fallback cell {key!r}")
            if value_text:
                raise ValueError(f"fallback mortality cell {key!r} must be blank")
            value = None
        else:
            raise ValueError(f"mortality cell {key!r} has unknown status {status!r}")
        mortality[key] = value
        mortality_status[key] = status
    expected_mortality = {
        (year, state, sex, age_band)
        for year in RUN_YEARS
        for state in MODEL_STATES
        for sex in SEXES
        for age_band in SOURCE_AGE_BANDS
    }
    _require_grid("mortality", set(mortality), expected_mortality)
    actual_fallbacks = {key for key, value in mortality.items() if value is None}
    if actual_fallbacks != set(FALLBACK_CELLS):
        raise ValueError(
            f"mortality fallbacks changed: {sorted(actual_fallbacks)!r}; "
            f"expected {sorted(FALLBACK_CELLS)!r}"
        )

    national_mortality: dict[tuple[int, str, str], Decimal] = {}
    for row in _rows(extracts, "mortality_rates_national_age_sex.csv"):
        key = (int(row["year"]), row["sex"], row["age_band"])
        _insert_unique(
            national_mortality,
            key,
            _decimal(row["rate_per_1000"], f"national mortality {key}"),
            "national mortality",
        )
    _require_grid(
        "national mortality",
        set(national_mortality),
        {
            (year, sex, age_band)
            for year in RUN_YEARS
            for sex in SEXES
            for age_band in SOURCE_AGE_BANDS
        },
    )

    deaths: dict[tuple[int, str, str, str], int] = {}
    for row in _rows(extracts, "deaths_state_age_sex.csv"):
        key = (int(row["year"]), row["state"], row["sex"], row["age_band"])
        _insert_unique(
            deaths, key, _integer(row["deaths"], f"deaths {key}"), "deaths"
        )
    _require_grid(
        "deaths",
        set(deaths),
        {
            (year, state, sex, age_band)
            for year in RUN_YEARS
            for state in MODEL_STATES
            for sex in SEXES
            for age_band in DEATH_AGE_BANDS
        },
    )

    overseas: dict[tuple[int, str], tuple[int, int]] = {}
    for row in _rows(extracts, "overseas_margins.csv"):
        key = (int(row["run_year"]), row["state"])
        value = (
            _integer(row["arrivals"], f"overseas arrivals {key}"),
            _integer(row["departures"], f"overseas departures {key}"),
        )
        _insert_unique(overseas, key, value, "overseas margins")
    _require_grid(
        "overseas margins",
        set(overseas),
        {(year, state) for year in RUN_YEARS for state in MODEL_STATES},
    )

    erp_cells: dict[tuple[int, str, str, int], int] = {}
    for row in _rows(extracts, "erp_state_age_sex.csv"):
        key = (int(row["year"]), row["state"], row["sex"], int(row["age"]))
        _insert_unique(
            erp_cells, key, _integer(row["persons"], f"ERP {key}"), "ERP"
        )
    _require_grid(
        "ERP",
        set(erp_cells),
        {
            (year, state, sex, age)
            for year in ERP_YEARS
            for state in MODEL_STATES
            for sex in SEXES
            for age in range(101)
        },
    )
    erp = {
        (year, state): sum(
            erp_cells[(year, state, sex, age)]
            for sex in SEXES
            for age in range(101)
        )
        for year in ERP_YEARS
        for state in MODEL_STATES
    }

    components: dict[int, dict[str, int]] = {}
    for row in _rows(extracts, "components_national.csv"):
        year = int(row["run_year"])
        value = {
            name: _integer(row[name], f"component {year} {name}")
            for name in (
                "births", "deaths", "overseas_arrivals", "overseas_departures"
            )
        }
        _insert_unique(components, year, value, "national components")
    _require_grid("national components", set(components), set(RUN_YEARS))

    life_tables: list[tuple[int, int, str, str, int, Decimal]] = []
    life_keys = set()
    for row in _rows(extracts, "life_tables_state_age_sex.csv"):
        key = (
            int(row["period_start"]), int(row["period_end"]), row["state"],
            row["sex"], int(row["age"]),
        )
        if key in life_keys:
            raise ValueError(f"life tables contain duplicate cell {key!r}")
        life_keys.add(key)
        life_tables.append((*key, _decimal(row["qx"], f"life-table qx {key}")))
    periods = tuple((year, year + 2) for year in range(2018, 2023))
    _require_grid(
        "life tables",
        life_keys,
        {
            (start, end, state, sex, age)
            for start, end in periods
            for state in MODEL_STATES
            for sex in SEXES
            for age in range(101)
        },
    )

    manifest = json.loads(pathlib.Path(sources_path).read_text(encoding="utf-8"))
    if manifest.get("format") != "sembla.abs-sources/v1":
        raise ValueError("sources.json format changed")
    sources = manifest.get("sources")
    if not isinstance(sources, dict):
        raise ValueError("sources.json has no source map")

    return Inputs(
        births=births,
        national_births=national_births,
        mortality=mortality,
        mortality_status=mortality_status,
        national_mortality=national_mortality,
        deaths=deaths,
        overseas=overseas,
        erp=erp,
        components=components,
        life_tables=tuple(sorted(life_tables)),
        sources=sources,
    )


def expected_parameter_names() -> set[str]:
    names = set(FREE_DEFAULTS)
    names.update(f"birth_rate_{state}" for state in MODEL_STATES)
    names.update(
        f"mortality_{state}_{model_band}_{sex}"
        for state in MODEL_STATES
        for model_band in MODEL_AGE_BANDS
        for sex in SEXES
    )
    names.update(f"overseas_arrival_{state}" for state in MODEL_STATES)
    names.update(f"emigration_{state}" for state in MODEL_STATES)
    return names


def entry_calculation(
    stream: str, year: int, state: str, start_vacancies: int, entries: int
) -> EntryCalculation:
    """Return the exact monthly hazard for an expected twelve-tick flow.

    A slot survives a month with probability ``exp(-h)``.  Choosing
    ``h = -log(1 - E/V) / 12`` therefore leaves ``V-E`` expected vacancies
    after twelve ticks.  The reported average exposure is the arithmetic mean
    of the twelve expected start-of-month vacancy counts.
    """
    if start_vacancies <= 0:
        raise ValueError(f"{stream} {year} {state} has no vacant slots")
    if entries < 0:
        raise ValueError(f"{stream} {year} {state} has negative entries")
    if entries >= start_vacancies:
        raise ValueError(
            f"{stream} {year} {state} target {entries} exhausts "
            f"{start_vacancies} vacant slots"
        )

    with localcontext() as context:
        context.prec = 50
        start = Decimal(start_vacancies)
        flow = Decimal(entries)
        if entries == 0:
            hazard = Decimal(0)
            monthly_probability = Decimal(0)
            average = start
        else:
            annual_survival = Decimal(1) - flow / start
            hazard = -annual_survival.ln() / Decimal(12)
            monthly_survival = (-hazard).exp()
            monthly_probability = Decimal(1) - monthly_survival
            average = sum(
                start * (-hazard * Decimal(month)).exp()
                for month in range(12)
            ) / Decimal(12)
            expected = average * Decimal(12) * monthly_probability
            if abs(expected - flow) > Decimal("1e-35") * max(flow, Decimal(1)):
                raise AssertionError("entry exposure and hazard no longer close")

    return EntryCalculation(
        stream=stream,
        year=year,
        state=state,
        start_vacancies=start_vacancies,
        target_entries=entries,
        end_vacancies=start_vacancies - entries,
        average_expected_vacancies=float(average),
        monthly_probability=float(monthly_probability),
        monthly_hazard=float(hazard),
    )


def _mortality_rate(inputs: Inputs, key: tuple[int, str, str, str]) -> tuple[Decimal, str]:
    value = inputs.mortality[key]
    if value is not None:
        return value, "published"
    year, _state, sex, age_band = key
    if key not in FALLBACK_CELLS:
        raise ValueError(f"unapproved mortality fallback {key!r}")
    return inputs.national_mortality[(year, sex, age_band)], "national_zero_exposure_fallback"


def _state_capacities(plan: build_state.StatePlan) -> tuple[dict[str, int], dict[str, int]]:
    if plan.scale != "full" or plan.divisor != 1:
        raise ValueError("rate derivation requires the full-scale state plan")
    births = {state: 0 for state in MODEL_STATES}
    overseas = {state: 0 for state in MODEL_STATES}
    for (state, _sex), count in plan.birth_counts.items():
        births[state] += count
    for (state, _sex, _age_band), count in plan.overseas_counts.items():
        overseas[state] += count
    if sum(births.values()) != plan.birth_slots:
        raise AssertionError("birth state capacities do not close")
    if sum(overseas.values()) != plan.overseas_slots:
        raise AssertionError("overseas state capacities do not close")
    return births, overseas


def _life_table_metrics(inputs: Inputs) -> tuple[LifeTableMetric, ...]:
    errors: dict[tuple[int, int], list[float]] = {}
    for start, end, state, sex, age, qx in inputs.life_tables:
        source_band = SOURCE_AGE_BANDS[min(age // 5, len(SOURCE_AGE_BANDS) - 1)]
        probabilities = []
        for year in range(start, end + 1):
            rate, _provenance = _mortality_rate(inputs, (year, state, sex, source_band))
            with localcontext() as context:
                context.prec = 50
                probability = Decimal(1) - (-(rate / Decimal(1000))).exp()
            probabilities.append(probability)
        direct_qx = sum(probabilities) / Decimal(len(probabilities))
        errors.setdefault((start, end), []).append(float(direct_qx - qx))

    metrics = []
    for (start, end), values in sorted(errors.items()):
        count = len(values)
        metrics.append(
            LifeTableMetric(
                period_start=start,
                period_end=end,
                cells=count,
                mean_signed_error=sum(values) / count,
                mean_absolute_error=sum(abs(value) for value in values) / count,
                root_mean_square_error=math.sqrt(
                    sum(value * value for value in values) / count
                ),
                maximum_absolute_error=max(abs(value) for value in values),
            )
        )
    return tuple(metrics)


def derive_rates(
    inputs: Inputs | None = None,
    plan: build_state.StatePlan | None = None,
) -> RateArtifacts:
    """Derive all annual parameter maps and supporting evidence."""
    inputs = inputs or load_inputs()
    plan = plan or build_state.build_plan("full")
    birth_capacities, overseas_capacities = _state_capacities(plan)
    birth_vacancies = dict(birth_capacities)
    overseas_vacancies = dict(overseas_capacities)
    annual_params: dict[int, dict[str, float]] = {}
    calculations: list[EntryCalculation] = []
    provenance: list[dict[str, object]] = []
    expected_names = expected_parameter_names()

    for year in RUN_YEARS:
        params = dict(FREE_DEFAULTS)

        for state in MODEL_STATES:
            birth = entry_calculation(
                "birth", year, state, birth_vacancies[state], inputs.births[(year, state)]
            )
            calculations.append(birth)
            params[f"birth_rate_{state}"] = birth.monthly_hazard
            birth_vacancies[state] = birth.end_vacancies

        for state in MODEL_STATES:
            for source_band, model_band in AGE_BANDS:
                for sex in SEXES:
                    key = (year, state, sex, source_band)
                    rate, source = _mortality_rate(inputs, key)
                    name = f"mortality_{state}_{model_band}_{sex}"
                    params[name] = float(rate / MORTALITY_DIVISOR)
                    if source != "published":
                        provenance.append(
                            {
                                "year": year,
                                "parameter": name,
                                "provenance": source,
                                "source": "mortality_rates_national_age_sex.csv",
                                "source_cell": {
                                    "year": year,
                                    "sex": sex,
                                    "age_band": source_band,
                                },
                                "rate_per_1000": float(rate),
                            }
                        )

        for state in MODEL_STATES:
            arrivals, departures = inputs.overseas[(year, state)]
            arrival = entry_calculation(
                "overseas", year, state, overseas_vacancies[state], arrivals
            )
            calculations.append(arrival)
            params[f"overseas_arrival_{state}"] = arrival.monthly_hazard
            overseas_vacancies[state] = arrival.end_vacancies

            population = inputs.erp[(year, state)]
            if population <= 0:
                raise ValueError(f"ERP exposure is zero for {year} {state}")
            params[f"emigration_{state}"] = float(
                Decimal(departures) / Decimal(population) / Decimal(12)
            )

        if set(params) != expected_names:
            raise AssertionError(
                f"{year} parameter keys changed: "
                f"missing={sorted(expected_names - set(params))!r}, "
                f"extra={sorted(set(params) - expected_names)!r}"
            )
        if len(params) != 377:
            raise AssertionError(f"{year} has {len(params)} parameters, expected 377")
        for name, value in params.items():
            if not math.isfinite(value) or value < 0:
                raise ValueError(f"{year} parameter {name} is invalid: {value!r}")
        annual_params[year] = params

    if len(provenance) != 2:
        raise AssertionError(f"expected two mortality fallbacks, got {len(provenance)}")
    return RateArtifacts(
        inputs=inputs,
        annual_params=annual_params,
        entry_calculations=tuple(calculations),
        fallback_provenance=tuple(provenance),
        life_table_metrics=_life_table_metrics(inputs),
        birth_capacities=birth_capacities,
        overseas_capacities=overseas_capacities,
    )


def _prior_for_default(value: float, spread: float = DIRECT_LOG_NORMAL_SPREAD) -> dict:
    if value > 0:
        with localcontext() as context:
            context.prec = 50
            location = float(Decimal(str(value)).ln())
        return {
            "family": "log_normal",
            "location": location,
            "spread": spread,
            "centering": "median",
        }
    return {
        "family": "normal",
        "location": 0.0,
        "spread": ZERO_MORTALITY_NORMAL_SPREAD,
        "centering": "mean",
        "reason": "published_zero_at_abs_one_decimal_precision",
    }


def priors_payload(artifacts: RateArtifacts) -> dict:
    """Build PRD 0008's sole machine-readable fixed/free registry."""
    values_2010 = artifacts.annual_params[2010]
    parameters = {}
    for name in sorted(values_2010):
        if name in FREE_DEFAULTS:
            if name == "interstate_base":
                group, evidence, spread = "interstate_base", "published_interstate_margins", 0.5
            elif name.startswith("push_"):
                group, evidence, spread = "push_o", "published_interstate_margins", 0.25
            elif name.startswith("pull_"):
                group, evidence, spread = "pull_d", "published_interstate_margins", 0.25
            elif name == "peak_months":
                group, evidence, spread = "peak", "simulated_stock_age_structure", 0.25
            else:
                group, evidence, spread = "k", "simulated_stock_age_structure", 0.5
            parameters[name] = {
                "classification": "free",
                "identification": {"group": group, "evidence": evidence},
                "precalibration_value": FREE_DEFAULTS[name],
                "lean_prior_2010": _prior_for_default(FREE_DEFAULTS[name], spread),
            }
            continue

        if name.startswith("birth_rate_"):
            source, role = "births_state.csv", "fertility"
        elif name.startswith("mortality_"):
            source, role = "mortality_rates_state_age_sex.csv", "mortality"
        elif name.startswith("overseas_arrival_"):
            source, role = "overseas_margins.csv", "overseas_arrival"
        elif name.startswith("emigration_"):
            source, role = "overseas_margins.csv", "emigration"
        else:
            raise AssertionError(f"unclassified parameter {name}")
        parameters[name] = {
            "classification": "fixed",
            "identification": {"group": "abs_direct", "evidence": source},
            "role": role,
            "source": source,
            "lean_prior_2010": _prior_for_default(values_2010[name]),
        }

    fixed = sum(item["classification"] == "fixed" for item in parameters.values())
    free = sum(item["classification"] == "free" for item in parameters.values())
    if (fixed, free) != (360, 17):
        raise AssertionError(f"fixed/free split changed to {(fixed, free)!r}")
    return {
        "format": "sembla.abs-priors/v1",
        "parameter_count": len(parameters),
        "classification_counts": {
            "fixed": fixed,
            "free": free,
            "fixed_normalization": 0,
        },
        "free_parameters": sorted(FREE_DEFAULTS),
        "parameters": parameters,
        "provenance_overrides": list(artifacts.fallback_provenance),
    }


def _load_fidelity(path: pathlib.Path) -> dict:
    predeclaration_bytes = FIDELITY_PREDECLARATION.read_bytes()
    predeclaration_hash = hashlib.sha256(predeclaration_bytes).hexdigest()
    if predeclaration_hash != FIDELITY_PREDECLARATION_SHA256:
        raise ValueError("fidelity predeclaration bytes changed")
    predeclaration = json.loads(predeclaration_bytes)
    if predeclaration.get("status") != "predeclared":
        raise ValueError("fidelity predeclaration status changed")

    payload = json.loads(path.read_text(encoding="utf-8"))
    if payload.get("format") != "sembla.australian-population-fidelity/v1":
        raise ValueError(f"{path} has an unknown fidelity-evidence format")
    if payload.get("pilot_seeds") != predeclaration.get("pilot_seeds"):
        raise ValueError("fidelity pilot seeds changed")
    if payload.get("held_out_seed") != predeclaration.get("held_out_seed"):
        raise ValueError("fidelity held-out seed changed")
    if payload.get("tolerance_rule") != predeclaration.get("tolerance_rule"):
        raise ValueError("fidelity tolerance rule changed")
    if payload.get("predeclaration_path") != FIDELITY_PREDECLARATION.name:
        raise ValueError("fidelity predeclaration path changed")
    if payload.get("predeclaration_sha256") != predeclaration_hash:
        raise ValueError("fidelity predeclaration fingerprint changed")
    return payload


def _source_rows(inputs: Inputs) -> Iterable[str]:
    source_ids = [
        "births_state",
        "births_sex",
        "deaths_state_age_sex",
        "mortality_rates_state_age_sex",
        "overseas_arrivals",
        "overseas_departures",
        "components_national",
        "life_tables_2018_2020",
        "life_tables_2019_2021",
        "life_tables_2020_2022",
        "life_tables_2021_2023",
        "life_tables_2022_2024",
    ]
    for source_id in source_ids:
        source = inputs.sources[source_id]
        yield (
            f"| `{source_id}` | {source['release']} | {source['release_date']} | "
            f"`{source['sha256']}` |"
        )


def _fidelity_report(payload: dict) -> list[str]:
    lines = [
        "## Predeclared 2010 fidelity check",
        "",
        "The implementation workflow wrote `params/fidelity-2010-predeclaration.json` "
        "before its first simulation: pilot seeds 1001--1010, held-out seed 2001, "
        "12 ticks at one-in-a-hundred scale, full-scale counts obtained by multiplying "
        "by 100, and tolerance `ceil(3 × pilot sample SD)` with no bias term. Its "
        f"preserved bytes have SHA-256 `{FIDELITY_PREDECLARATION_SHA256}`. Direct rates "
        "and the tolerance rule were not tuned. The declaration and measured evidence "
        "are part of the same uncommitted implementation series, so Git history alone "
        "does not independently timestamp their order; the implementation-session "
        "workflow transcript is the temporal record.",
        "",
    ]
    if payload.get("status") != "measured":
        lines.extend(["Status: **predeclared; measurements pending**.", ""])
        return lines

    lines.extend(
        [
            "| outcome | published eight-state target | pilot mean | pilot sample SD | tolerance | held-out result | signed error | pass |",
            "|---|---:|---:|---:|---:|---:|---:|:---:|",
        ]
    )
    for outcome in ("births", "deaths"):
        stats = payload["statistics"][outcome]
        held = payload["held_out"][outcome]
        lines.append(
            f"| {outcome} | {payload['targets'][outcome]:,} | "
            f"{stats['mean']:.3f} | {stats['sample_standard_deviation']:.3f} | "
            f"{stats['tolerance']:,} | {held['simulated']:,} | "
            f"{held['signed_error']:+,} | {'yes' if held['pass'] else 'no'} |"
        )

    age_target = sum(
        row["published"]
        for row in payload["held_out"]["deaths_by_age_band"].values()
    )
    not_stated = payload["targets"]["deaths"] - age_target
    lines.extend(
        [
            "",
            f"The national and state death target includes {not_stated:,} published "
            "`not_stated` deaths. The five-year age-band table excludes that source "
            f"category and therefore closes to {age_target:,}, while every simulated "
            "death has an observed model age and is included in a model band.",
        ]
    )

    for heading, key in (
        ("Held-out births by state", "births_by_state"),
        ("Held-out deaths by state", "deaths_by_state"),
        ("Held-out deaths by five-year age band", "deaths_by_age_band"),
        ("Held-out overseas arrivals by state", "overseas_arrivals_by_state"),
        ("Held-out overseas departures by state", "overseas_departures_by_state"),
    ):
        lines.extend(
            [
                "",
                f"### {heading}",
                "",
                "| cell | published | simulated | signed error |",
                "|---|---:|---:|---:|",
            ]
        )
        for cell, row in sorted(payload["held_out"][key].items()):
            lines.append(
                f"| {cell} | {row['published']:,} | {row['simulated']:,} | "
                f"{row['signed_error']:+,} |"
            )
    lines.append("")
    return lines


def render_report(artifacts: RateArtifacts, fidelity: dict) -> str:
    inputs = artifacts.inputs
    calculations = {
        (row.stream, row.year, row.state): row
        for row in artifacts.entry_calculations
    }
    lines = [
        "# ABS-derived Australian population rates",
        "",
        "This report is generated by `data/abs/rates.py` from checksum-pinned, "
        "committed extracts. No network access, Sembla API, or third-party Python "
        "package is used.",
        "",
        "## Conversion contract",
        "",
        "- Run year `y` uses calendar registration year `y` for births and mortality.",
        "- Mortality monthly hazard: `rate_per_1000 / 1000 / 12`.",
        "- Birth and overseas-arrival monthly hazard: `-log1p(-E / V) / 12`, "
        "  where `V` is the projected start-of-year full-scale vacancy and `E` is "
        "  the published annual entry count. This is exactly equivalent to dividing "
        "  entries by the twelve expected start-of-month vacancy exposures to obtain "
        "  a monthly activation probability, then converting that probability to a "
        "  continuous-time hazard.",
        "- Emigration monthly central hazard: `departures / start-year state ERP / 12`.",
        "- Vacancy paths subtract published entries, never simulated entries, and all "
        "  entry hazards are derived from the full-scale pool so they are scale-invariant.",
        "",
        "Offline logarithms are parameter derivation only. No transcendental expression "
        "is added to the model or IR.",
        "",
        "## Source provenance",
        "",
        "| manifest id | ABS release | release date | pinned SHA-256 |",
        "|---|---|---|---|",
        *_source_rows(inputs),
        "",
        "## Mortality mapping and fallback",
        "",
        "All 336 state × sex × five-year-band parameters are retained. Published `0.0` "
        "rates remain exact zero hazards. Positive 2010 defaults use median-centred "
        "LogNormal priors with spread 0.5; exact zero defaults use the sole documented "
        "exception, `Normal(0, 0.05 / 12000)`, because a LogNormal cannot be centred "
        "at zero. These parameters are fixed and never enter the inference vector.",
        "",
        "Exactly two source cells use `national_zero_exposure_fallback`:",
        "",
        "| run year | parameter | national rate per 1,000 | monthly hazard | provenance |",
        "|---:|---|---:|---:|---|",
    ]
    for row in artifacts.fallback_provenance:
        value = artifacts.annual_params[row["year"]][row["parameter"]]
        lines.append(
            f"| {row['year']} | `{row['parameter']}` | {row['rate_per_1000']:.1f} | "
            f"{value:.15g} | `{row['provenance']}` |"
        )

    lines.extend(
        [
            "",
            "## Calendar registrations versus financial-year components",
            "",
            "The eight-state registration series and national financial-year occurrence "
            "components have different period and geography contracts. Signed differences "
            "below are retained evidence, not adjustments. Registered deaths include the "
            "published `not_stated` band.",
            "",
            "| year | registered births, 8 states | registered births, Australia | FY births, Australia | 8-state minus FY | registered deaths, 8 states | FY deaths, Australia | 8-state minus FY |",
            "|---:|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for year in RUN_YEARS:
        births = sum(inputs.births[(year, state)] for state in MODEL_STATES)
        deaths = sum(
            inputs.deaths[(year, state, sex, age_band)]
            for state in MODEL_STATES
            for sex in SEXES
            for age_band in DEATH_AGE_BANDS
        )
        fy = inputs.components[year]
        lines.append(
            f"| {year} | {births:,} | {inputs.national_births[year]:,} | "
            f"{fy['births']:,} | {births - fy['births']:+,} | {deaths:,} | "
            f"{fy['deaths']:,} | {deaths - fy['deaths']:+,} |"
        )

    lines.extend(
        [
            "",
            "## Projected entry-vacancy denominators",
            "",
            "`average V` is the arithmetic mean of the twelve expected start-of-month "
            "vacancy counts under the listed hazard. The expected annual activation count "
            "is therefore exactly the published target before finite-scale Monte Carlo noise.",
            "",
            "| year | state | birth start V | births | birth end V | birth average V | birth monthly h | overseas start V | arrivals | overseas end V | overseas average V | overseas monthly h |",
            "|---:|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|",
        ]
    )
    for year in RUN_YEARS:
        for state in MODEL_STATES:
            birth = calculations[("birth", year, state)]
            overseas = calculations[("overseas", year, state)]
            lines.append(
                f"| {year} | {state} | {birth.start_vacancies:,} | "
                f"{birth.target_entries:,} | {birth.end_vacancies:,} | "
                f"{birth.average_expected_vacancies:.3f} | {birth.monthly_hazard:.15g} | "
                f"{overseas.start_vacancies:,} | {overseas.target_entries:,} | "
                f"{overseas.end_vacancies:,} | {overseas.average_expected_vacancies:.3f} | "
                f"{overseas.monthly_hazard:.15g} |"
            )

    lines.extend(
        [
            "",
            "## Fixed/free parameter registry",
            "",
            "Every annual file contains all 377 parameters: 360 ABS-derived fixed "
            "parameters and exactly 17 free migration parameters. The free set is:",
            "",
            *[f"- `{name}`" for name in sorted(FREE_DEFAULTS)],
            "",
            "`interstate_base`, `push_o`, and `pull_d` are identified from published "
            "interstate margins. `peak` and `k` are identified only through simulated "
            "stock age structure. The machine-readable authority is `params/priors.json`; "
            "the unchanged placeholder values in annual files are pre-calibration values, "
            "not estimates.",
            "",
            "## Independent period-life-table comparison",
            "",
            "For validation only, the direct annual central rate is converted to "
            "`q = 1 - exp(-rate_per_1000 / 1000)`, averaged over each three-year life-table "
            "period, and compared with published single-age `qx`. These errors never rescale "
            "or replace a direct annual rate.",
            "",
            "| period | cells | mean signed error | MAE | RMSE | maximum absolute error |",
            "|---|---:|---:|---:|---:|---:|",
        ]
    )
    for metric in artifacts.life_table_metrics:
        lines.append(
            f"| {metric.period_start}--{metric.period_end} | {metric.cells:,} | "
            f"{metric.mean_signed_error:.9f} | {metric.mean_absolute_error:.9f} | "
            f"{metric.root_mean_square_error:.9f} | "
            f"{metric.maximum_absolute_error:.9f} |"
        )

    lines.extend(["", *_fidelity_report(fidelity)])
    lines.extend(
        [
            "## Fixed-pool caveat",
            "",
            "Entry rates are re-derived every year and are **not comparable across years "
            "as behavioural quantities**: they are activation hazards relative to a "
            "single-use, depleting fixed pool. If a vacancy margin approaches zero, entry "
            "flow is suppressed regardless of theta. PRD 0008's saturation diagnostic is "
            "therefore a correctness check, not a convenience.",
            "",
        ]
    )
    return "\n".join(lines)


def write_artifacts(
    artifacts: RateArtifacts,
    params_dir: pathlib.Path = PARAMS,
    report_path: pathlib.Path = REPORT,
    fidelity_path: pathlib.Path = FIDELITY_EVIDENCE,
) -> None:
    params_dir = pathlib.Path(params_dir)
    for year in RUN_YEARS:
        canonical.write_json(params_dir / f"{year}.json", artifacts.annual_params[year])
    canonical.write_json(params_dir / "priors.json", priors_payload(artifacts))
    fidelity = _load_fidelity(pathlib.Path(fidelity_path))
    report_path = pathlib.Path(report_path)
    report_path.parent.mkdir(parents=True, exist_ok=True)
    report_path.write_text(
        render_report(artifacts, fidelity), encoding="utf-8", newline=""
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--params-dir", type=pathlib.Path, default=PARAMS)
    parser.add_argument("--report", type=pathlib.Path, default=REPORT)
    parser.add_argument(
        "--fidelity-evidence", type=pathlib.Path, default=FIDELITY_EVIDENCE
    )
    args = parser.parse_args(argv)
    artifacts = derive_rates()
    write_artifacts(artifacts, args.params_dir, args.report, args.fidelity_evidence)
    print(
        f"wrote {len(artifacts.annual_params)} annual parameter files, priors, "
        f"and {args.report}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Offline Poisson gravity fit for the spatial interstate parameters.

`DECISIONS.md` §N5 writes the monthly interstate hazard as

    interstate_base * push[origin] * pull[destination] * ageProfile(age_months)

with `push_nsw` and `pull_nsw` fixed at one. The published origin-destination
table therefore identifies fifteen spatial quantities directly, without any
simulation: with an age-weighted origin exposure offset the model is an ordinary
Poisson log-linear (quasi-independence) fit over the 56 off-diagonal cells,
leaving 41 residual degrees of freedom.

`peak_months` and `k` are deliberately *not* fitted here. An all-age O-D table
carries no separable age information — any aggregate age effect at an origin is
absorbed by that origin's push factor — so they are held at their prior centres
and left to the NPE stage (PRD 0008 §3).

Standard library only, like the rest of `data/abs/`.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import pathlib

import canonical

HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"
PARAMS = HERE / "params"
FITTED = PARAMS / "gravity"

FORMAT = "sembla.abs-gravity-fit/v1"
STATES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")
REFERENCE = "nsw"
NON_REFERENCE = tuple(state for state in STATES if state != REFERENCE)
RUN_YEARS = tuple(range(2010, 2025))
TICKS = 12
MONTHS_PER_YEAR = 12
CELL_COUNT = len(STATES) * (len(STATES) - 1)
PARAMETER_COUNT = 1 + 2 * len(NON_REFERENCE)
RESIDUAL_DEGREES_OF_FREEDOM = CELL_COUNT - PARAMETER_COUNT

# Newton steps on an exactly-concave Poisson log-likelihood; the tolerance is on
# the step itself, so convergence is quadratic and the iteration count is small.
MAXIMUM_ITERATIONS = 100
STEP_TOLERANCE = 1e-13


class GravityFitError(ValueError):
    """An input or a fit contract failed."""


def _rows(path: pathlib.Path) -> list[dict[str, str]]:
    try:
        with path.open(encoding="utf-8", newline="") as handle:
            return list(csv.DictReader(handle))
    except OSError as error:
        raise GravityFitError(f"cannot read {path}: {error}") from error


def load_flows(extracts: pathlib.Path = EXTRACTS) -> dict[int, dict[tuple[str, str], int]]:
    """Read every published directed origin-destination count."""
    flows: dict[int, dict[tuple[str, str], int]] = {}
    for row in _rows(extracts / "interstate_flows.csv"):
        year = int(row["year"])
        origin = row["origin"]
        destination = row["destination"]
        if origin not in STATES or destination not in STATES:
            raise GravityFitError(f"unknown state in interstate_flows.csv: {row}")
        if origin == destination:
            raise GravityFitError(f"diagonal O-D cell is not published: {row}")
        cells = flows.setdefault(year, {})
        if (origin, destination) in cells:
            raise GravityFitError(f"duplicate O-D cell {origin}->{destination} in {year}")
        cells[(origin, destination)] = int(row["persons"])
    for year in RUN_YEARS:
        if year not in flows:
            raise GravityFitError(f"interstate_flows.csv is missing run year {year}")
        if len(flows[year]) != CELL_COUNT:
            raise GravityFitError(
                f"run year {year} has {len(flows[year])} O-D cells, expected {CELL_COUNT}"
            )
    return flows


def load_erp(extracts: pathlib.Path = EXTRACTS) -> dict[tuple[int, str], dict[int, int]]:
    """Read the single-year-of-age ERP stock that supplies origin exposure."""
    stock: dict[tuple[int, str], dict[int, int]] = {}
    for row in _rows(extracts / "erp_state_age_sex.csv"):
        key = (int(row["year"]), row["state"])
        ages = stock.setdefault(key, {})
        age = int(row["age"])
        ages[age] = ages.get(age, 0) + int(row["persons"])
    return stock


def load_margins(extracts: pathlib.Path = EXTRACTS) -> dict[tuple[int, str], dict[str, int]]:
    """Read the separately published arrival and departure margins."""
    margins: dict[tuple[int, str], dict[str, int]] = {}
    for row in _rows(extracts / "interstate_margins.csv"):
        margins[(int(row["run_year"]), row["state"])] = {
            "arrivals": int(row["arrivals"]),
            "departures": int(row["departures"]),
        }
    return margins


def age_profile(age_months: int, peak_months: float, k: float) -> float:
    """The rational age profile exactly as the model evaluates it."""
    difference = age_months - peak_months
    return 1.0 / (1.0 + k * difference * difference)


def _age_exposure(age_years: int, peak_months: float, k: float) -> float:
    """Twelve ticks of exposure for one person of a published single year of age.

    `build_state.py` spreads a year of age uniformly across its twelve months and
    `age_monthly` advances every present row each tick, so a person's exposure is
    the profile averaged over the birth-month offset and summed over the run's
    twelve ticks.
    """
    total = 0.0
    for offset in range(MONTHS_PER_YEAR):
        base = age_years * MONTHS_PER_YEAR + offset
        for tick in range(TICKS):
            total += age_profile(base + tick, peak_months, k)
    return total / MONTHS_PER_YEAR


def origin_exposure(
    ages: dict[int, int], peak_months: float, k: float
) -> float:
    """Age-weighted person-months of exposure for one origin."""
    return sum(
        persons * _age_exposure(age, peak_months, k)
        for age, persons in sorted(ages.items())
    )


def _design_row(origin: str, destination: str) -> list[float]:
    row = [0.0] * PARAMETER_COUNT
    row[0] = 1.0
    if origin != REFERENCE:
        row[1 + NON_REFERENCE.index(origin)] = 1.0
    if destination != REFERENCE:
        row[1 + len(NON_REFERENCE) + NON_REFERENCE.index(destination)] = 1.0
    return row


def _solve(matrix: list[list[float]], vector: list[float]) -> list[float]:
    """Gaussian elimination with partial pivoting."""
    size = len(vector)
    augmented = [row[:] + [vector[index]] for index, row in enumerate(matrix)]
    for column in range(size):
        pivot = max(range(column, size), key=lambda row: abs(augmented[row][column]))
        if abs(augmented[pivot][column]) < 1e-300:
            raise GravityFitError("gravity design matrix is singular")
        augmented[column], augmented[pivot] = augmented[pivot], augmented[column]
        for row in range(column + 1, size):
            factor = augmented[row][column] / augmented[column][column]
            if factor == 0.0:
                continue
            for index in range(column, size + 1):
                augmented[row][index] -= factor * augmented[column][index]
    solution = [0.0] * size
    for column in reversed(range(size)):
        total = augmented[column][size]
        for index in range(column + 1, size):
            total -= augmented[column][index] * solution[index]
        solution[column] = total / augmented[column][column]
    return solution


def _deviance_contribution(observed: int, expected: float) -> float:
    if observed <= 0:
        return 2.0 * expected
    return 2.0 * (observed * math.log(observed / expected) - (observed - expected))


def fit_cells(
    observed: dict[tuple[str, str], int], exposure: dict[str, float]
) -> dict:
    """Newton/IRLS maximum likelihood for one year's 56 published cells."""
    cells = [
        (origin, destination)
        for origin in STATES
        for destination in STATES
        if origin != destination
    ]
    missing = [cell for cell in cells if cell not in observed]
    if missing:
        raise GravityFitError(f"missing published O-D cells: {missing}")
    for origin in STATES:
        if exposure[origin] <= 0.0:
            raise GravityFitError(f"origin {origin} has no exposure")
    design = [_design_row(origin, destination) for origin, destination in cells]
    offsets = [math.log(exposure[origin]) for origin, _ in cells]
    counts = [observed[cell] for cell in cells]

    coefficients = [0.0] * PARAMETER_COUNT
    total_observed = sum(counts)
    total_exposure = sum(math.exp(offset) for offset in offsets)
    coefficients[0] = math.log(total_observed / total_exposure)

    iterations = 0
    step_size = float("inf")
    while step_size > STEP_TOLERANCE:
        if iterations >= MAXIMUM_ITERATIONS:
            raise GravityFitError("gravity fit did not converge")
        expected = [
            math.exp(
                offsets[index]
                + sum(design[index][term] * coefficients[term] for term in range(PARAMETER_COUNT))
            )
            for index in range(len(cells))
        ]
        gradient = [
            sum(
                design[index][term] * (counts[index] - expected[index])
                for index in range(len(cells))
            )
            for term in range(PARAMETER_COUNT)
        ]
        hessian = [
            [
                sum(
                    design[index][row] * design[index][column] * expected[index]
                    for index in range(len(cells))
                )
                for column in range(PARAMETER_COUNT)
            ]
            for row in range(PARAMETER_COUNT)
        ]
        step = _solve(hessian, gradient)
        coefficients = [
            coefficients[term] + step[term] for term in range(PARAMETER_COUNT)
        ]
        step_size = max(abs(value) for value in step)
        iterations += 1

    expected = [
        math.exp(
            offsets[index]
            + sum(design[index][term] * coefficients[term] for term in range(PARAMETER_COUNT))
        )
        for index in range(len(cells))
    ]
    residual_cells = [
        {
            "deviance_contribution": _deviance_contribution(counts[index], expected[index]),
            "destination": cells[index][1],
            "expected": expected[index],
            "observed": counts[index],
            "origin": cells[index][0],
            "pearson_residual": (counts[index] - expected[index])
            / math.sqrt(expected[index]),
            "signed_residual": counts[index] - expected[index],
        }
        for index in range(len(cells))
    ]
    values = {"interstate_base": math.exp(coefficients[0])}
    for index, state in enumerate(NON_REFERENCE):
        values[f"push_{state}"] = math.exp(coefficients[1 + index])
        values[f"pull_{state}"] = math.exp(
            coefficients[1 + len(NON_REFERENCE) + index]
        )
    return {
        "cells": residual_cells,
        "deviance": sum(cell["deviance_contribution"] for cell in residual_cells),
        "iterations": iterations,
        "maximum_absolute_gradient": max(
            abs(
                sum(
                    design[index][term] * (counts[index] - expected[index])
                    for index in range(len(cells))
                )
            )
            for term in range(PARAMETER_COUNT)
        ),
        "values": values,
    }


def _margin_comparison(
    fit: dict, margins: dict[tuple[int, str], dict[str, int]], year: int
) -> list[dict]:
    """Compare the fitted table's margins with the separately published ones.

    The O-D table and the margin workbook are different ABS products. Where they
    disagree — notably 2020 — the disagreement is reported, never reconciled.
    """
    departures: dict[str, float] = {state: 0.0 for state in STATES}
    arrivals: dict[str, float] = {state: 0.0 for state in STATES}
    observed_departures: dict[str, int] = {state: 0 for state in STATES}
    observed_arrivals: dict[str, int] = {state: 0 for state in STATES}
    for cell in fit["cells"]:
        departures[cell["origin"]] += cell["expected"]
        arrivals[cell["destination"]] += cell["expected"]
        observed_departures[cell["origin"]] += cell["observed"]
        observed_arrivals[cell["destination"]] += cell["observed"]
    comparison = []
    for state in STATES:
        published = margins.get((year, state))
        if published is None:
            raise GravityFitError(f"interstate_margins.csv is missing {year} {state}")
        comparison.append(
            {
                "fitted_arrivals": arrivals[state],
                "fitted_departures": departures[state],
                "published_margin_arrivals": published["arrivals"],
                "published_margin_departures": published["departures"],
                "state": state,
                "table_arrivals": observed_arrivals[state],
                "table_departures": observed_departures[state],
                "table_minus_margin_arrivals": observed_arrivals[state]
                - published["arrivals"],
                "table_minus_margin_departures": observed_departures[state]
                - published["departures"],
            }
        )
    return comparison


def fit_all_years(
    extracts: pathlib.Path = EXTRACTS, params_dir: pathlib.Path = PARAMS
) -> dict:
    """Fit every run year and return the complete report."""
    flows = load_flows(extracts)
    stock = load_erp(extracts)
    margins = load_margins(extracts)
    priors = json.loads((params_dir / "priors.json").read_text(encoding="utf-8"))
    free = set(priors["free_parameters"])
    centres = {
        name: priors["parameters"][name]["precalibration_value"] for name in free
    }
    peak_months = centres["peak_months"]
    k = centres["k"]

    years = []
    for year in RUN_YEARS:
        exposure = {}
        for state in STATES:
            ages = stock.get((year, state))
            if not ages:
                raise GravityFitError(f"erp_state_age_sex.csv is missing {year} {state}")
            exposure[state] = origin_exposure(ages, peak_months, k)
        fit = fit_cells(flows[year], exposure)
        years.append(
            {
                "cells": fit["cells"],
                "deviance": fit["deviance"],
                "deviance_per_degree_of_freedom": fit["deviance"]
                / RESIDUAL_DEGREES_OF_FREEDOM,
                "exposure_person_months": {
                    state: exposure[state] for state in STATES
                },
                "iterations": fit["iterations"],
                "margin_comparison": _margin_comparison(fit, margins, year),
                "maximum_absolute_gradient": fit["maximum_absolute_gradient"],
                "run_year": year,
                "values": fit["values"],
            }
        )
    return {
        "age_profile_centres": {"k": k, "peak_months": peak_months},
        "cell_count": CELL_COUNT,
        "fitted_parameters": sorted(years[0]["values"]),
        "format": FORMAT,
        "held_at_prior_centre": ["k", "peak_months"],
        "reference_state": REFERENCE,
        "residual_degrees_of_freedom": RESIDUAL_DEGREES_OF_FREEDOM,
        "spatial_parameter_count": PARAMETER_COUNT,
        "years": years,
    }


def annual_parameters(report: dict, params_dir: pathlib.Path = PARAMS) -> dict[int, dict]:
    """Full 377-parameter files with the fitted spatial values substituted in."""
    priors = json.loads((params_dir / "priors.json").read_text(encoding="utf-8"))
    free = set(priors["free_parameters"])
    written: dict[int, dict] = {}
    for year_report in report["years"]:
        year = year_report["run_year"]
        baseline = json.loads(
            (params_dir / f"{year}.json").read_text(encoding="utf-8")
        )
        values = dict(baseline)
        for name, value in year_report["values"].items():
            if name not in free:
                raise GravityFitError(f"{name} is not a free parameter")
            values[name] = value
        if set(values) != set(baseline):
            raise GravityFitError(f"{year} fitted parameters changed the parameter set")
        for name, value in values.items():
            if name not in free and value != baseline[name]:
                raise GravityFitError(f"{year} fitted parameters altered fixed {name}")
        written[year] = values
    return written


def write_artifacts(
    report: dict,
    parameters: dict[int, dict],
    out_dir: pathlib.Path = FITTED,
) -> None:
    canonical.write_json(out_dir / "fit-report.json", report)
    for year, values in sorted(parameters.items()):
        canonical.write_json(out_dir / f"{year}.json", values)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--extracts", type=pathlib.Path, default=EXTRACTS)
    parser.add_argument("--params-dir", type=pathlib.Path, default=PARAMS)
    parser.add_argument("--out", type=pathlib.Path, default=FITTED)
    args = parser.parse_args(argv)
    report = fit_all_years(args.extracts, args.params_dir)
    parameters = annual_parameters(report, args.params_dir)
    write_artifacts(report, parameters, args.out)
    worst = max(report["years"], key=lambda year: year["deviance"])
    print(
        f"fitted {len(report['years'])} run years over {CELL_COUNT} O-D cells; "
        f"worst deviance {worst['deviance']:.1f} "
        f"({worst['deviance_per_degree_of_freedom']:.1f} per df) in "
        f"{worst['run_year']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

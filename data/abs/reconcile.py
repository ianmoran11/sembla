"""Reconcile the committed extracts and write a report.

Checks that can fail the pipeline are separated from residuals that are
expected to be nonzero. ABS components are modelled, confidentialised,
constrained and revised, so a residual between published components and
estimated resident population is quantified and reported, never forced away
(DECISIONS.md N13).
"""

from __future__ import annotations

import collections
import pathlib
import sys

from canonical import read_csv
from normalise import STATE_CODES, TERMINAL_AGE

HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"

EXPECTED_SEXES = ("female", "male")
EXPECTED_YEARS = tuple(range(2010, 2026))
EXPECTED_RUN_YEARS = tuple(range(2010, 2025))


class Report:
    def __init__(self) -> None:
        self.lines: list[str] = []
        self.failures: list[str] = []

    def say(self, text: str = "") -> None:
        self.lines.append(text)

    def check(self, ok: bool, description: str) -> bool:
        self.say(f"- {'PASS' if ok else 'FAIL'}: {description}")
        if not ok:
            self.failures.append(description)
        return ok


def load_erp() -> tuple[dict, dict]:
    _, state_rows = read_csv(EXTRACTS / "erp_state_age_sex.csv")
    _, national_rows = read_csv(EXTRACTS / "erp_national_age_sex.csv")
    state = {
        (int(y), s, sex, int(age)): int(n) for y, s, sex, age, n in state_rows
    }
    national = {
        (int(y), sex, int(age)): int(n) for y, _r, sex, age, n in national_rows
    }
    return state, national


def reconcile(report: Report) -> None:
    state, national = load_erp()
    years = list(EXPECTED_YEARS)
    state_years = {key[0] for key in state}
    national_years = {key[0] for key in national}
    report.say(f"# ABS extract reconciliation")
    report.say()
    report.say(f"Years covered: {years[0]}-{years[-1]} ({len(years)} annual points).")
    report.say(f"States: {', '.join(STATE_CODES)}.")
    report.say(f"Ages: 0-{TERMINAL_AGE}, where {TERMINAL_AGE} is the open terminal group.")
    report.say()

    report.say("## Structural completeness")
    report.say()
    coverage_ok = report.check(
        state_years == set(EXPECTED_YEARS)
        and national_years == set(EXPECTED_YEARS),
        "state and national ERP both cover the fixed years 2010-2025",
    )
    if not coverage_ok:
        return
    expected = len(years) * len(STATE_CODES) * len(EXPECTED_SEXES) * (TERMINAL_AGE + 1)
    report.check(len(state) == expected,
                 f"state cells complete: {len(state)} of {expected} expected")
    report.check(all(v >= 0 for v in state.values()), "no negative state counts")
    report.check(all(v >= 0 for v in national.values()), "no negative national counts")

    missing = [
        (y, s, sex, age)
        for y in years for s in STATE_CODES
        for sex in EXPECTED_SEXES for age in range(TERMINAL_AGE + 1)
        if (y, s, sex, age) not in state
    ]
    report.check(not missing, f"no missing (year, state, sex, age) cells "
                              f"[{len(missing)} missing]")
    report.say()

    report.say("## Regional 2010 cross-check")
    report.say()
    _, regional_rows = read_csv(
        EXTRACTS / "erp_regional_2010_state_age_sex.csv"
    )
    regional = {
        (code, sex, age_band): int(persons)
        for _year, code, sex, age_band, persons in regional_rows
    }
    state_bands = collections.Counter()
    for (year, code, sex, age), persons in state.items():
        if year != 2010:
            continue
        age_band = "85+" if age >= 85 else f"{age // 5 * 5}-{age // 5 * 5 + 4}"
        state_bands[(code, sex, age_band)] += persons
    report.check(
        regional == dict(state_bands),
        "2010 SA2 sums reproduce every state-sex-age-band ERP cell exactly",
    )
    report.say()
    report.say("The regional workbook is an independently structured SA2 "
               "back-series on 2021 ASGS boundaries. Exact agreement after "
               "summing 2,454 SA2 rows verifies both the 2010 state extraction "
               "and the exclusion of Other Territories; no regional values are "
               "used to alter the state ERP.")
    report.say()

    report.say("## State totals against the published national series")
    report.say()
    report.say("The national series covers Australia, which is the eight states and "
               "territories **plus Other Territories** (Christmas Island, the Cocos "
               "(Keeling) Islands and Jervis Bay Territory, joined by Norfolk Island "
               "from 2016). The model's geography is the eight states only "
               "(DECISIONS.md N1), so the national total is expected to exceed the "
               "sum of states by the Other Territories population. The invariant "
               "tested is therefore that the residual is non-negative and small, not "
               "that it is zero.")
    report.say()

    residual_by_year = {}
    for y in years:
        summed = sum(v for (yy, _s, _x, _a), v in state.items() if yy == y)
        published = sum(v for (yy, _x, _a), v in national.items() if yy == y)
        residual_by_year[y] = published - summed

    report.check(all(r >= 0 for r in residual_by_year.values()),
                 "national total is never below the sum of the eight states")
    worst_share = max(
        residual_by_year[y] / sum(v for (yy, _x, _a), v in national.items() if yy == y)
        for y in years
    )
    report.check(worst_share < 0.0005,
                 f"Other Territories residual stays below 0.05% of national "
                 f"[worst {worst_share:.4%}]")

    negative_cells = [
        (y, sex, age)
        for y in years for sex in EXPECTED_SEXES for age in range(TERMINAL_AGE + 1)
        if (y, sex, age) in national
        and national[(y, sex, age)] - sum(state[(y, s, sex, age)] for s in STATE_CODES) < 0
    ]
    report.check(not negative_cells,
                 f"no cell where the states exceed the national figure "
                 f"[{len(negative_cells)} such cells]")
    report.say()

    report.say("Implied Other Territories population by year:")
    report.say()
    report.say("| year | national | eight states | Other Territories |")
    report.say("|---|---:|---:|---:|")
    for y in years:
        summed = sum(v for (yy, _s, _x, _a), v in state.items() if yy == y)
        published = sum(v for (yy, _x, _a), v in national.items() if yy == y)
        report.say(f"| {y} | {published:,} | {summed:,} | {residual_by_year[y]:,} |")
    report.say()

    step = residual_by_year.get(2016, 0) - residual_by_year.get(2015, 0)
    report.say(f"The {step:+,} step between 2015 and 2016 is Norfolk Island entering "
               "the estimated resident population, which independently corroborates "
               "that this residual is Other Territories rather than a parsing error.")
    report.say()
    report.say("**Scope consequence.** The model does not represent Other "
               "Territories, so its national total is the sum of the eight states "
               "and is below published Australian ERP by the amounts above. Any "
               "comparison against a national figure must use the eight-state sum.")
    report.say()

    report.say("## Annual population by state")
    report.say()
    report.say("| year | " + " | ".join(STATE_CODES) + " | total |")
    report.say("|---" * (len(STATE_CODES) + 2) + "|")
    for y in years:
        totals = [
            sum(state[(y, s, sex, age)]
                for sex in EXPECTED_SEXES for age in range(TERMINAL_AGE + 1))
            for s in STATE_CODES
        ]
        report.say(f"| {y} | " + " | ".join(f"{t:,}" for t in totals)
                   + f" | {sum(totals):,} |")
    report.say()

    report.say("## Year-on-year national change")
    report.say()
    report.say("| year | population | change | growth |")
    report.say("|---|---:|---:|---:|")
    previous = None
    for y in years:
        total = sum(v for (yy, _s, _x, _a), v in state.items() if yy == y)
        if previous is None:
            report.say(f"| {y} | {total:,} | - | - |")
        else:
            delta = total - previous
            report.say(f"| {y} | {total:,} | {delta:+,} | {delta / previous:+.2%} |")
        previous = total
    report.say()

    reconcile_components(report, state, years)


def reconcile_components(report: Report, state: dict, years: list[int]) -> None:
    """Check the stock-flow identity and the interstate margins."""
    _, comp_rows = read_csv(EXTRACTS / "components_state.csv")
    _, margin_rows = read_csv(EXTRACTS / "interstate_margins.csv")
    components = {
        (int(y), s): (int(ni), int(nom), int(nim))
        for y, s, ni, nom, nim in comp_rows
    }
    margins = {
        (int(y), s): (int(a), int(d)) for y, s, a, d in margin_rows
    }
    run_years = list(EXPECTED_RUN_YEARS)
    component_years = {key[0] for key in components}
    margin_years = {key[0] for key in margins}

    report.say("## Stock-flow identity")
    report.say()
    flow_coverage_ok = report.check(
        component_years == set(EXPECTED_RUN_YEARS)
        and margin_years == set(EXPECTED_RUN_YEARS),
        "components and interstate margins both cover fixed run years "
        "2010-2024",
    )
    if not flow_coverage_ok:
        return
    report.say()
    report.say("For each state and run year, the change in estimated resident "
               "population from 30 June to 30 June is compared against the "
               "published components of that change:")
    report.say()
    report.say("```text")
    report.say("ERP(y+1) - ERP(y) = natural increase + net overseas migration")
    report.say("                    + net interstate migration + intercensal discrepancy")
    report.say("```")
    report.say()
    report.say("The residual is the intercensal discrepancy and rebasing. It is "
               "reported, never forced to zero (DECISIONS.md N13). Expect it to "
               "vanish for run years after the most recent Census rebase, where "
               "estimates are carried forward from the components themselves, and "
               "to be nonzero before it, where the discrepancy between successive "
               "Census bases has been distributed across the intercensal years.")
    report.say()

    residuals = {}
    for y in run_years:
        if y + 1 not in years:
            continue
        for code in STATE_CODES:
            if (y, code) not in components:
                continue
            start = sum(v for (yy, s, _x, _a), v in state.items()
                        if yy == y and s == code)
            end = sum(v for (yy, s, _x, _a), v in state.items()
                      if yy == y + 1 and s == code)
            ni, nom, nim = components[(y, code)]
            residuals[(y, code)] = (end - start) - (ni + nom + nim)

    report.check(
        len(residuals) == len(run_years) * len(STATE_CODES),
        "stock-flow identity evaluated for all 120 state-year cells",
    )
    if residuals:
        worst = max(abs(v) for v in residuals.values())
        national_2010 = sum(v for (yy, _s, _x, _a), v in state.items()
                            if yy == min(years))
        report.check(worst < 0.01 * national_2010,
                     f"largest state-year residual stays well below 1% of national "
                     f"population [worst {worst:,}]")
        report.say()
        report.say("Residual by state and run year:")
        report.say()
        report.say("| run year | " + " | ".join(STATE_CODES) + " |")
        report.say("|---|" + "---:|" * len(STATE_CODES))
        for year in run_years:
            report.say(
                f"| {year} | "
                + " | ".join(f"{residuals[(year, code)]:+,}"
                             for code in STATE_CODES)
                + " |"
            )
        report.say()
        report.say("Residual by run year, summed over states (absolute):")
        report.say()
        report.say("| run year | signed residual | absolute residual |")
        report.say("|---|---:|---:|")
        for y in run_years:
            vals = [residuals[(y, c)] for c in STATE_CODES]
            report.say(f"| {y} | {sum(vals):+,} | {sum(abs(v) for v in vals):,} |")
        report.say()

    report.say("## Interstate migration consistency")
    report.say()
    _, od_rows = read_csv(EXTRACTS / "interstate_flows.csv")
    od_arrivals = collections.Counter()
    od_departures = collections.Counter()
    od_keys = set()
    for year, origin, destination, persons in od_rows:
        key = (int(year), origin, destination)
        od_keys.add(key)
        value = int(persons)
        od_departures[(int(year), origin)] += value
        od_arrivals[(int(year), destination)] += value
    expected_od = {
        (year, origin, destination)
        for year in run_years
        for origin in STATE_CODES
        for destination in STATE_CODES
        if origin != destination
    }
    report.check(
        od_keys == expected_od,
        "origin-destination flows cover all 56 directed cells in every run year",
    )
    report.check(
        all(int(row[-1]) >= 0 and row[1] != row[2] for row in od_rows),
        "origin-destination flows are non-negative and exclude diagonal cells",
    )
    od_margin_diffs = {}
    for key, (published_arrivals, published_departures) in margins.items():
        od_margin_diffs[key] = (
            od_arrivals[key] - published_arrivals,
            od_departures[key] - published_departures,
        )
    worst_od_by_year = {
        year: max(
            abs(value)
            for key, pair in od_margin_diffs.items() if key[0] == year
            for value in pair
        )
        for year in run_years
    }
    exact_years = set(range(2016, 2025)) - {2020}
    report.check(
        all(worst_od_by_year[year] == 0 for year in exact_years),
        "O-D-derived margins exactly match the separate margin workbooks in "
        "2016-2019 and 2021-2024",
    )
    report.check(
        max(worst_od_by_year[year] for year in range(2010, 2016)) <= 500,
        "pre-2016 O-D margins stay within 500 persons of the revised margin "
        "workbooks",
    )
    report.say()
    report.say("| run year | worst O-D versus margin difference |")
    report.say("|---|---:|")
    for year in run_years:
        report.say(f"| {year} | {worst_od_by_year[year]:,} |")
    report.say()
    report.say("The 2020 difference (worst 27,626) is real published-vintage "
               "evidence, not a parsing residual: the quarterly O-D dataflow and "
               "the separately revised ERP margin workbooks diverge during the "
               "COVID interstate-migration shock. Both are retained. Calibration "
               "must not force the O-D cells to those incompatible margins.")
    report.say()

    _, nim_detail_rows = read_csv(
        EXTRACTS / "interstate_state_age_sex.csv"
    )
    nim_detail = collections.defaultdict(lambda: [0, 0])
    for year, code, _sex, _age, arrivals, departures in nim_detail_rows:
        values = nim_detail[(int(year), code)]
        values[0] += int(arrivals)
        values[1] += int(departures)
    nim_detail_diffs = {
        key: (values[0] - margins[key][0], values[1] - margins[key][1])
        for key, values in nim_detail.items()
        if key in margins
    }
    worst_nim_detail = max(
        (abs(value) for pair in nim_detail_diffs.values() for value in pair),
        default=0,
    )
    report.check(
        len(nim_detail_diffs) == len(run_years) * len(STATE_CODES),
        "interstate age-sex detail compared with every state-year margin",
    )
    report.check(
        worst_nim_detail <= 100,
        "interstate age-sex detail sums to separately rounded margins within "
        f"100 persons [worst {worst_nim_detail}]",
    )
    report.say()
    report.say("The age-sex interstate margins cover the whole window and are "
               "independently rounded by cell. They provide age-profile evidence "
               "without pretending that age is observed jointly with O-D.")
    report.say()

    net_ok, margin_ok = [], []
    for y in run_years:
        national_net = sum(components[(y, c)][2] for c in STATE_CODES
                           if (y, c) in components)
        net_ok.append((y, national_net))
        for code in STATE_CODES:
            if (y, code) in margins and (y, code) in components:
                arrivals, departures = margins[(y, code)]
                margin_ok.append(
                    abs((arrivals - departures) - components[(y, code)][2])
                )
    # The eight states do not close exactly, because Other Territories also
    # exchange population with the states and are not published as a series in
    # this table. The residual is therefore small but not zero.
    other_territories_tolerance = 200
    worst_net = max(abs(v) for _y, v in net_ok)
    report.check(worst_net <= other_territories_tolerance,
                 f"net interstate migration across the eight states closes to within "
                 f"{other_territories_tolerance} persons a year [worst {worst_net}]")
    report.check(all(v <= 1 for v in margin_ok),
                 "arrivals minus departures equals net interstate migration for "
                 f"every state-year [worst {max(margin_ok) if margin_ok else 0}]")
    report.say()
    report.say("Every interstate move is one region's arrival and another's "
               "departure, so net interstate migration sums to zero across *all* "
               "regions. It does not sum to exactly zero across the eight states "
               "alone, because Other Territories also exchange population with "
               "them and are not published as a separate series in this table. "
               "The residual below is that exchange plus rounding.")
    report.say()
    report.say("| run year | net interstate migration, eight states |")
    report.say("|---|---:|")
    for y, v in net_ok:
        report.say(f"| {y} | {v:+,} |")
    report.say()
    report.say("This is the published counterpart of the exact-conservation "
               "invariant the model asserts under N1. The model is closed over "
               "the eight states, so its own interstate flows must cancel "
               "exactly; the small published residual is the price of Other "
               "Territories being out of scope.")
    report.say()

    report.say("## Overseas migration margins")
    report.say()
    report.say("Arrivals and departures come from the Overseas Migration release, "
               "while net overseas migration comes from the ERP components table. "
               "They are independently published and independently rounded, so "
               "arrivals minus departures should track published net overseas "
               "migration closely without matching exactly.")
    report.say()
    _, overseas_rows = read_csv(EXTRACTS / "overseas_margins.csv")
    net_published = {(int(r[0]), r[1]): int(r[3]) for r in comp_rows}
    diffs = {}
    for run_year, code, arrivals, departures in overseas_rows:
        key = (int(run_year), code)
        if key in net_published:
            diffs[key] = (int(arrivals) - int(departures)) - net_published[key]

    settled = {k: v for k, v in diffs.items() if k[0] <= 2022}
    report.check(bool(diffs), "overseas margins compared against published net")
    if settled:
        worst_settled = max(abs(v) for v in settled.values())
        report.check(worst_settled < 1000,
                     f"for settled run years to 2022, implied net overseas "
                     f"migration matches the published figure to within 1,000 "
                     f"persons [worst {worst_settled:,}]")
    report.say()
    report.say("| run year | worst absolute difference across states |")
    report.say("|---|---:|")
    for year in sorted({k[0] for k in diffs}):
        worst = max(abs(v) for k, v in diffs.items() if k[0] == year)
        report.say(f"| {year} | {worst:,} |")
    report.say()
    report.say("The final run years diverge more because the most recent "
               "financial year is preliminary in both releases and is revised on "
               "different schedules. Treat those years as provisional rather than "
               "settled evidence.")
    report.say()

    _, detailed_rows = read_csv(EXTRACTS / "nom_state_age_sex.csv")
    detailed = collections.defaultdict(lambda: [0, 0])
    for year, code, _sex, _age, arrivals, departures in detailed_rows:
        values = detailed[(int(year), code)]
        values[0] += int(arrivals)
        values[1] += int(departures)
    published_gross = {
        (int(year), code): (int(arrivals), int(departures))
        for year, code, arrivals, departures in overseas_rows
    }
    detail_diffs = {
        key: (values[0] - published_gross[key][0],
              values[1] - published_gross[key][1])
        for key, values in detailed.items()
        if key in published_gross
    }
    worst_detail = max(
        (abs(value) for pair in detail_diffs.values() for value in pair),
        default=0,
    )
    report.check(
        len(detail_diffs) == len(run_years) * len(STATE_CODES),
        "age-sex overseas detail compared with every state-year margin",
    )
    report.check(
        worst_detail <= 100,
        "age-sex overseas detail sums to independently extracted gross margins "
        f"within cell-rounding tolerance [worst {worst_detail}]",
    )
    report.say()
    report.say("The detailed SDMX cells and the workbook margins are separate "
               "physical sources from the same release. Each detailed cell is "
               "rounded to the nearest 10, so summing 28 age-sex cells need not "
               "equal the separately rounded margin; the observed worst "
               f"difference is {worst_detail} persons.")
    report.say()

    report.say("## Birth and death registration data")
    report.say()
    _, birth_rows = read_csv(EXTRACTS / "births_state.csv")
    _, birth_sex_rows = read_csv(EXTRACTS / "births_sex.csv")
    _, death_rows = read_csv(EXTRACTS / "deaths_state_age_sex.csv")
    birth_years = {int(row[0]) for row in birth_rows}
    birth_sex_years = {int(row[0]) for row in birth_sex_rows}
    death_years = {int(row[0]) for row in death_rows}
    event_years = set(run_years)
    report.check(
        birth_years == event_years,
        "registered births cover calendar years 2010-2024",
    )
    report.check(
        birth_sex_years == event_years
        and {(row[0], row[1]) for row in birth_sex_rows}
        == {(str(year), sex) for year in event_years
            for sex in ("female", "male")},
        "national male and female births cover calendar years 2010-2024",
    )
    report.check(
        death_years == event_years,
        "registered deaths by age and sex cover calendar years 2010-2024",
    )
    report.check(
        all(int(row[-1]) >= 0
            for row in birth_rows + birth_sex_rows + death_rows),
        "birth and death extracts contain no negative counts",
    )
    report.say()
    births_by_year = collections.Counter()
    births_by_year_sex = collections.Counter()
    deaths_by_year = collections.Counter()
    unstated_age = collections.Counter()
    for year, _code, births in birth_rows:
        births_by_year[int(year)] += int(births)
    for year, sex, births in birth_sex_rows:
        births_by_year_sex[(int(year), sex)] += int(births)
    national_births = {
        year: sum(births_by_year_sex[(year, sex)]
                  for sex in ("female", "male"))
        for year in event_years
    }
    birth_residuals = {
        year: national_births[year] - births_by_year[year]
        for year in event_years
    }
    report.check(
        min(birth_residuals.values()) >= 0
        and max(birth_residuals.values()) <= 50,
        "national sex totals exceed the eight-state birth sum only by the "
        "published residual [worst "
        f"{max(birth_residuals.values()):,}]",
    )
    for year, _code, _sex, age_band, deaths in death_rows:
        deaths_by_year[int(year)] += int(deaths)
        if age_band == "not_stated":
            unstated_age[int(year)] += int(deaths)
    report.say("| calendar year | births, eight states | male births, Australia | "
               "female births, Australia | national residual | deaths, eight "
               "states | deaths with age not stated |")
    report.say("|---|---:|---:|---:|---:|---:|---:|")
    for year in sorted(event_years):
        report.say(
            f"| {year} | {births_by_year[year]:,} | "
            f"{births_by_year_sex[(year, 'male')]:,} | "
            f"{births_by_year_sex[(year, 'female')]:,} | "
            f"{birth_residuals[year]:,} | {deaths_by_year[year]:,} | "
            f"{unstated_age[year]:,} |"
        )
    report.say()
    report.say("These are final **calendar registration-year** counts from Births, "
               "Australia and Deaths, Australia. The national male/female series "
               "sets the entrant sex ratio; the eight-state residual is retained. "
               "These counts are direct evidence, but they are not the same "
               "temporal quantity as the financial-year natural-increase "
               "component. The pipeline preserves both and does not force one to "
               "equal the other.")
    report.say()

    report.say("## Annual age-specific mortality rates")
    report.say()
    _, mortality_rows = read_csv(
        EXTRACTS / "mortality_rates_state_age_sex.csv"
    )
    published_rates = [row for row in mortality_rows if row[-1] == "published"]
    missing_rates = [row for row in mortality_rows if row[-1] != "published"]
    report.check(
        len(mortality_rows) == 5040,
        "mortality-rate extract carries the full 2010-2024 state-sex-band grid",
    )
    report.check(
        all(float(row[-2]) >= 0 for row in published_rates),
        "all published mortality rates are non-negative",
    )
    observed_missing = {
        (int(row[0]), row[1], row[2], row[3]) for row in missing_rates
    }
    expected_missing = {
        (2010, "nt", "female", "100+"),
        (2011, "nt", "female", "100+"),
    }
    report.check(
        observed_missing == expected_missing,
        "the only unpublished rates are NT female 100+ in 2010 and 2011 "
        "(zero published exposure)",
    )
    report.say()
    report.say("Rates are ABS registered deaths per 1,000 population in "
               "non-overlapping five-year age bands. They are the direct annual "
               "mortality input selected in DECISIONS.md §N6. The two absent "
               "terminal-band cells remain blank and explicitly flagged; PRD "
               "0005 uses the same-year Australian female 100+ rate rather than "
               "treating a missing rate as zero.")
    report.say()

    report.say("## Life-table validation snapshots")
    report.say()
    _, life_rows = read_csv(EXTRACTS / "life_tables_state_age_sex.csv")
    life_periods = {(int(row[0]), int(row[1])) for row in life_rows}
    report.check(
        life_periods == {(year, year + 2) for year in range(2018, 2023)},
        "life-table validation covers the five published XLSX periods from "
        "2018-2020 through 2022-2024",
    )
    report.check(
        len(life_rows) == 8080,
        "each life-table period has all eight states, both sexes and ages 0-100",
    )
    report.check(
        all(0.0 <= float(row[-1]) <= 1.0 for row in life_rows),
        "all published qx values lie in [0,1]",
    )
    report.say()
    report.say("These are overlapping three-year **period** life tables. Their "
               "period boundaries are retained in the extract; no snapshot is "
               "relabelled as an annual observation. Under §N6 they validate the "
               "level and age shape of the independently published annual death "
               "rates but do not supply model parameters.")
    report.say()

    report.say("## Remaining coverage limitation")
    report.say()
    report.say("Recorded rather than worked around; see `sources.json`:")
    report.say()
    report.say("- **Birth/death temporal basis.** Direct state birth counts and "
               "state-age-sex death counts now cover 2010-2024, but they are final "
               "calendar registration-year observations. ERP natural increase is "
               "a financial-year component based on occurrence and demographic "
               "adjustment. Applying the registration series to model run years "
               "therefore needs an explicit, documented convention in PRD 0005; "
               "this acquisition PRD does not shift or scale the observations.")
    report.say()


def main(argv: list[str] | None = None) -> int:
    report = Report()
    reconcile(report)
    output = EXTRACTS / "reconciliation.md"
    output.write_text("\n".join(report.lines) + "\n", encoding="utf-8", newline="")
    print(f"wrote {output.relative_to(HERE)}")
    for failure in report.failures:
        print(f"FAIL: {failure}", file=sys.stderr)
    return 1 if report.failures else 0


if __name__ == "__main__":
    raise SystemExit(main())

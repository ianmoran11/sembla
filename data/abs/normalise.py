"""Normalise cached ABS workbooks into canonical committed extracts.

Reads only the local cache, never the network. Every output regenerates byte
for byte from the same cached inputs, which is what makes each downstream build
offline and reproducible.
"""

from __future__ import annotations

import pathlib
import re
import sys

import absts
import fetch
import sdmx_csv
from canonical import write_csv
from xlsx import Workbook

HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE / "extracts"

# The eight states and territories, in the order the model's `area` enum
# declares them (DECISIONS.md N1).
STATE_CODES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")

# `100 and over` is the terminal open age group and is carried as age 100.
TERMINAL_AGE = 100

_ERP_SERIES = re.compile(
    r"^Estimated Resident Population ;\s*(Male|Female|Persons) ;\s*(.+?)\s*;$"
)

_FLOW_SERIES = re.compile(r"^(.+?) ;\s*(.+?)\s*;$")

# ABS spells regions in full; the model's enum uses these codes (N1).
STATE_NAMES = {
    "New South Wales": "nsw",
    "Victoria": "vic",
    "Queensland": "qld",
    "South Australia": "sa",
    "Western Australia": "wa",
    "Tasmania": "tas",
    "Northern Territory": "nt",
    "Australian Capital Territory": "act",
}


def run_year_of(quarter_end) -> int:
    """Map a quarter-end date to the model run year that contains it.

    A run year covers 1 July to 30 June, so the September, December, March and
    June quarters that follow 30 June of year Y all belong to run year Y. The
    resulting flow total lines up with the stock change from ERP(Y) to ERP(Y+1).
    """
    return quarter_end.year if quarter_end.month > 6 else quarter_end.year - 1


def _accumulate(path, wanted, first_run_year, last_run_year, scale=1.0):
    """Sum quarterly FLOW series into run-year totals.

    Returns ``{(run_year, region): {measure: total}}``. A run year is only kept
    when all four of its quarters are present, so a partial year at either end
    of the series is dropped rather than silently under-counted.
    """
    totals: dict[tuple[int, str], dict[str, float]] = {}
    counts: dict[tuple[int, str, str], int] = {}
    for series in absts.read_series(path):
        match = _FLOW_SERIES.match(series.descriptor)
        if match is None:
            continue
        measure, region = match.group(1).strip(), match.group(2).strip()
        if measure not in wanted:
            continue
        code = STATE_NAMES.get(region, "aus" if region == "Australia" else None)
        if code is None:
            continue
        for when, value in series.observations.items():
            year = run_year_of(when)
            if not first_run_year <= year <= last_run_year:
                continue
            key = (year, code)
            totals.setdefault(key, {}).setdefault(wanted[measure], 0.0)
            totals[key][wanted[measure]] += value * scale
            counts[(year, code, wanted[measure])] = (
                counts.get((year, code, wanted[measure]), 0) + 1
            )
    complete = {
        key: values
        for key, values in totals.items()
        if all(counts[(key[0], key[1], m)] == 4 for m in values)
    }
    return complete


def parse_age(label: str) -> int | None:
    label = label.strip()
    if label.isdigit():
        return int(label)
    if label.lower() == f"{TERMINAL_AGE} and over":
        return TERMINAL_AGE
    return None


def erp_rows(region: str, path: pathlib.Path, first_year: int, last_year: int):
    """Yield ``(year, region, sex, age, persons)`` for one ERP workbook."""
    for series in absts.read_series(path):
        match = _ERP_SERIES.match(series.descriptor)
        if match is None:
            continue
        sex_label, age_label = match.group(1), match.group(2)
        # `Persons` is the sum of the two sex series and is recomputed during
        # reconciliation rather than carried as data.
        if sex_label == "Persons":
            continue
        age = parse_age(age_label)
        if age is None:
            raise ValueError(f"{path.name}: unrecognised age label {age_label!r}")
        sex = sex_label.lower()
        for when, value in series.observations.items():
            # ERP in this cube is an annual series referenced to 30 June.
            if when.month != 6 or not first_year <= when.year <= last_year:
                continue
            count = int(round(value))
            if count < 0:
                raise ValueError(
                    f"{path.name}: negative count {value} at {when} age {age}"
                )
            yield (when.year, region, sex, age, count)


def build_erp(sources: dict, first_year: int, last_year: int) -> dict[str, int]:
    written = {}

    state_rows = []
    for code in STATE_CODES:
        entry = sources[f"erp_{code}"]
        path = fetch.CACHE / entry["filename"]
        state_rows.extend(erp_rows(code, path, first_year, last_year))
    written["erp_state_age_sex.csv"] = write_csv(
        EXTRACTS / "erp_state_age_sex.csv",
        ["year", "state", "sex", "age", "persons"],
        state_rows,
    )

    national = list(
        erp_rows("aus", fetch.CACHE / sources["erp_aus"]["filename"],
                 first_year, last_year)
    )
    written["erp_national_age_sex.csv"] = write_csv(
        EXTRACTS / "erp_national_age_sex.csv",
        ["year", "region", "sex", "age", "persons"],
        national,
    )
    return written


def build_flows(sources: dict, first_run_year: int, last_run_year: int) -> dict[str, int]:
    written = {}

    components = _accumulate(
        fetch.CACHE / sources["components_state"]["filename"],
        {
            "Natural Increase": "natural_increase",
            "Net Overseas Migration": "net_overseas_migration",
            "Net Interstate Migration": "net_interstate_migration",
        },
        first_run_year,
        last_run_year,
    )
    rows = [
        (year, code, int(round(v["natural_increase"])),
         int(round(v["net_overseas_migration"])),
         int(round(v["net_interstate_migration"])))
        for (year, code), v in components.items()
        if code in STATE_CODES and len(v) == 3
    ]
    written["components_state.csv"] = write_csv(
        EXTRACTS / "components_state.csv",
        ["run_year", "state", "natural_increase", "net_overseas_migration",
         "net_interstate_migration"],
        rows,
    )

    arrivals = _accumulate(
        fetch.CACHE / sources["interstate_arrivals"]["filename"],
        {"Interstate Arrivals": "arrivals"}, first_run_year, last_run_year,
    )
    departures = _accumulate(
        fetch.CACHE / sources["interstate_departures"]["filename"],
        {"Interstate Departures": "departures"}, first_run_year, last_run_year,
    )
    margin_rows = [
        (year, code, int(round(arrivals[(year, code)]["arrivals"])),
         int(round(departures[(year, code)]["departures"])))
        for (year, code) in sorted(set(arrivals) & set(departures))
        if code in STATE_CODES
    ]
    written["interstate_margins.csv"] = write_csv(
        EXTRACTS / "interstate_margins.csv",
        ["run_year", "state", "arrivals", "departures"],
        margin_rows,
    )
    od_entry = sources["interstate_od"]
    od_path = fetch.CACHE / od_entry["filename"]
    od_totals = {}
    od_quarters = {}
    seen_od = set()
    for row in sdmx_csv.read(
        od_path,
        od_entry["dataflow"],
        {"DATAFLOW", "SEX", "STATE_DEP", "STATE_ARR", "FREQ",
         "TIME_PERIOD", "OBS_VALUE", "UNIT_MEASURE"},
        frequency="Q",
        unit_measure="PSNS",
    ):
        if row["SEX"] != "3":
            raise ValueError(f"{od_path.name}: expected persons sex code 3")
        try:
            origin = sdmx_csv.STATE_BY_REGION[row["STATE_DEP"]]
            destination = sdmx_csv.STATE_BY_REGION[row["STATE_ARR"]]
        except KeyError as error:
            raise ValueError(
                f"{od_path.name}: unknown state code {error.args[0]!r}"
            ) from error
        match = re.fullmatch(r"(\d{4})-Q([1-4])", row["TIME_PERIOD"])
        if match is None:
            raise ValueError(
                f"{od_path.name}: invalid quarter {row['TIME_PERIOD']!r}"
            )
        calendar_year, quarter = int(match.group(1)), int(match.group(2))
        run_year = calendar_year if quarter >= 3 else calendar_year - 1
        value = sdmx_csv.count(row, od_path.name)
        observation = (row["TIME_PERIOD"], origin, destination)
        if observation in seen_od:
            raise ValueError(f"{od_path.name}: duplicate {observation!r}")
        seen_od.add(observation)
        if origin == destination:
            if value != 0:
                raise ValueError(
                    f"{od_path.name}: non-zero interstate diagonal "
                    f"{observation!r}"
                )
            continue
        if not first_run_year <= run_year <= last_run_year:
            continue
        key = (run_year, origin, destination)
        od_totals[key] = od_totals.get(key, 0) + value
        od_quarters[key] = od_quarters.get(key, 0) + 1
    expected_od = {
        (year, origin, destination)
        for year in range(first_run_year, last_run_year + 1)
        for origin in STATE_CODES
        for destination in STATE_CODES
        if origin != destination
    }
    _require_complete("interstate_flows", set(od_totals), expected_od)
    incomplete_od = [key for key, count in od_quarters.items() if count != 4]
    if incomplete_od:
        raise ValueError(
            f"interstate_flows: incomplete quarters at {min(incomplete_od)!r}"
        )
    written["interstate_flows.csv"] = write_csv(
        EXTRACTS / "interstate_flows.csv",
        ["year", "origin", "destination", "persons"],
        ((*key, value) for key, value in od_totals.items()),
    )

    # Table 1 is published in thousands, so it is scaled to persons here and
    # its rounding is recorded in the reconciliation report.
    national = _accumulate(
        fetch.CACHE / sources["components_national"]["filename"],
        {
            "Births": "births",
            "Deaths": "deaths",
            "Overseas Arrivals": "overseas_arrivals",
            "Overseas Departures": "overseas_departures",
        },
        first_run_year,
        last_run_year,
        scale=1000.0,
    )
    national_rows = [
        (year, int(round(v["births"])), int(round(v["deaths"])),
         int(round(v["overseas_arrivals"])), int(round(v["overseas_departures"])))
        for (year, code), v in national.items()
        if code == "aus" and len(v) == 4
    ]
    written["components_national.csv"] = write_csv(
        EXTRACTS / "components_national.csv",
        ["run_year", "births", "deaths", "overseas_arrivals", "overseas_departures"],
        national_rows,
    )
    return written


# The Overseas Migration cubes are ordinary data cubes rather than time-series
# workbooks: one sheet per region, countries of birth down the rows, financial
# years across the columns, and a `Total` row. Sheet suffix 1 is Australia and
# 2 to 9 are the states in the order ABS lists them.
_OVERSEAS_SHEET_REGION = {
    1: "aus", 2: "nsw", 3: "vic", 4: "qld", 5: "sa",
    6: "wa", 7: "tas", 8: "nt", 9: "act",
}

_FINANCIAL_YEAR = re.compile(r"^(\d{4})-\d{2}")


def cube_totals_by_run_year(path, sheet: str) -> dict[int, int]:
    """Read a cube sheet's `Total` row, keyed by the run year it starts in.

    Financial year 2010-11 is run year 2010, matching N7's 1 July to 30 June
    run. Footnote markers such as `2024-25(e)` are tolerated.
    """
    with Workbook(path) as book:
        cells = book.cells(sheet)
    header_rows = {
        row for (col, row), value in cells.items()
        if col == 1 and isinstance(value, str) and value.startswith("SACC code")
    }
    if not header_rows:
        raise ValueError(f"{path}:{sheet}: no SACC code header row")
    header_row = min(header_rows)
    years = {}
    for (col, row), value in cells.items():
        if row != header_row or col < 3:
            continue
        match = _FINANCIAL_YEAR.match(str(value))
        if match:
            years[col] = int(match.group(1))

    total_rows = [
        row for (col, row), value in cells.items()
        if col == 2 and isinstance(value, str) and value.strip() == "Total"
    ]
    if len(total_rows) != 1:
        raise ValueError(f"{path}:{sheet}: expected exactly one Total row, "
                         f"found {len(total_rows)}")
    total_row = total_rows[0]

    out = {}
    for col, year in years.items():
        raw = cells.get((col, total_row))
        if raw in (None, ""):
            continue
        out[year] = int(round(float(raw)))
    return out


def build_overseas(sources: dict, first_run_year: int, last_run_year: int) -> dict[str, int]:
    arrivals, departures = {}, {}
    for key, sink in (("overseas_arrivals", arrivals),
                      ("overseas_departures", departures)):
        path = fetch.CACHE / sources[key]["filename"]
        prefix = sources[key]["sheet_prefix"]
        for suffix, region in _OVERSEAS_SHEET_REGION.items():
            if region not in STATE_CODES:
                continue
            for year, value in cube_totals_by_run_year(
                path, f"{prefix}.{suffix}"
            ).items():
                if first_run_year <= year <= last_run_year:
                    sink[(year, region)] = value

    rows = [
        (year, region, arrivals[(year, region)], departures[(year, region)])
        for (year, region) in sorted(set(arrivals) & set(departures))
    ]
    return {
        "overseas_margins.csv": write_csv(
            EXTRACTS / "overseas_margins.csv",
            ["run_year", "state", "arrivals", "departures"],
            rows,
        )
    }


_LIFE_TABLE_STATE = {
    1: "nsw", 2: "vic", 3: "qld", 4: "sa",
    5: "wa", 6: "tas", 7: "nt", 8: "act",
}


def life_table_rows(entry: dict):
    """Yield one published period life table's state/sex/single-age qx rows."""
    path = fetch.CACHE / entry["filename"]
    period_start = int(entry["period_start"])
    period_end = int(entry["period_end"])
    with Workbook(path) as book:
        for suffix, state in _LIFE_TABLE_STATE.items():
            candidates = (f"Table {suffix}", f"Table_1.{suffix}")
            names = [name for name in candidates if name in book.sheet_names]
            if len(names) != 1:
                raise ValueError(
                    f"{path.name}: expected one sheet for table {suffix}, "
                    f"found {names!r}"
                )
            cells = book.cells(names[0])
            age_headers = [
                row for (col, row), value in cells.items()
                if col == 1 and value == "Age"
            ]
            if len(age_headers) != 1:
                raise ValueError(f"{path.name}:{names[0]}: missing Age header")
            if cells.get((3, 6)) != "qx" or cells.get((7, 6)) != "qx":
                raise ValueError(f"{path.name}:{names[0]}: missing qx headers")
            age_rows = {
                int(value): row
                for (col, row), value in cells.items()
                if col == 1 and str(value).isdigit()
                and 0 <= int(value) <= TERMINAL_AGE
            }
            expected_ages = set(range(TERMINAL_AGE + 1))
            if set(age_rows) != expected_ages:
                raise ValueError(
                    f"{path.name}:{names[0]}: life-table ages are incomplete"
                )
            for sex, column in (("male", 3), ("female", 7)):
                for age in expected_ages:
                    raw = cells.get((column, age_rows[age]))
                    try:
                        qx = float(raw)
                    except (TypeError, ValueError) as error:
                        raise ValueError(
                            f"{path.name}:{names[0]}: invalid qx at "
                            f"{sex} age {age}: {raw!r}"
                        ) from error
                    if not 0.0 <= qx <= 1.0:
                        raise ValueError(
                            f"{path.name}:{names[0]}: qx outside [0,1] at "
                            f"{sex} age {age}: {qx}"
                        )
                    yield (period_start, period_end, state, sex, age, qx)


def build_life_tables(sources: dict) -> dict[str, int]:
    rows = []
    entries = [
        entry for key, entry in sources.items()
        if key.startswith("life_tables_")
    ]
    entries.sort(key=lambda entry: (entry["period_start"], entry["period_end"]))
    for entry in entries:
        rows.extend(life_table_rows(entry))
    return {
        "life_tables_state_age_sex.csv": write_csv(
            EXTRACTS / "life_tables_state_age_sex.csv",
            ["period_start", "period_end", "state", "sex", "age", "qx"],
            rows,
        )
    }


def build_regional_crosscheck(sources: dict, year: int = 2010) -> dict[str, int]:
    """Aggregate the SA2 back-series to state/sex/age for a 2010 cross-check."""
    entry = sources["erp_regional_age_sex"]
    path = fetch.CACHE / entry["filename"]
    totals = {}
    sa2_codes = {}
    with Workbook(path) as book:
        for sheet, sex in (("Table 1", "male"), ("Table 2", "female")):
            age_columns = None
            seen = set()
            for row_number, values in book.iter_rows(sheet):
                if row_number == 6:
                    age_columns = {}
                    for column in range(12, 30):
                        label = values.get(column)
                        if label is None:
                            raise ValueError(
                                f"{path.name}:{sheet}: missing age header "
                                f"column {column}"
                            )
                        label = label.replace("–", "-")
                        if label == "85 and over":
                            label = "85+"
                        age_columns[column] = label
                    continue
                if values.get(1) != str(year):
                    continue
                if age_columns is None:
                    raise ValueError(f"{path.name}:{sheet}: data before headers")
                state = sdmx_csv.STATE_BY_REGION.get(values.get(2))
                if state is None:
                    # State code 9 is Other Territories, outside N1 geography.
                    if values.get(2) != "9":
                        raise ValueError(
                            f"{path.name}:{sheet}: unknown state code "
                            f"{values.get(2)!r}"
                        )
                    continue
                sa2 = values.get(10)
                if not sa2 or sa2 in seen:
                    raise ValueError(
                        f"{path.name}:{sheet}: invalid duplicate SA2 {sa2!r}"
                    )
                seen.add(sa2)
                for column, age_band in age_columns.items():
                    raw = values.get(column)
                    try:
                        count = int(raw)
                    except (TypeError, ValueError) as error:
                        raise ValueError(
                            f"{path.name}:{sheet}: invalid count at "
                            f"{sa2} {age_band}: {raw!r}"
                        ) from error
                    key = (year, state, sex, age_band)
                    totals[key] = totals.get(key, 0) + count
            sa2_codes[sex] = seen
    if sa2_codes.get("male") != sa2_codes.get("female"):
        raise ValueError(f"{path.name}: male and female SA2 sets differ")
    expected = {
        (year, state, sex, age_band)
        for state in STATE_CODES
        for sex in ("male", "female")
        for age_band in (
            "0-4", "5-9", "10-14", "15-19", "20-24", "25-29",
            "30-34", "35-39", "40-44", "45-49", "50-54", "55-59",
            "60-64", "65-69", "70-74", "75-79", "80-84", "85+",
        )
    }
    _require_complete("erp_regional_2010_state_age_sex", set(totals), expected)
    return {
        "erp_regional_2010_state_age_sex.csv": write_csv(
            EXTRACTS / "erp_regional_2010_state_age_sex.csv",
            ["year", "state", "sex", "age_band", "persons"],
            ((*key, value) for key, value in totals.items()),
        )
    }


def _require_complete(label: str, actual: set, expected: set) -> None:
    missing = expected - actual
    extra = actual - expected
    if missing or extra:
        details = []
        if missing:
            details.append(f"{len(missing)} missing, first {min(missing)!r}")
        if extra:
            details.append(f"{len(extra)} extra, first {min(extra)!r}")
        raise ValueError(f"{label}: " + "; ".join(details))


def _put_unique(target: dict, key: tuple, value, source: str) -> None:
    if key in target:
        raise ValueError(f"{source}: duplicate observation {key!r}")
    target[key] = value


def build_demographic_detail(
    sources: dict, first_year: int, last_year: int
) -> dict[str, int]:
    """Build births, deaths and detailed overseas-migration extracts.

    Births and deaths are calendar registration-year observations. Overseas
    migration is financial-year data whose year is the starting/run year, so
    2010 denotes 2010-11. The temporal mismatch is retained and documented; it
    is not silently forced into an ERP component identity here.
    """
    written = {}
    years = set(range(first_year, last_year + 1))

    birth_entry = sources["births_state"]
    birth_path = fetch.CACHE / birth_entry["filename"]
    births = {}
    for row in sdmx_csv.read(
        birth_path,
        birth_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "REGION", "FREQ", "TIME_PERIOD",
         "OBS_VALUE", "UNIT_MEASURE"},
    ):
        if row["MEASURE"] != "1":
            raise ValueError(f"{birth_path.name}: expected births measure 1")
        state = sdmx_csv.STATE_BY_REGION.get(row["REGION"])
        if state is None:
            # The targeted response also carries Australia for reconciliation.
            if row["REGION"] != "AUS":
                raise ValueError(
                    f"{birth_path.name}: unknown region {row['REGION']!r}"
                )
            continue
        year = int(row["TIME_PERIOD"])
        if first_year <= year <= last_year:
            key = (year, state)
            _put_unique(births, key, sdmx_csv.count(row, birth_path.name),
                        birth_path.name)
    expected_births = {
        (year, state) for year in years for state in STATE_CODES
    }
    _require_complete("births_state", set(births), expected_births)
    written["births_state.csv"] = write_csv(
        EXTRACTS / "births_state.csv",
        ["year", "state", "births"],
        ((year, state, value) for (year, state), value in births.items()),
    )

    birth_sex_entry = sources["births_sex"]
    birth_sex_path = fetch.CACHE / birth_sex_entry["filename"]
    birth_sex = {}
    birth_sex_measure = {"4": "male", "5": "female"}
    for row in sdmx_csv.read(
        birth_sex_path,
        birth_sex_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "REGION", "FREQ", "TIME_PERIOD",
         "OBS_VALUE", "UNIT_MEASURE"},
    ):
        try:
            sex = birth_sex_measure[row["MEASURE"]]
        except KeyError as error:
            raise ValueError(
                f"{birth_sex_path.name}: unknown births measure "
                f"{error.args[0]!r}"
            ) from error
        if row["REGION"] != "AUS":
            raise ValueError(
                f"{birth_sex_path.name}: expected Australia, got "
                f"{row['REGION']!r}"
            )
        year = int(row["TIME_PERIOD"])
        if first_year <= year <= last_year:
            _put_unique(
                birth_sex,
                (year, sex),
                sdmx_csv.count(row, birth_sex_path.name),
                birth_sex_path.name,
            )
    expected_birth_sex = {
        (year, sex) for year in years for sex in birth_sex_measure.values()
    }
    _require_complete("births_sex", set(birth_sex), expected_birth_sex)
    written["births_sex.csv"] = write_csv(
        EXTRACTS / "births_sex.csv",
        ["year", "sex", "births"],
        ((*key, value) for key, value in birth_sex.items()),
    )

    death_entry = sources["deaths_state_age_sex"]
    death_path = fetch.CACHE / death_entry["filename"]
    deaths = {}
    for row in sdmx_csv.read(
        death_path,
        death_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "SEX", "AGE", "REGION", "FREQ",
         "TIME_PERIOD", "OBS_VALUE", "UNIT_MEASURE"},
    ):
        try:
            state = sdmx_csv.STATE_BY_REGION[row["REGION"]]
            sex = sdmx_csv.SEX_BY_CODE[row["SEX"]]
            age_band = sdmx_csv.DEATH_AGE_BAND[row["AGE"]]
        except KeyError as error:
            raise ValueError(
                f"{death_path.name}: unknown dimension code {error.args[0]!r}"
            ) from error
        if row["MEASURE"] != "4":
            raise ValueError(f"{death_path.name}: expected deaths measure 4")
        year = int(row["TIME_PERIOD"])
        if first_year <= year <= last_year:
            key = (year, state, sex, age_band)
            _put_unique(deaths, key, sdmx_csv.count(row, death_path.name),
                        death_path.name)
    expected_deaths = {
        (year, state, sex, age_band)
        for year in years
        for state in STATE_CODES
        for sex in sdmx_csv.SEX_BY_CODE.values()
        for age_band in sdmx_csv.DEATH_AGE_BAND.values()
    }
    _require_complete("deaths_state_age_sex", set(deaths), expected_deaths)
    written["deaths_state_age_sex.csv"] = write_csv(
        EXTRACTS / "deaths_state_age_sex.csv",
        ["year", "state", "sex", "age_band", "deaths"],
        ((*key, value) for key, value in deaths.items()),
    )

    mortality_entry = sources["mortality_rates_state_age_sex"]
    mortality_path = fetch.CACHE / mortality_entry["filename"]
    mortality = {}
    national_mortality = {}
    for row in sdmx_csv.read(
        mortality_path,
        mortality_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "SEX", "AGE", "REGION", "FREQ",
         "TIME_PERIOD", "OBS_VALUE", "UNIT_MEASURE"},
    ):
        try:
            sex = sdmx_csv.SEX_BY_CODE[row["SEX"]]
            age_band = sdmx_csv.MORTALITY_AGE_BAND[row["AGE"]]
        except KeyError as error:
            raise ValueError(
                f"{mortality_path.name}: unknown dimension code "
                f"{error.args[0]!r}"
            ) from error
        if row["MEASURE"] != "12":
            raise ValueError(
                f"{mortality_path.name}: expected death-rate measure 12"
            )
        year = int(row["TIME_PERIOD"])
        if not first_year <= year <= last_year:
            continue
        value = sdmx_csv.real(row, mortality_path.name)
        if row["REGION"] == "AUS":
            _put_unique(
                national_mortality, (year, sex, age_band), value,
                mortality_path.name,
            )
            continue
        try:
            state = sdmx_csv.STATE_BY_REGION[row["REGION"]]
        except KeyError as error:
            raise ValueError(
                f"{mortality_path.name}: unknown region {error.args[0]!r}"
            ) from error
        _put_unique(
            mortality, (year, state, sex, age_band), value,
            mortality_path.name,
        )
    expected_mortality = {
        (year, state, sex, age_band)
        for year in years
        for state in STATE_CODES
        for sex in sdmx_csv.SEX_BY_CODE.values()
        for age_band in sdmx_csv.MORTALITY_AGE_BAND.values()
    }
    unexpected = set(mortality) - expected_mortality
    if unexpected:
        raise ValueError(
            f"mortality_rates_state_age_sex: unexpected {min(unexpected)!r}"
        )
    missing_mortality = expected_mortality - set(mortality)
    known_missing = {
        (2010, "nt", "female", "100+"),
        (2011, "nt", "female", "100+"),
    }
    if missing_mortality != known_missing:
        raise ValueError(
            "mortality_rates_state_age_sex: published gaps changed: "
            f"{sorted(missing_mortality)!r}"
        )
    written["mortality_rates_state_age_sex.csv"] = write_csv(
        EXTRACTS / "mortality_rates_state_age_sex.csv",
        ["year", "state", "sex", "age_band", "rate_per_1000", "status"],
        (
            (*key, mortality[key], "published")
            if key in mortality
            else (*key, "", "not_published_zero_exposure")
            for key in expected_mortality
        ),
    )
    expected_national_mortality = {
        (year, sex, age_band)
        for year in years
        for sex in sdmx_csv.SEX_BY_CODE.values()
        for age_band in sdmx_csv.MORTALITY_AGE_BAND.values()
    }
    _require_complete(
        "mortality_rates_national_age_sex",
        set(national_mortality),
        expected_national_mortality,
    )
    written["mortality_rates_national_age_sex.csv"] = write_csv(
        EXTRACTS / "mortality_rates_national_age_sex.csv",
        ["year", "sex", "age_band", "rate_per_1000"],
        ((*key, value) for key, value in national_mortality.items()),
    )

    nim_entry = sources["interstate_age_sex"]
    nim_path = fetch.CACHE / nim_entry["filename"]
    nim = {}
    nim_measure_name = {"1": "arrivals", "2": "departures"}
    for row in sdmx_csv.read(
        nim_path,
        nim_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "AGE", "SEX", "REGION", "FREQ",
         "TIME_PERIOD", "OBS_VALUE", "UNIT_MEASURE"},
    ):
        try:
            measure = nim_measure_name[row["MEASURE"]]
            state = sdmx_csv.STATE_BY_REGION[row["REGION"]]
            sex = sdmx_csv.SEX_BY_CODE[row["SEX"]]
            age_band = sdmx_csv.NIM_AGE_BAND[row["AGE"]]
        except KeyError as error:
            raise ValueError(
                f"{nim_path.name}: unknown dimension code {error.args[0]!r}"
            ) from error
        # NIM_FY, like NOM_FY, labels a financial year by its ending June.
        year = int(row["TIME_PERIOD"]) - 1
        if not first_year <= year <= last_year:
            continue
        key = (year, state, sex, age_band)
        bucket = nim.setdefault(key, {})
        if measure in bucket:
            raise ValueError(
                f"{nim_path.name}: duplicate {measure} observation {key!r}"
            )
        bucket[measure] = sdmx_csv.count(row, nim_path.name)
    expected_nim = {
        (year, state, sex, age_band)
        for year in years
        for state in STATE_CODES
        for sex in sdmx_csv.SEX_BY_CODE.values()
        for age_band in sdmx_csv.NIM_AGE_BAND.values()
    }
    _require_complete("interstate_state_age_sex", set(nim), expected_nim)
    incomplete_nim = [
        key for key, values in nim.items()
        if set(values) != set(nim_measure_name.values())
    ]
    if incomplete_nim:
        raise ValueError(
            f"interstate_state_age_sex: incomplete directions at "
            f"{min(incomplete_nim)!r}"
        )
    written["interstate_state_age_sex.csv"] = write_csv(
        EXTRACTS / "interstate_state_age_sex.csv",
        ["year", "state", "sex", "age_band", "arrivals", "departures"],
        ((*key, values["arrivals"], values["departures"])
         for key, values in nim.items()),
    )

    nom_entry = sources["nom_state_age_sex"]
    nom_path = fetch.CACHE / nom_entry["filename"]
    nom = {}
    measure_name = {"1": "arrivals", "2": "departures"}
    for row in sdmx_csv.read(
        nom_path,
        nom_entry["dataflow"],
        {"DATAFLOW", "MEASURE", "AGE", "SEX", "REGION", "FREQ",
         "TIME_PERIOD", "OBS_VALUE", "UNIT_MEASURE"},
    ):
        try:
            measure = measure_name[row["MEASURE"]]
            state = sdmx_csv.STATE_BY_REGION[row["REGION"]]
            sex = sdmx_csv.SEX_BY_CODE[row["SEX"]]
            age_band = sdmx_csv.NOM_AGE_BAND[row["AGE"]]
        except KeyError as error:
            raise ValueError(
                f"{nom_path.name}: unknown dimension code {error.args[0]!r}"
            ) from error
        # NOM_FY labels a financial year by its ending June: TIME_PERIOD 2011
        # is 2010-11, hence model run year 2010.
        year = int(row["TIME_PERIOD"]) - 1
        if not first_year <= year <= last_year:
            continue
        key = (year, state, sex, age_band)
        bucket = nom.setdefault(key, {})
        if measure in bucket:
            raise ValueError(
                f"{nom_path.name}: duplicate {measure} observation {key!r}"
            )
        bucket[measure] = sdmx_csv.count(row, nom_path.name)
    expected_nom = {
        (year, state, sex, age_band)
        for year in years
        for state in STATE_CODES
        for sex in sdmx_csv.SEX_BY_CODE.values()
        for age_band in sdmx_csv.NOM_AGE_BAND.values()
    }
    _require_complete("nom_state_age_sex", set(nom), expected_nom)
    incomplete = [key for key, values in nom.items()
                  if set(values) != set(measure_name.values())]
    if incomplete:
        raise ValueError(
            f"nom_state_age_sex: incomplete directions at {min(incomplete)!r}"
        )
    written["nom_state_age_sex.csv"] = write_csv(
        EXTRACTS / "nom_state_age_sex.csv",
        ["year", "state", "sex", "age_band", "arrivals", "departures"],
        ((*key, values["arrivals"], values["departures"])
         for key, values in nom.items()),
    )
    return written


def main(argv: list[str] | None = None) -> int:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--first-year", type=int, default=2010)
    parser.add_argument("--last-year", type=int, default=2025)
    args = parser.parse_args(argv)

    sources = fetch.load_sources()
    status = fetch.verify(sources)
    bad = {k: v for k, v in status.items() if v != "ok"}
    if bad:
        for key, state in sorted(bad.items()):
            print(f"{key}: {state}", file=sys.stderr)
        print(
            "cache is not verified; run `python3 fetch.py --download`",
            file=sys.stderr,
        )
        return 1

    written = build_erp(sources, args.first_year, args.last_year)
    # Run year Y carries stocks from 30 June Y to 30 June Y+1, so the last run
    # year is one before the last stock year.
    written.update(build_flows(sources, args.first_year, args.last_year - 1))
    written.update(build_overseas(sources, args.first_year, args.last_year - 1))
    written.update(build_life_tables(sources))
    written.update(build_regional_crosscheck(sources, 2010))
    written.update(
        build_demographic_detail(sources, args.first_year, args.last_year - 1)
    )
    for name, count in sorted(written.items()):
        print(f"{name:30s} {count:>7d} rows")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

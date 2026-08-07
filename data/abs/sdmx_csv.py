"""Read pinned ABS SDMX-CSV responses without contacting the Data API.

The raw responses are deliberately narrow API queries: only the measures,
regions, sexes and non-overlapping age bands used by the canonical extracts are
cached. Codes are mapped explicitly here rather than trusting display labels,
which keeps extraction stable if ABS changes wording while retaining the pinned
dataflow version.
"""

from __future__ import annotations

import csv
import pathlib

STATE_BY_REGION = {
    "1": "nsw",
    "2": "vic",
    "3": "qld",
    "4": "sa",
    "5": "wa",
    "6": "tas",
    "7": "nt",
    "8": "act",
}

SEX_BY_CODE = {"1": "male", "2": "female"}

DEATH_AGE_BAND = {
    "A04": "0-4",
    "A59": "5-9",
    "A10": "10-14",
    "A15": "15-19",
    "A20": "20-24",
    "A25": "25-29",
    "A30": "30-34",
    "A35": "35-39",
    "A40": "40-44",
    "A45": "45-49",
    "A50": "50-54",
    "A55": "55-59",
    "A60": "60-64",
    "A65": "65-69",
    "A70": "70-74",
    "A75": "75-79",
    "A80": "80-84",
    "A85": "85-89",
    "A90": "90-94",
    "A95": "95-99",
    "A99": "100+",
    "999": "not_stated",
}

MORTALITY_AGE_BAND = {
    code: label for code, label in DEATH_AGE_BAND.items() if code != "999"
}

NIM_AGE_BAND = {
    "A04": "0-4", "A59": "5-9", "A10": "10-14", "A15": "15-19",
    "A20": "20-24", "A25": "25-29", "A30": "30-34",
    "A35": "35-39", "A40": "40-44", "A45": "45-49",
    "A50": "50-54", "A55": "55-59", "A60": "60-64",
    "A65": "65-69", "A70": "70-74", "7599": "75+",
}

NOM_AGE_BAND = {
    "A04": "0-4",
    "A59": "5-9",
    "A10": "10-14",
    "A15": "15-19",
    "A20": "20-24",
    "A25": "25-29",
    "A30": "30-34",
    "A35": "35-39",
    "A40": "40-44",
    "A45": "45-49",
    "A50": "50-54",
    "A55": "55-59",
    "A60": "60-64",
    "6599": "65+",
}


def read(
    path, expected_dataflow: str, required: set[str],
    frequency: str = "A", unit_measure: str = "NUM",
):
    """Yield validated SDMX observations as dictionaries of code strings."""
    path = pathlib.Path(path)
    with path.open("r", encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        fields = set(reader.fieldnames or ())
        missing = required - fields
        if missing:
            raise ValueError(
                f"{path.name}: missing SDMX columns {sorted(missing)!r}"
            )
        for line_number, row in enumerate(reader, 2):
            if row.get("DATAFLOW") != expected_dataflow:
                raise ValueError(
                    f"{path.name}:{line_number}: unexpected DATAFLOW "
                    f"{row.get('DATAFLOW')!r}"
                )
            if row.get("FREQ") != frequency:
                raise ValueError(
                    f"{path.name}:{line_number}: expected frequency "
                    f"{frequency!r}"
                )
            if row.get("UNIT_MEASURE") != unit_measure:
                raise ValueError(
                    f"{path.name}:{line_number}: expected unit "
                    f"{unit_measure!r}"
                )
            yield row


def count(row: dict[str, str], source: str) -> int:
    """Parse one non-negative integer count, rejecting suppression markers."""
    raw = row.get("OBS_VALUE", "")
    try:
        value = int(raw)
    except ValueError as error:
        raise ValueError(f"{source}: non-integer observation {raw!r}") from error
    if value < 0:
        raise ValueError(f"{source}: negative count {value}")
    return value


def real(row: dict[str, str], source: str) -> float:
    """Parse one finite non-negative real observation."""
    raw = row.get("OBS_VALUE", "")
    try:
        value = float(raw)
    except ValueError as error:
        raise ValueError(f"{source}: non-real observation {raw!r}") from error
    if value < 0 or value != value or value == float("inf"):
        raise ValueError(f"{source}: invalid real observation {value!r}")
    return value

"""Reader for ABS time-series workbooks.

ABS publishes its time-series cubes in a stable shape: an ``Index`` sheet, one
or more ``Data1``..``DataN`` sheets, and an ``Enquiries`` sheet. On a data
sheet, row 1 carries the series descriptor for each column, a block of labelled
metadata rows follows in column A, and the observations begin on the row after
``Series ID``. Column A of an observation row holds an Excel date serial.

The metadata block is located by finding the ``Series ID`` label rather than by
assuming fixed row numbers, so a workbook that gains or loses a metadata row is
read correctly instead of silently mis-parsed.
"""

from __future__ import annotations

import datetime

from xlsx import Workbook, serial_to_date

_SERIES_ID_LABEL = "Series ID"


class Series:
    """One ABS time series: its descriptor, metadata and observations."""

    def __init__(self, descriptor: str, metadata: dict[str, str]) -> None:
        self.descriptor = descriptor
        self.metadata = metadata
        self.observations: dict[datetime.date, float] = {}

    @property
    def series_id(self) -> str:
        return self.metadata.get(_SERIES_ID_LABEL, "")

    def __repr__(self) -> str:
        return (
            f"Series({self.descriptor!r}, id={self.series_id!r}, "
            f"n={len(self.observations)})"
        )


def data_sheet_names(workbook: Workbook) -> list[str]:
    """Return the ``Data*`` sheet names in workbook order."""
    return [name for name in workbook.sheet_names if name.startswith("Data")]


def read_series(path) -> list[Series]:
    """Read every series from every data sheet of an ABS time-series workbook."""
    out: list[Series] = []
    with Workbook(path) as book:
        sheets = data_sheet_names(book)
        if not sheets:
            raise ValueError(f"{path}: no Data* sheet found")
        for sheet in sheets:
            out.extend(_read_sheet(book, sheet, path))
    return out


def _read_sheet(book: Workbook, sheet: str, path) -> list[Series]:
    cells = book.cells(sheet)
    if not cells:
        return []
    max_col = max(col for col, _ in cells)
    max_row = max(row for _, row in cells)

    label_rows = {
        cells[(1, row)]: row
        for row in range(1, max_row + 1)
        if (1, row) in cells
    }
    if _SERIES_ID_LABEL not in label_rows:
        raise ValueError(f"{path}:{sheet}: no {_SERIES_ID_LABEL!r} metadata row")
    id_row = label_rows[_SERIES_ID_LABEL]
    first_data_row = id_row + 1

    series_by_col: dict[int, Series] = {}
    for col in range(2, max_col + 1):
        descriptor = cells.get((col, 1))
        if not descriptor:
            continue
        metadata = {
            label: cells.get((col, row), "")
            for label, row in label_rows.items()
            if row <= id_row
        }
        series_by_col[col] = Series(descriptor, metadata)

    for row in range(first_data_row, max_row + 1):
        stamp = cells.get((1, row))
        if not stamp:
            continue
        try:
            when = serial_to_date(float(stamp))
        except ValueError:
            # Trailing copyright and footnote rows are not observations.
            continue
        for col, series in series_by_col.items():
            raw = cells.get((col, row))
            if raw in (None, ""):
                continue
            try:
                series.observations[when] = float(raw)
            except ValueError:
                continue

    return list(series_by_col.values())

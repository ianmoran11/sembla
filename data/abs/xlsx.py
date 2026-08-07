"""Minimal read-only XLSX reader built on the Python standard library.

An `.xlsx` file is a zip of XML parts, so `zipfile` plus
`xml.etree.ElementTree` is sufficient and no third-party reader is needed
(DECISIONS.md N10). This module resolves cells by their declared reference
rather than by position, because ABS workbooks contain sparse rows, merged
header blocks and footnote rows that a positional parser mis-reads silently.

Only the features ABS workbooks actually use are supported: shared strings,
inline strings, numeric cells and boolean cells. Formulas are read as their
cached values, which is what the published workbooks carry.
"""

from __future__ import annotations

import datetime
import re
import xml.etree.ElementTree as ET
import zipfile

_MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
_RELS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
_NS = {"m": _MAIN, "r": _RELS}

_CELL_REF = re.compile(r"^([A-Z]+)(\d+)$")

# Excel's 1900 date system counts day 1 as 1900-01-01 but wrongly treats 1900
# as a leap year, so the usual serial-to-date origin is 1899-12-30.
_EPOCH = datetime.date(1899, 12, 30)


def column_index(letters: str) -> int:
    """Convert a column label such as ``AA`` to a 1-based index."""
    n = 0
    for ch in letters:
        n = n * 26 + (ord(ch) - 64)
    return n


def parse_ref(ref: str) -> tuple[int, int]:
    """Convert a cell reference such as ``B12`` to ``(column, row)``, 1-based."""
    match = _CELL_REF.match(ref)
    if match is None:
        raise ValueError(f"unrecognised cell reference {ref!r}")
    return column_index(match.group(1)), int(match.group(2))


def serial_to_date(serial: float) -> datetime.date:
    """Convert an Excel serial day number to a date."""
    return _EPOCH + datetime.timedelta(days=int(serial))


class Workbook:
    """A read-only view of an XLSX file."""

    def __init__(self, path) -> None:
        self._zip = zipfile.ZipFile(path)
        self._shared = self._read_shared_strings()
        self._sheets = self._read_sheet_map()

    def close(self) -> None:
        self._zip.close()

    def __enter__(self) -> "Workbook":
        return self

    def __exit__(self, *exc) -> None:
        self.close()

    @property
    def sheet_names(self) -> list[str]:
        return list(self._sheets)

    def _read_shared_strings(self) -> list[str]:
        try:
            raw = self._zip.read("xl/sharedStrings.xml")
        except KeyError:
            return []
        root = ET.fromstring(raw)
        # A shared string may be split across several runs; concatenating every
        # <t> descendant rebuilds the logical value including rich-text runs.
        return [
            "".join(t.text or "" for t in si.iter(f"{{{_MAIN}}}t"))
            for si in root.findall("m:si", _NS)
        ]

    def _read_sheet_map(self) -> dict[str, str]:
        targets = {
            rel.get("Id"): rel.get("Target")
            for rel in ET.fromstring(self._zip.read("xl/_rels/workbook.xml.rels"))
        }
        book = ET.fromstring(self._zip.read("xl/workbook.xml"))
        sheets = {}
        for sheet in book.findall(".//m:sheet", _NS):
            target = targets[sheet.get(f"{{{_RELS}}}id")]
            if target.startswith("/xl/"):
                path = target.lstrip("/")
            elif target.startswith("xl/"):
                path = target
            else:
                path = "xl/" + target.lstrip("/")
            sheets[sheet.get("name")] = path
        return sheets

    def _cell_value(self, cell) -> str | None:
        kind = cell.get("t")
        if kind == "inlineStr":
            node = cell.find(f"{{{_MAIN}}}is")
            if node is None:
                return None
            return "".join(t.text or "" for t in node.iter(f"{{{_MAIN}}}t"))
        node = cell.find(f"{{{_MAIN}}}v")
        if node is None or node.text is None:
            return None
        return self._shared[int(node.text)] if kind == "s" else node.text

    def iter_rows(self, sheet_name: str):
        """Yield ``(row, {column: value})`` without materialising the sheet.

        Large ABS regional cubes contain millions of cells. Streaming by row
        keeps extraction bounded while retaining the same sparse-coordinate
        semantics as :meth:`cells`.
        """
        if sheet_name not in self._sheets:
            raise KeyError(f"no sheet named {sheet_name!r}; have {self.sheet_names}")
        with self._zip.open(self._sheets[sheet_name]) as stream:
            for _event, element in ET.iterparse(stream, events=("end",)):
                if element.tag != f"{{{_MAIN}}}row":
                    continue
                values = {}
                row_number = int(element.get("r", "0"))
                for cell in element.findall(f"{{{_MAIN}}}c"):
                    ref = cell.get("r")
                    if ref is None:
                        continue
                    column, declared_row = parse_ref(ref)
                    if row_number == 0:
                        row_number = declared_row
                    value = self._cell_value(cell)
                    if value is not None:
                        values[column] = value
                if values:
                    yield row_number, values
                element.clear()

    def cells(self, sheet_name: str) -> dict[tuple[int, int], str]:
        """Return ``{(column, row): value}`` for every populated cell."""
        out: dict[tuple[int, int], str] = {}
        for row, values in self.iter_rows(sheet_name):
            out.update(((column, row), value) for column, value in values.items())
        return out

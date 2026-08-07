"""Tests for the canonical writers and the XLSX / ABS time-series readers."""

from __future__ import annotations

import ast
import datetime
import pathlib
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import absts  # noqa: E402
import canonical  # noqa: E402
import xlsx  # noqa: E402

MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main"
RELS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"


def build_workbook(path: pathlib.Path, sheet_xml: str, shared: list[str]) -> None:
    """Write a minimal but structurally valid XLSX containing one data sheet."""
    si = "".join(f"<si><t>{s}</t></si>" for s in shared)
    with zipfile.ZipFile(path, "w") as z:
        z.writestr(
            "xl/workbook.xml",
            f'<workbook xmlns="{MAIN}" xmlns:r="{RELS}"><sheets>'
            f'<sheet name="Index" sheetId="1" r:id="rId1"/>'
            f'<sheet name="Data1" sheetId="2" r:id="rId2"/>'
            f"</sheets></workbook>",
        )
        z.writestr(
            "xl/_rels/workbook.xml.rels",
            '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/'
            'relationships">'
            '<Relationship Id="rId1" Target="worksheets/sheet1.xml"/>'
            '<Relationship Id="rId2" Target="worksheets/sheet2.xml"/>'
            "</Relationships>",
        )
        z.writestr(
            "xl/sharedStrings.xml",
            f'<sst xmlns="{MAIN}" count="{len(shared)}" '
            f'uniqueCount="{len(shared)}">{si}</sst>',
        )
        z.writestr("xl/worksheets/sheet1.xml", f'<worksheet xmlns="{MAIN}"/>')
        z.writestr("xl/worksheets/sheet2.xml", sheet_xml)


class TestQuarantine(unittest.TestCase):
    def test_pipeline_imports_only_stdlib_and_local_modules(self):
        root = pathlib.Path(__file__).resolve().parent.parent
        local = {path.stem for path in root.glob("*.py")}
        for path in root.glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            imports = set()
            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    imports.update(alias.name.split(".")[0] for alias in node.names)
                elif isinstance(node, ast.ImportFrom) and node.module:
                    imports.add(node.module.split(".")[0])
            external = imports - local - sys.stdlib_module_names
            self.assertEqual(external, set(), f"{path.name}: {sorted(external)}")
            self.assertNotIn("sembla", imports, path.name)


class TestCanonical(unittest.TestCase):
    def test_round_trip_preserves_rows(self):
        rows = [(2, "b", 1.5), (1, "a,x", 2.0)]
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "t.csv"
            canonical.write_csv(p, ["n", "s", "v"], rows)
            header, back = canonical.read_csv(p)
        self.assertEqual(header, ["n", "s", "v"])
        # Sorted by the full key tuple, so row 1 comes first.
        self.assertEqual(back[0], ["1", "a,x", "2.0"])
        self.assertEqual(back[1], ["2", "b", "1.5"])

    def test_embedded_comma_is_quoted_and_recovered(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "t.csv"
            canonical.write_csv(p, ["a"], [('x,"y"',)])
            text = p.read_text()
            _, rows = canonical.read_csv(p)
        self.assertIn('"x,""y"""', text)
        self.assertEqual(rows[0][0], 'x,"y"')

    def test_output_is_lf_terminated(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "t.csv"
            canonical.write_csv(p, ["a"], [(1,)])
            raw = p.read_bytes()
        self.assertTrue(raw.endswith(b"\n"))
        self.assertNotIn(b"\r", raw)

    def test_rejects_row_of_wrong_width(self):
        with tempfile.TemporaryDirectory() as d:
            with self.assertRaises(ValueError):
                canonical.write_csv(pathlib.Path(d) / "t.csv", ["a", "b"], [(1,)])

    def test_real_formatting_is_stable(self):
        self.assertEqual(canonical.format_value(1.0), "1.0")
        self.assertEqual(canonical.format_value(0.000123456789), "0.000123457")
        self.assertEqual(canonical.format_value(7), "7")


class TestXlsx(unittest.TestCase):
    def test_reads_checked_in_merged_shared_inline_and_sparse_fixture(self):
        path = pathlib.Path(__file__).parent / "fixtures" / "xlsx-reader.xlsx"
        with zipfile.ZipFile(path) as archive:
            sheet_xml = archive.read("xl/worksheets/sheet2.xml")
        self.assertIn(b'<mergeCell ref="A1:C1"', sheet_xml)
        with xlsx.Workbook(path) as book:
            rows = list(book.iter_rows("Data1"))
            cells = book.cells("Data1")
            names = book.sheet_names
        # Only the top-left merged cell has a value. Sparse rows and columns
        # keep their declared references rather than shifting left or upward.
        self.assertEqual(cells[(1, 1)], "merged header")
        self.assertEqual(cells[(3, 3)], "inline")
        self.assertEqual(cells[(2, 5)], "42")
        self.assertNotIn((2, 1), cells)
        self.assertEqual(rows, [
            (1, {1: "merged header"}),
            (3, {3: "inline"}),
            (5, {2: "42"}),
        ])
        self.assertEqual(names, ["Index", "Data1"])

    def test_column_and_reference_parsing(self):
        self.assertEqual(xlsx.column_index("A"), 1)
        self.assertEqual(xlsx.column_index("Z"), 26)
        self.assertEqual(xlsx.column_index("AA"), 27)
        self.assertEqual(xlsx.parse_ref("AB12"), (28, 12))

    def test_serial_dates(self):
        self.assertEqual(xlsx.serial_to_date(26085), datetime.date(1971, 6, 1))
        self.assertEqual(xlsx.serial_to_date(45809), datetime.date(2025, 6, 1))


class TestAbsTimeSeries(unittest.TestCase):
    SHEET = (
        f'<worksheet xmlns="{MAIN}"><sheetData>'
        '<row r="1"><c r="B1" t="s"><v>0</v></c></row>'
        '<row r="2"><c r="A2" t="s"><v>1</v></c><c r="B2" t="s"><v>2</v></c></row>'
        '<row r="3"><c r="A3" t="s"><v>3</v></c><c r="B3" t="s"><v>4</v></c></row>'
        '<row r="4"><c r="A4"><v>26085</v></c><c r="B4"><v>100</v></c></row>'
        '<row r="5"><c r="A5"><v>26451</v></c><c r="B5"><v>110</v></c></row>'
        '<row r="6"><c r="A6" t="s"><v>5</v></c></row>'
        "</sheetData></worksheet>"
    )
    SHARED = ["ERP ; Male ; 0 ;", "Unit", "Persons", "Series ID", "A1234X",
              "\u00a9 Commonwealth of Australia"]

    def test_metadata_block_located_by_series_id(self):
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "ts.xlsx"
            build_workbook(p, self.SHEET, self.SHARED)
            series = absts.read_series(p)
        self.assertEqual(len(series), 1)
        one = series[0]
        self.assertEqual(one.descriptor, "ERP ; Male ; 0 ;")
        self.assertEqual(one.series_id, "A1234X")
        self.assertEqual(one.metadata["Unit"], "Persons")
        # The trailing copyright row is not an observation.
        self.assertEqual(len(one.observations), 2)
        self.assertEqual(one.observations[datetime.date(1971, 6, 1)], 100.0)

    def test_missing_series_id_is_an_error(self):
        sheet = (
            f'<worksheet xmlns="{MAIN}"><sheetData>'
            '<row r="1"><c r="B1" t="s"><v>0</v></c></row>'
            "</sheetData></worksheet>"
        )
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "bad.xlsx"
            build_workbook(p, sheet, ["ERP ; Male ; 0 ;"])
            with self.assertRaises(ValueError):
                absts.read_series(p)


if __name__ == "__main__":
    unittest.main()

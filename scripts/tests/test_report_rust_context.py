#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "report-rust-context.py"
SPEC = importlib.util.spec_from_file_location("report_rust_context", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
REPORT = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = REPORT
SPEC.loader.exec_module(REPORT)


class RustContextReportTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        for crate in REPORT.BACKEND_CRATES:
            source = self.root / "crates" / crate / "src" / "lib.rs"
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_text("pub fn identity(value: bool) -> bool { value }\n")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def run_script(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), "--root", str(self.root), *arguments],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_masks_literals_and_nested_comments_without_losing_lines(self) -> None:
        source = (
            'fn example() { if true { println!(r#"match while"#); } }\n'
            "/* outer\n/* nested */\nend */\n"
        )

        masked = REPORT.mask_non_code(source)

        self.assertEqual(masked.count("\n"), source.count("\n"))
        self.assertNotIn("match while", masked)
        self.assertNotIn("nested", masked)
        self.assertIn("if true", masked)

    def test_reports_functions_and_excludes_adjacent_test_sources(self) -> None:
        source_directory = self.root / "crates" / "sembla-cpu" / "src"
        (source_directory / "lib.rs").write_text(
            "pub fn choose(left: bool, right: bool) -> bool {\n"
            "    if left && right { true } else { false }\n"
            "}\n",
            encoding="utf-8",
        )
        (source_directory / "lib_tests.rs").write_text(
            "fn enormous_test() { loop {} }\n", encoding="utf-8"
        )

        report = REPORT.build_report(self.root)
        function = next(
            metric for metric in report["functions"] if metric["name"] == "choose"
        )

        self.assertEqual(function["complexity"], 3)
        self.assertFalse(
            any(metric["name"] == "enormous_test" for metric in report["functions"])
        )

    def test_budget_failure_is_reported(self) -> None:
        budget = self.root / "budget.json"
        budget.write_text(
            json.dumps(
                {
                    "limits": {
                        "total_tokens": 1,
                        "max_file_code_lines": 10,
                        "max_function_lines": 10,
                        "max_function_complexity": 10,
                    },
                    "crate_token_limits": {},
                    "crate_file_line_limits": {},
                }
            ),
            encoding="utf-8",
        )

        result = self.run_script("--check", str(budget))

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("total_tokens", result.stderr)

    def test_json_report_contains_each_backend_crate(self) -> None:
        result = self.run_script("--json")

        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        self.assertEqual(set(report["crates"]), set(REPORT.BACKEND_CRATES))
        self.assertEqual(report["crates"]["sembla-cpu"]["max_file_code_lines"], 1)


if __name__ == "__main__":
    unittest.main()

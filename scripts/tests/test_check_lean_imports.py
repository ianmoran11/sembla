#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[2] / "frontend" / "scripts" / "check-imports.py"


class LeanImportCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.write(
            "lakefile.toml",
            'defaultTargets = ["Sembla", "SemblaTests"]\n\n'
            '[[lean_lib]]\nname = "Sembla"\n\n'
            '[[lean_lib]]\nname = "SemblaTests"\n',
        )
        self.write("Sembla.lean", "import Sembla.IR\n")
        self.write("SemblaTests.lean", "import Sembla\nimport Sembla.IRTests\n")
        self.write("Main.lean", "import Sembla\n")
        self.write("LinkMain.lean", "import Sembla.Plan\n")
        self.write("Sembla/IR.lean", "import Std\n")
        self.write("Sembla/IRTests.lean", "import Sembla.IR\n")
        self.write("Sembla/Plan.lean", "import Sembla.IR\n")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write(self, relative_path: str, content: str) -> None:
        path = self.root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(CHECKER), "--root", str(self.root)],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_separate_production_and_test_import_surfaces_pass(self) -> None:
        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Lean import architecture passed", result.stdout)

    def test_production_umbrella_cannot_import_tests(self) -> None:
        self.write("Sembla.lean", "import Sembla.IR\nimport Sembla.IRTests\n")

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("production module must not import test module", result.stderr)

    def test_contract_module_cannot_import_dsl(self) -> None:
        self.write("Sembla/IR.lean", "import Sembla.DSL\n")

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("contract module must not import higher-level Sembla.DSL", result.stderr)

    def test_only_semantics_raw_may_import_source_contract(self) -> None:
        self.write("Sembla/Semantics/CheckModel.lean", "import Sembla.Composition.Source\n")
        self.write("Sembla/Semantics/Raw.lean", "import Sembla.Composition.Source\n")

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn(
            "Sembla.Semantics.CheckModel: semantics may not import composition module",
            result.stderr,
        )
        self.assertNotIn("Sembla.Semantics.Raw:", result.stderr)


if __name__ == "__main__":
    unittest.main()

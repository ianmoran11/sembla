#!/usr/bin/env python3

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check-artifact-registry.py"


class ArtifactRegistryCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source = self.root / "crates" / "demo" / "src" / "lib.rs"
        self.source.parent.mkdir(parents=True)
        self.source.write_text(
            'pub const SCHEMA: &str = "sembla.demo/v1";\n', encoding="utf-8"
        )
        self.registry_path = self.root / "registry.json"

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def entry(self, identifier: str = "sembla.demo/v1") -> dict[str, object]:
        return {
            "identifier": identifier,
            "kind": "artifact-schema",
            "owner": "crates/demo/src/lib.rs",
            "producers": ["crates/demo/src/lib.rs"],
            "consumers": ["crates/demo/src/lib.rs"],
            "stability": "versioned",
            "source_discovery": True,
        }

    def write_registry(self, entries: list[dict[str, object]]) -> None:
        self.registry_path.write_text(
            json.dumps(
                {
                    "registry_schema": "sembla.architecture-artifact-registry/v1",
                    "entries": entries,
                }
            ),
            encoding="utf-8",
        )

    def run_checker(self) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(CHECKER),
                "--root",
                str(self.root),
                "--registry",
                str(self.registry_path),
            ],
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            check=False,
        )

    def test_matching_source_and_registry_pass(self) -> None:
        self.write_registry([self.entry()])

        result = self.run_checker()

        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("1 entries, 1 source-discovered identifiers", result.stdout)

    def test_unregistered_source_identifier_fails(self) -> None:
        self.write_registry([])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("unregistered source identifier sembla.demo/v1", result.stderr)

    def test_stale_identifier_and_missing_owner_fail(self) -> None:
        entry = self.entry("sembla.stale/v1")
        entry["owner"] = "missing.rs"
        self.write_registry([entry])

        result = self.run_checker()

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("owner path does not exist: missing.rs", result.stderr)
        self.assertIn("stale source-discovery registry entry: sembla.stale/v1", result.stderr)


if __name__ == "__main__":
    unittest.main()

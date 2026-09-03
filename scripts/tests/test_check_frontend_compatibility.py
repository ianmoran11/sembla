#!/usr/bin/env python3

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest


CHECKER = Path(__file__).resolve().parents[1] / "check-frontend-compatibility.py"
SPEC = importlib.util.spec_from_file_location("check_frontend_compatibility", CHECKER)
assert SPEC is not None and SPEC.loader is not None
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


class FrontendCompatibilityCheckerTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.fixture = self.root / "fixtures" / "model.json"
        self.fixture.parent.mkdir(parents=True)
        self.fixture.write_bytes(b'{"schema":"sembla.bundle/v1"}\n')
        self.generated = self.root / "generated" / "table.json"
        self.generated.parent.mkdir(parents=True)
        self.generated.write_bytes(b"[]\n")

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def pin(self) -> dict[str, object]:
        return {
            "schema": checker.PIN_SCHEMA,
            "repository": checker.FRONTEND_REPOSITORY,
            "commit": "a" * 40,
            "contracts": checker.EXPECTED_CONTRACTS,
            "backend_surface": {
                "tracked_sha256": {
                    "fixtures/model.json": digest(self.fixture.read_bytes())
                },
                "generated_sha256": {
                    "generated/table.json": digest(self.generated.read_bytes())
                },
            },
        }

    def test_matching_pin_and_bytes_pass(self) -> None:
        commit, tracked, generated = checker.validate_pin(
            self.pin(), {"fixtures/model.json"}, {"generated/table.json"}
        )

        checker.verify_files(self.root, tracked, "tracked")
        checker.verify_files(self.root, generated, "generated")

        self.assertEqual(commit, "a" * 40)

    def test_drift_is_rejected(self) -> None:
        _, tracked, _ = checker.validate_pin(
            self.pin(), {"fixtures/model.json"}, {"generated/table.json"}
        )
        self.fixture.write_text("changed\n", encoding="utf-8")

        with self.assertRaisesRegex(checker.CheckError, "tracked drift"):
            checker.verify_files(self.root, tracked, "tracked")

    def test_pin_cannot_drop_a_required_path(self) -> None:
        pin = self.pin()
        surface = pin["backend_surface"]
        assert isinstance(surface, dict)
        surface["tracked_sha256"] = {}

        with self.assertRaisesRegex(checker.CheckError, "missing required paths"):
            checker.validate_pin(
                pin, {"fixtures/model.json"}, {"generated/table.json"}
            )

    def test_unsafe_path_is_rejected(self) -> None:
        pin = self.pin()
        surface = pin["backend_surface"]
        assert isinstance(surface, dict)
        surface["tracked_sha256"] = {"../model.json": "0" * 64}

        with self.assertRaisesRegex(checker.CheckError, "unsafe compatibility path"):
            checker.validate_pin(pin, {"../model.json"}, {"generated/table.json"})

    def test_pin_is_json_serializable(self) -> None:
        json.dumps(self.pin())


if __name__ == "__main__":
    unittest.main()

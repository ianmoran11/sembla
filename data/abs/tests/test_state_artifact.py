"""Cross-language conformance tests for the Python state-artifact writer."""

from __future__ import annotations

import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import state_artifact  # noqa: E402


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODEL = ROOT / "fixtures/state/models/refs_small.json"
ARTIFACT = ROOT / "fixtures/state/refs_small.state"


def refs_columns():
    return {
        ("world", "Area", "rate"): [1.0, 1.5],
        ("world", "Person", "age"): [10, 20, 30],
        ("world", "Person", "status"): ["Open", "Closed", "Open"],
        ("world", "Person", "area"): [0, 1, 0],
    }


class TestStateArtifactWriter(unittest.TestCase):
    def test_python_bytes_equal_the_rust_generated_fixture(self):
        model = state_artifact.load_model(MODEL)
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "refs.state"
            digest = state_artifact.write_state(output, model, refs_columns())
            self.assertEqual(output.read_bytes(), ARTIFACT.read_bytes())
            self.assertEqual(
                digest.record(),
                "state sha256 sembla.state-artifact/v1 "
                "bdf363c0b12a157f9adffcca3591b33026e4907d99ec9a871623857935140cdc",
            )

    def test_iterable_factories_are_streamed_in_model_order(self):
        calls = []

        def source(label, values):
            def generate():
                calls.append(label)
                yield from values
            return generate

        columns = {
            ("world", "Area", "rate"): source("rate", [1.0, 1.5]),
            ("world", "Person", "age"): source("age", [10, 20, 30]),
            ("world", "Person", "status"): source("status", [0, 1, 0]),
            ("world", "Person", "area"): source("area", [0, 1, 0]),
        }
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "refs.state"
            state_artifact.write_state(
                output, state_artifact.load_model(MODEL), columns
            )
            self.assertEqual(calls, ["rate", "age", "status", "area"])
            self.assertEqual(output.read_bytes(), ARTIFACT.read_bytes())

    def test_companion_uses_synth_state_name_and_canonical_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            artifact = pathlib.Path(directory) / "refs.state"
            companion = state_artifact.write_companion_bytes(
                artifact, MODEL.read_bytes()
            )
            self.assertEqual(companion.name, "refs.state.model.json")
            self.assertEqual(companion.read_bytes(), MODEL.read_bytes())

    def test_resizing_changes_only_selected_size_hints(self):
        model = state_artifact.load_model(MODEL)
        resized = state_artifact.resized_model(
            model, {("world", "Area"): 4, ("world", "Person"): 7}
        )
        self.assertEqual(model["boxes"][0]["tables"][0]["size_hint"], 2)
        self.assertEqual(resized["boxes"][0]["tables"][0]["size_hint"], 4)
        self.assertEqual(resized["boxes"][0]["tables"][1]["size_hint"], 7)
        self.assertEqual(
            [table["name"] for table in resized["boxes"][0]["tables"]],
            ["Area", "Person"],
        )

    def test_bad_column_does_not_replace_an_existing_artifact(self):
        model = state_artifact.load_model(MODEL)
        columns = refs_columns()
        columns[("world", "Person", "area")] = [0, 2, 0]
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "refs.state"
            output.write_bytes(b"previous")
            with self.assertRaisesRegex(ValueError, "integer in \\[0, 1\\]"):
                state_artifact.write_state(output, model, columns)
            self.assertEqual(output.read_bytes(), b"previous")

    def test_missing_and_short_columns_are_rejected(self):
        model = state_artifact.load_model(MODEL)
        missing = refs_columns()
        del missing[("world", "Person", "age")]
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "missing.state"
            with self.assertRaisesRegex(ValueError, "missing="):
                state_artifact.write_state(output, model, missing)

            short = refs_columns()
            short[("world", "Person", "age")] = [10, 20]
            with self.assertRaisesRegex(ValueError, "has 2 values; expected 3"):
                state_artifact.write_state(output, model, short)
            self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()

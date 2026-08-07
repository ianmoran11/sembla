"""Contract tests for the theta vector and draw stream."""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import theta  # noqa: E402

PARAMS = theta.PARAMS
GRAVITY = theta.GRAVITY


class TestThetaIsExactlyTheFreeParameters(unittest.TestCase):
    def test_theta_has_seventeen_members_named_by_the_registry(self):
        names = theta.free_parameters()
        self.assertEqual(len(names), 17)
        self.assertEqual(tuple(sorted(names)), names)
        self.assertIn("interstate_base", names)
        self.assertIn("peak_months", names)
        self.assertIn("k", names)
        self.assertNotIn("push_nsw", names)
        self.assertNotIn("pull_nsw", names)

    def test_registry_lists_no_other_free_parameters(self):
        priors = json.loads((PARAMS / "priors.json").read_text(encoding="utf-8"))
        classified = sum(
            entry["classification"] == "free"
            for entry in priors["parameters"].values()
        )
        self.assertEqual(classified, 17)
        self.assertEqual(priors["classification_counts"]["free"], 17)
        self.assertEqual(priors["classification_counts"]["fixed"], 360)


class TestDrawsHoldFixedParametersAtTheirAnnualValues(unittest.TestCase):
    def test_no_fixed_parameter_varies_across_any_draw(self):
        names = set(theta.free_parameters())
        centre = json.loads((GRAVITY / "2010.json").read_text(encoding="utf-8"))
        assignments = theta.build_draws(
            centre,
            draws=8,
            key=theta.draw_key("identity", "hundredth", 2010, "digest", 8, "test"),
            spreads=theta.prior_spreads(),
            names=theta.free_parameters(),
        )
        for assignment in assignments:
            self.assertEqual(set(assignment), set(centre))
            for name, value in centre.items():
                if name not in names:
                    self.assertEqual(assignment[name], value)

    def test_every_free_parameter_varies_across_draws(self):
        centre = json.loads((GRAVITY / "2010.json").read_text(encoding="utf-8"))
        assignments = theta.build_draws(
            centre,
            draws=16,
            key=theta.draw_key("identity", "hundredth", 2010, "digest", 16, "test"),
            spreads=theta.prior_spreads(),
            names=theta.free_parameters(),
        )
        for name in theta.free_parameters():
            values = {assignment[name] for assignment in assignments}
            self.assertGreater(len(values), 1, f"{name} never moved")
            self.assertTrue(all(value > 0 for value in values))

    def test_draws_are_deterministic_and_coordinate_sensitive(self):
        centre = json.loads((GRAVITY / "2010.json").read_text(encoding="utf-8"))
        names = theta.free_parameters()
        spreads = theta.prior_spreads()

        def stream(year: int, purpose: str):
            key = theta.draw_key("identity", "hundredth", year, "digest", 4, purpose)
            return theta.build_draws(centre, draws=4, key=key, spreads=spreads, names=names)

        self.assertEqual(stream(2010, "a"), stream(2010, "a"))
        self.assertNotEqual(stream(2010, "a"), stream(2011, "a"))
        self.assertNotEqual(stream(2010, "a"), stream(2010, "b"))


class TestDrawFileShape(unittest.TestCase):
    def test_write_draws_emits_full_assignments_and_a_record(self):
        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / "theta.json"
            record = theta.write_draws(
                out,
                2010,
                draws=3,
                model_identity="identity",
                scale="hundredth",
            )
            assignments = json.loads(out.read_text(encoding="utf-8"))
            self.assertEqual(len(assignments), 3)
            self.assertEqual(len(assignments[0]), 377)
            self.assertEqual(record["draws"], 3)
            self.assertEqual(record["free_parameters"], list(theta.free_parameters()))
            self.assertEqual(record["fixed_parameter_count"], 360)
            self.assertEqual(len(record["theta_file_sha256"]), 64)

    def test_write_draws_refuses_a_missing_free_slot(self):
        with tempfile.TemporaryDirectory() as directory:
            broken = pathlib.Path(directory)
            centre = json.loads((GRAVITY / "2010.json").read_text(encoding="utf-8"))
            del centre["push_vic"]
            (broken / "2010.json").write_text(json.dumps(centre), encoding="utf-8")
            with self.assertRaises(theta.ThetaError):
                theta.write_draws(
                    broken / "theta.json",
                    2010,
                    draws=2,
                    model_identity="identity",
                    scale="hundredth",
                    params_dir=broken,
                )


if __name__ == "__main__":
    unittest.main()

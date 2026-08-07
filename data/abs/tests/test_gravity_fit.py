"""Contract tests for the offline Poisson gravity fit."""

from __future__ import annotations

import json
import math
import pathlib
import sys
import unittest

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))

import gravity_fit  # noqa: E402

ROOT = HERE.parent.parent.parent
PARAMS = gravity_fit.PARAMS
FITTED = gravity_fit.FITTED
MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"


def _report() -> dict:
    return json.loads((FITTED / "fit-report.json").read_text(encoding="utf-8"))


class TestGravityIdentification(unittest.TestCase):
    def test_reference_state_is_normalised_away(self):
        report = _report()
        self.assertEqual(report["reference_state"], "nsw")
        for year in report["years"]:
            self.assertNotIn("push_nsw", year["values"])
            self.assertNotIn("pull_nsw", year["values"])
            self.assertEqual(len(year["values"]), gravity_fit.PARAMETER_COUNT)

    def test_residual_degrees_of_freedom_are_fifty_six_minus_fifteen(self):
        report = _report()
        self.assertEqual(report["cell_count"], 56)
        self.assertEqual(report["spatial_parameter_count"], 15)
        self.assertEqual(report["residual_degrees_of_freedom"], 41)
        for year in report["years"]:
            self.assertEqual(len(year["cells"]), 56)
            self.assertEqual(
                len({(cell["origin"], cell["destination"]) for cell in year["cells"]}),
                56,
            )
            for cell in year["cells"]:
                self.assertNotEqual(cell["origin"], cell["destination"])

    def test_age_profile_parameters_are_held_at_their_prior_centres(self):
        report = _report()
        priors = json.loads((PARAMS / "priors.json").read_text(encoding="utf-8"))
        self.assertEqual(sorted(report["held_at_prior_centre"]), ["k", "peak_months"])
        for name in ("k", "peak_months"):
            self.assertEqual(
                report["age_profile_centres"][name],
                priors["parameters"][name]["precalibration_value"],
            )
        for year in report["years"]:
            self.assertNotIn("k", year["values"])
            self.assertNotIn("peak_months", year["values"])

    def test_age_profile_matches_the_compiled_model_expression(self):
        model = json.loads(MODEL.read_text(encoding="utf-8"))
        transitions = model["boxes"][0]["transitions"]
        move = next(
            transition
            for transition in transitions
            if transition["name"] == "move_nsw_vic"
        )

        def evaluate(node: dict, age_months: int, values: dict[str, float]) -> float:
            kind = node["kind"]
            if kind == "real":
                return node["value"]
            if kind == "param":
                return values[node["name"]]
            if kind == "self_attr":
                self.assertEqual(node["name"], "age_months")
                return float(age_months)
            if kind == "add":
                return evaluate(node["lhs"], age_months, values) + evaluate(
                    node["rhs"], age_months, values
                )
            if kind == "sub":
                return evaluate(node["lhs"], age_months, values) - evaluate(
                    node["rhs"], age_months, values
                )
            if kind == "mul":
                return evaluate(node["lhs"], age_months, values) * evaluate(
                    node["rhs"], age_months, values
                )
            if kind == "div":
                return evaluate(node["lhs"], age_months, values) / evaluate(
                    node["rhs"], age_months, values
                )
            raise AssertionError(f"unexpected expression node {kind}")

        report = _report()
        peak = report["age_profile_centres"]["peak_months"]
        k = report["age_profile_centres"]["k"]
        values = {
            "interstate_base": 1.0,
            "push_vic": 1.0,
            "pull_vic": 1.0,
            "peak_months": peak,
            "k": k,
        }
        for age_months in (0, 1, 199, 360, 361, 780, 1212):
            self.assertAlmostEqual(
                evaluate(move["hazard"], age_months, values),
                gravity_fit.age_profile(age_months, peak, k),
                places=15,
            )


class TestGravityFitIsMaximumLikelihood(unittest.TestCase):
    def test_fitted_margins_equal_the_published_table_margins(self):
        report = _report()
        for year in report["years"]:
            for row in year["margin_comparison"]:
                self.assertAlmostEqual(
                    row["fitted_departures"], row["table_departures"], delta=1e-6
                )
                self.assertAlmostEqual(
                    row["fitted_arrivals"], row["table_arrivals"], delta=1e-6
                )

    def test_every_year_converged_with_a_vanishing_gradient(self):
        report = _report()
        for year in report["years"]:
            self.assertLess(year["iterations"], gravity_fit.MAXIMUM_ITERATIONS)
            self.assertLess(year["maximum_absolute_gradient"], 1e-6)

    def test_deviance_is_the_sum_of_its_reported_cell_contributions(self):
        report = _report()
        for year in report["years"]:
            total = sum(cell["deviance_contribution"] for cell in year["cells"])
            self.assertAlmostEqual(year["deviance"], total, places=6)
            self.assertAlmostEqual(
                year["deviance_per_degree_of_freedom"],
                year["deviance"] / 41,
                places=9,
            )
            for cell in year["cells"]:
                self.assertAlmostEqual(
                    cell["signed_residual"],
                    cell["observed"] - cell["expected"],
                    places=6,
                )
                self.assertAlmostEqual(
                    cell["pearson_residual"],
                    (cell["observed"] - cell["expected"]) / math.sqrt(cell["expected"]),
                    places=9,
                )

    def test_separable_gravity_is_rejected_rather_than_assumed(self):
        report = _report()
        for year in report["years"]:
            self.assertGreater(
                year["deviance_per_degree_of_freedom"],
                1.0,
                "a separable push-times-pull table would fit near one per degree "
                "of freedom; this evidence must keep recording that it does not",
            )


class TestPublishedConflictsSurvive(unittest.TestCase):
    def test_the_2020_table_and_margin_vintages_still_disagree(self):
        report = _report()
        year = next(row for row in report["years"] if row["run_year"] == 2020)
        by_state = {row["state"]: row for row in year["margin_comparison"]}
        self.assertEqual(by_state["nsw"]["table_minus_margin_arrivals"], 27_626)
        self.assertEqual(
            sum(
                row["table_minus_margin_departures"]
                for row in year["margin_comparison"]
            ),
            43_250,
        )

    def test_later_years_close_against_their_margins(self):
        report = _report()
        for run_year in (2021, 2022, 2023, 2024):
            year = next(row for row in report["years"] if row["run_year"] == run_year)
            for field in (
                "table_minus_margin_arrivals",
                "table_minus_margin_departures",
            ):
                self.assertEqual(
                    sum(row[field] for row in year["margin_comparison"]), 0
                )


class TestFittedParameterFiles(unittest.TestCase):
    def test_only_the_fifteen_spatial_free_slots_move(self):
        priors = json.loads((PARAMS / "priors.json").read_text(encoding="utf-8"))
        free = set(priors["free_parameters"])
        report = _report()
        for year in gravity_fit.RUN_YEARS:
            baseline = json.loads(
                (PARAMS / f"{year}.json").read_text(encoding="utf-8")
            )
            fitted = json.loads(
                (FITTED / f"{year}.json").read_text(encoding="utf-8")
            )
            self.assertEqual(set(fitted), set(baseline))
            self.assertEqual(len(fitted), priors["parameter_count"])
            moved = {
                name
                for name, value in fitted.items()
                if value != baseline[name]
            }
            self.assertTrue(moved <= free)
            self.assertNotIn("k", moved)
            self.assertNotIn("peak_months", moved)
            expected = next(
                row for row in report["years"] if row["run_year"] == year
            )["values"]
            for name, value in expected.items():
                self.assertEqual(fitted[name], value)

    def test_committed_files_match_a_fresh_fit(self):
        report = gravity_fit.fit_all_years()
        self.assertEqual(report, _report())
        parameters = gravity_fit.annual_parameters(report)
        for year, values in parameters.items():
            committed = json.loads(
                (FITTED / f"{year}.json").read_text(encoding="utf-8")
            )
            self.assertEqual(values, committed)


class TestExposure(unittest.TestCase):
    def test_exposure_spans_twelve_ticks_of_ageing(self):
        flat = gravity_fit._age_exposure(30, 360.0, 0.0)
        self.assertAlmostEqual(flat, 12.0, places=12)

    def test_exposure_peaks_at_the_profile_peak(self):
        peak = gravity_fit._age_exposure(30, 360.0, 1e-05)
        older = gravity_fit._age_exposure(70, 360.0, 1e-05)
        younger = gravity_fit._age_exposure(0, 360.0, 1e-05)
        self.assertGreater(peak, older)
        self.assertGreater(peak, younger)

    def test_a_missing_published_year_is_refused(self):
        with self.assertRaises(gravity_fit.GravityFitError):
            gravity_fit.fit_cells({}, {state: 1.0 for state in gravity_fit.STATES})


if __name__ == "__main__":
    unittest.main()

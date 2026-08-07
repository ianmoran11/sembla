"""Tests for deterministic ABS-derived annual parameter generation."""

from __future__ import annotations

import csv
import hashlib
import json
import math
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import rates  # noqa: E402


HERE = pathlib.Path(__file__).resolve().parent
EXTRACTS = HERE.parent / "extracts"
PARAMS = HERE.parent / "params"


class TestRateInputs(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = rates.load_inputs()

    def test_complete_source_grids(self):
        self.assertEqual(len(self.inputs.births), 15 * 8)
        self.assertEqual(len(self.inputs.national_births), 15)
        self.assertEqual(len(self.inputs.mortality), 15 * 8 * 2 * 21)
        self.assertEqual(len(self.inputs.national_mortality), 15 * 2 * 21)
        self.assertEqual(len(self.inputs.deaths), 15 * 8 * 2 * 22)
        self.assertEqual(len(self.inputs.overseas), 15 * 8)
        self.assertEqual(len(self.inputs.erp), 16 * 8)
        self.assertEqual(len(self.inputs.life_tables), 5 * 8 * 2 * 101)

    def test_exactly_two_source_fallbacks(self):
        actual = {
            key for key, value in self.inputs.mortality.items() if value is None
        }
        self.assertEqual(actual, set(rates.FALLBACK_CELLS))
        self.assertEqual(
            float(self.inputs.national_mortality[(2010, "female", "100+")]),
            433.9,
        )
        self.assertEqual(
            float(self.inputs.national_mortality[(2011, "female", "100+")]),
            450.0,
        )

    def _mutated_extracts(self, mutate):
        temporary = tempfile.TemporaryDirectory()
        destination = pathlib.Path(temporary.name) / "extracts"
        shutil.copytree(EXTRACTS, destination)
        path = destination / "mortality_rates_state_age_sex.csv"
        with path.open(newline="", encoding="utf-8") as source:
            reader = csv.DictReader(source)
            rows = list(reader)
            header = reader.fieldnames
        mutate(rows)
        with path.open("w", newline="", encoding="utf-8") as target:
            writer = csv.DictWriter(target, fieldnames=header, lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
        return temporary, destination

    def test_missing_mortality_cell_fails_instead_of_becoming_zero(self):
        temporary, extracts = self._mutated_extracts(lambda rows: rows.pop(0))
        with temporary:
            with self.assertRaisesRegex(ValueError, "mortality grid changed"):
                rates.load_inputs(extracts)

    def test_third_fallback_is_rejected(self):
        def mutate(rows):
            row = next(
                row
                for row in rows
                if row["year"] == "2010"
                and row["state"] == "act"
                and row["sex"] == "female"
                and row["age_band"] == "0-4"
            )
            row["rate_per_1000"] = ""
            row["status"] = "not_published_zero_exposure"

        temporary, extracts = self._mutated_extracts(mutate)
        with temporary:
            with self.assertRaisesRegex(ValueError, "unexpected mortality fallback"):
                rates.load_inputs(extracts)


class TestRateDerivation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.artifacts = rates.derive_rates()

    def test_exact_entry_hazard_closes_expected_flow(self):
        row = rates.entry_calculation("test", 2010, "nsw", 10_000, 1_200)
        expected_hazard = -math.log1p(-1_200 / 10_000) / 12
        self.assertAlmostEqual(row.monthly_hazard, expected_hazard, places=15)
        expected_flow = (
            row.average_expected_vacancies
            * 12
            * (1 - math.exp(-row.monthly_hazard))
        )
        self.assertAlmostEqual(expected_flow, 1_200, places=9)
        self.assertEqual(row.end_vacancies, 8_800)

    def test_invalid_entry_exposures_fail_loudly(self):
        for vacancies, entries, message in [
            (0, 1, "no vacant slots"),
            (100, -1, "negative entries"),
            (100, 100, "exhausts"),
            (100, 101, "exhausts"),
        ]:
            with self.subTest(vacancies=vacancies, entries=entries):
                with self.assertRaisesRegex(ValueError, message):
                    rates.entry_calculation(
                        "test", 2010, "act", vacancies, entries
                    )

    def test_mortality_conversion_is_rate_divided_by_12000(self):
        # Hand-computed source fixture: ACT female 0-4 is 0.7 per 1,000 in 2010.
        self.assertEqual(
            self.artifacts.annual_params[2010]["mortality_act_00_04_female"],
            0.7 / 12_000,
        )
        # Published rounded zero remains an exact zero hazard.
        self.assertEqual(
            self.artifacts.annual_params[2010]["mortality_act_05_09_female"],
            0.0,
        )

    def test_fallback_values_and_provenance_are_exact(self):
        self.assertEqual(
            self.artifacts.annual_params[2010]["mortality_nt_100_plus_female"],
            433.9 / 12_000,
        )
        self.assertEqual(
            self.artifacts.annual_params[2011]["mortality_nt_100_plus_female"],
            450.0 / 12_000,
        )
        self.assertEqual(len(self.artifacts.fallback_provenance), 2)
        self.assertEqual(
            {row["provenance"] for row in self.artifacts.fallback_provenance},
            {"national_zero_exposure_fallback"},
        )

    def test_every_mortality_mapping_matches_its_published_or_fallback_cell(self):
        for year in rates.RUN_YEARS:
            for state in rates.MODEL_STATES:
                for source_band, model_band in rates.AGE_BANDS:
                    for sex in rates.SEXES:
                        source = self.artifacts.inputs.mortality[
                            (year, state, sex, source_band)
                        ]
                        if source is None:
                            source = self.artifacts.inputs.national_mortality[
                                (year, sex, source_band)
                            ]
                        actual = self.artifacts.annual_params[year][
                            f"mortality_{state}_{model_band}_{sex}"
                        ]
                        self.assertEqual(actual, float(source / 12_000))

    def test_every_annual_file_fully_specifies_the_model(self):
        expected = rates.expected_parameter_names()
        self.assertEqual(len(expected), 377)
        for year in rates.RUN_YEARS:
            with self.subTest(year=year):
                params = self.artifacts.annual_params[year]
                self.assertEqual(set(params), expected)
                self.assertEqual(len(params), 377)
                self.assertTrue(
                    all(math.isfinite(value) and value >= 0 for value in params.values())
                )
                self.assertEqual(
                    {name: params[name] for name in rates.FREE_DEFAULTS},
                    rates.FREE_DEFAULTS,
                )

    def test_full_scale_vacancy_paths_retain_exact_headroom(self):
        self.assertEqual(sum(self.artifacts.birth_capacities.values()), 5_008_051)
        self.assertEqual(sum(self.artifacts.overseas_capacities.values()), 8_209_168)
        last = {
            (row.stream, row.state): row
            for row in self.artifacts.entry_calculations
            if row.year == 2024
        }
        self.assertEqual(
            sum(last[("birth", state)].end_vacancies for state in rates.MODEL_STATES),
            455_278,
        )
        self.assertEqual(
            sum(
                last[("overseas", state)].end_vacancies
                for state in rates.MODEL_STATES
            ),
            746_288,
        )
        self.assertTrue(all(row.end_vacancies > 0 for row in last.values()))

    def test_entry_hazards_are_derived_from_full_scale_counts(self):
        row = next(
            row
            for row in self.artifacts.entry_calculations
            if (row.stream, row.year, row.state) == ("birth", 2010, "nsw")
        )
        self.assertEqual(row.start_vacancies, 1_611_520)
        self.assertEqual(row.target_entries, 101_266)
        self.assertEqual(
            row.monthly_hazard,
            -math.log1p(-row.target_entries / row.start_vacancies) / 12,
        )

    def test_life_tables_are_validation_only_and_complete(self):
        self.assertEqual(len(self.artifacts.life_table_metrics), 5)
        self.assertEqual(
            [(row.period_start, row.period_end) for row in self.artifacts.life_table_metrics],
            [(2018, 2020), (2019, 2021), (2020, 2022), (2021, 2023), (2022, 2024)],
        )
        self.assertTrue(all(row.cells == 8 * 2 * 101 for row in self.artifacts.life_table_metrics))


class TestRateArtifacts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.artifacts = rates.derive_rates()
        cls.priors = rates.priors_payload(cls.artifacts)

    def test_prior_registry_is_exhaustive_and_has_exact_free_set(self):
        self.assertEqual(self.priors["parameter_count"], 377)
        self.assertEqual(
            self.priors["classification_counts"],
            {"fixed": 360, "free": 17, "fixed_normalization": 0},
        )
        self.assertEqual(
            set(self.priors["free_parameters"]), set(rates.FREE_DEFAULTS)
        )
        free_groups = {
            item["identification"]["group"]
            for item in self.priors["parameters"].values()
            if item["classification"] == "free"
        }
        self.assertEqual(
            free_groups, {"interstate_base", "push_o", "pull_d", "peak", "k"}
        )

    def test_zero_2010_mortality_defaults_have_the_only_normal_priors(self):
        normal = {
            name
            for name, item in self.priors["parameters"].items()
            if item["lean_prior_2010"]["family"] == "normal"
        }
        expected = {
            name
            for name, value in self.artifacts.annual_params[2010].items()
            if name.startswith("mortality_") and value == 0
        }
        self.assertEqual(normal, expected)
        self.assertEqual(len(normal), 7)
        for name in normal:
            self.assertEqual(
                self.priors["parameters"][name]["lean_prior_2010"]["spread"],
                0.05 / 12_000,
            )

    def test_generation_is_byte_reproducible_and_matches_committed_files(self):
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first = pathlib.Path(first)
            second = pathlib.Path(second)
            for directory in (first, second):
                rates.write_artifacts(
                    self.artifacts,
                    directory / "params",
                    directory / "rates.md",
                    rates.FIDELITY_EVIDENCE,
                )
            first_files = {
                path.relative_to(first): path.read_bytes()
                for path in first.rglob("*")
                if path.is_file()
            }
            second_files = {
                path.relative_to(second): path.read_bytes()
                for path in second.rglob("*")
                if path.is_file()
            }
            self.assertEqual(first_files, second_files)
            for relative, content in first_files.items():
                committed = (
                    EXTRACTS / "rates.md"
                    if relative == pathlib.Path("rates.md")
                    else PARAMS / relative.relative_to("params")
                )
                self.assertEqual(content, committed.read_bytes(), str(committed))

    def test_annual_files_are_flat_numeric_cli_parameter_objects(self):
        for year in rates.RUN_YEARS:
            with self.subTest(year=year):
                payload = json.loads((PARAMS / f"{year}.json").read_text())
                self.assertEqual(set(payload), rates.expected_parameter_names())
                self.assertTrue(
                    all(type(value) in (int, float) for value in payload.values())
                )

    def test_fidelity_evidence_uses_the_predeclared_rule_and_passes(self):
        evidence = json.loads(rates.FIDELITY_EVIDENCE.read_text())
        self.assertEqual(evidence["status"], "measured")
        self.assertEqual(evidence["pilot_seeds"], list(range(1001, 1011)))
        self.assertEqual(evidence["held_out_seed"], 2001)
        self.assertEqual(
            evidence["tolerance_rule"],
            "ceil(3 * sample_standard_deviation)",
        )
        predeclaration = rates.FIDELITY_PREDECLARATION.read_bytes()
        self.assertEqual(
            hashlib.sha256(predeclaration).hexdigest(),
            rates.FIDELITY_PREDECLARATION_SHA256,
        )
        self.assertEqual(
            evidence["predeclaration_path"],
            rates.FIDELITY_PREDECLARATION.name,
        )
        self.assertEqual(
            evidence["predeclaration_sha256"],
            rates.FIDELITY_PREDECLARATION_SHA256,
        )
        self.assertEqual(evidence["targets"], {"births": 303_299, "deaths": 143_451})
        self.assertTrue(evidence["held_out"]["births"]["pass"])
        self.assertTrue(evidence["held_out"]["deaths"]["pass"])
        self.assertEqual(evidence["statistics"]["births"]["tolerance"], 17_363)
        self.assertEqual(evidence["statistics"]["deaths"]["tolerance"], 7_901)
        report = (EXTRACTS / "rates.md").read_text()
        self.assertIn("pilot sample SD", report)
        self.assertIn("| births | 303,299 |", report)
        self.assertIn("| deaths | 143,451 |", report)
        self.assertIn("includes 9 published `not_stated` deaths", report)


if __name__ == "__main__":
    unittest.main()

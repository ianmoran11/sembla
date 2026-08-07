"""Tests for the committed extracts and the fetch safety contract.

These run offline against the committed extracts and must never touch the
network.
"""

from __future__ import annotations

import hashlib
import json
import pathlib
import shutil
import sys
import tempfile
import unittest
import urllib.request

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import canonical  # noqa: E402
import fetch  # noqa: E402
import normalise  # noqa: E402
import reconcile  # noqa: E402
import sdmx_csv  # noqa: E402

EXTRACTS = pathlib.Path(__file__).resolve().parent.parent / "extracts"

FIRST_YEAR, LAST_YEAR = 2010, 2025


class TestErpExtract(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(EXTRACTS / "erp_state_age_sex.csv")

    def test_schema(self):
        self.assertEqual(self.header, ["year", "state", "sex", "age", "persons"])

    def test_complete_cross_product(self):
        years = LAST_YEAR - FIRST_YEAR + 1
        expected = years * len(normalise.STATE_CODES) * 2 * (normalise.TERMINAL_AGE + 1)
        self.assertEqual(len(self.rows), expected)
        keys = {(r[0], r[1], r[2], r[3]) for r in self.rows}
        self.assertEqual(len(keys), expected, "duplicate key tuple present")

    def test_domains(self):
        self.assertEqual({r[1] for r in self.rows}, set(normalise.STATE_CODES))
        self.assertEqual({r[2] for r in self.rows}, {"male", "female"})
        self.assertEqual(
            {int(r[3]) for r in self.rows}, set(range(normalise.TERMINAL_AGE + 1))
        )
        self.assertTrue(all(int(r[4]) >= 0 for r in self.rows))

    def test_rows_are_sorted(self):
        keyed = [(int(r[0]), r[1], r[2], int(r[3])) for r in self.rows]
        self.assertEqual(keyed, sorted(keyed))

    def test_re_emission_is_byte_identical(self):
        """Round-tripping the extract through the canonical writer is a no-op."""
        import tempfile

        original = (EXTRACTS / "erp_state_age_sex.csv").read_bytes()
        rows = [(int(r[0]), r[1], r[2], int(r[3]), int(r[4])) for r in self.rows]
        with tempfile.TemporaryDirectory() as d:
            p = pathlib.Path(d) / "again.csv"
            canonical.write_csv(p, self.header, rows)
            self.assertEqual(p.read_bytes(), original)

    def test_known_published_totals(self):
        """Anchor the parse against independently published ABS headline figures."""
        totals = {}
        for year, state, _sex, _age, persons in self.rows:
            totals[int(year)] = totals.get(int(year), 0) + int(persons)
        # 30 June 2010 and 30 June 2025 eight-state sums, which sit just below
        # published Australian ERP by the Other Territories population.
        self.assertEqual(totals[2010], 22_028_695)
        self.assertEqual(totals[2025], 27_606_008)
        for year in range(FIRST_YEAR, LAST_YEAR):
            self.assertLess(totals[year], totals[year + 1],
                            f"population fell from {year} to {year + 1}")


class TestRegionalCrosscheck(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "erp_regional_2010_state_age_sex.csv"
        )

    def test_schema_and_cross_product(self):
        self.assertEqual(
            self.header, ["year", "state", "sex", "age_band", "persons"]
        )
        self.assertEqual(len(self.rows), 8 * 2 * 18)
        self.assertEqual({int(r[0]) for r in self.rows}, {2010})
        self.assertEqual({r[1] for r in self.rows}, set(normalise.STATE_CODES))
        self.assertEqual({r[2] for r in self.rows}, {"male", "female"})

    def test_sa2_sums_exactly_reproduce_state_erp(self):
        regional = {
            (state, sex, age_band): int(persons)
            for _year, state, sex, age_band, persons in self.rows
        }
        _, state_rows = canonical.read_csv(EXTRACTS / "erp_state_age_sex.csv")
        state = {}
        for year, code, sex, age, persons in state_rows:
            if year != "2010":
                continue
            value = int(age)
            age_band = "85+" if value >= 85 else f"{value // 5 * 5}-{value // 5 * 5 + 4}"
            key = (code, sex, age_band)
            state[key] = state.get(key, 0) + int(persons)
        self.assertEqual(regional, state)


class TestNationalExtract(unittest.TestCase):
    def test_national_never_below_state_sum(self):
        _, srows = canonical.read_csv(EXTRACTS / "erp_state_age_sex.csv")
        _, nrows = canonical.read_csv(EXTRACTS / "erp_national_age_sex.csv")
        state = {}
        for y, s, sex, a, n in srows:
            state[(int(y), sex, int(a))] = state.get((int(y), sex, int(a)), 0) + int(n)
        for y, _r, sex, a, n in nrows:
            key = (int(y), sex, int(a))
            self.assertLessEqual(
                state.get(key, 0), int(n),
                f"eight states exceed national at {key}",
            )


class TestFlowExtracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.comp_header, cls.comp = canonical.read_csv(
            EXTRACTS / "components_state.csv")
        cls.marg_header, cls.marg = canonical.read_csv(
            EXTRACTS / "interstate_margins.csv")
        cls.od_header, cls.od = canonical.read_csv(
            EXTRACTS / "interstate_flows.csv")
        cls.nat_header, cls.nat = canonical.read_csv(
            EXTRACTS / "components_national.csv")

    def test_schemas(self):
        self.assertEqual(self.comp_header, [
            "run_year", "state", "natural_increase",
            "net_overseas_migration", "net_interstate_migration"])
        self.assertEqual(self.marg_header,
                         ["run_year", "state", "arrivals", "departures"])
        self.assertEqual(self.od_header,
                         ["year", "origin", "destination", "persons"])
        self.assertEqual(self.nat_header, [
            "run_year", "births", "deaths",
            "overseas_arrivals", "overseas_departures"])

    def test_run_years_are_complete(self):
        """Fifteen run years, 2010 to 2024, carrying stocks through 2025."""
        expected = set(range(FIRST_YEAR, LAST_YEAR))
        for rows in (self.comp, self.marg):
            self.assertEqual({int(r[0]) for r in rows}, expected)
            self.assertEqual(len(rows), len(expected) * len(normalise.STATE_CODES))
        self.assertEqual({int(r[0]) for r in self.nat}, expected)

    def test_partial_years_are_dropped(self):
        """A run year is kept only when all four of its quarters are present."""
        self.assertNotIn(LAST_YEAR, {int(r[0]) for r in self.comp})

    def test_interstate_od_is_complete(self):
        expected_years = set(range(FIRST_YEAR, LAST_YEAR))
        self.assertEqual({int(r[0]) for r in self.od}, expected_years)
        self.assertEqual(len(self.od), len(expected_years) * 8 * 7)
        keys = set()
        for year, origin, destination, persons in self.od:
            self.assertIn(origin, normalise.STATE_CODES)
            self.assertIn(destination, normalise.STATE_CODES)
            self.assertNotEqual(origin, destination)
            self.assertGreaterEqual(int(persons), 0)
            keys.add((int(year), origin, destination))
        self.assertEqual(len(keys), len(self.od))
        values = {(r[0], r[1], r[2]): int(r[3]) for r in self.od}
        self.assertEqual(values[("2010", "nsw", "vic")], 23_194)
        self.assertEqual(values[("2010", "vic", "nsw")], 20_171)

    def test_od_and_separate_margins_have_documented_revision_differences(self):
        arrivals, departures = {}, {}
        for year, origin, destination, persons in self.od:
            y, value = int(year), int(persons)
            arrivals[(y, destination)] = arrivals.get((y, destination), 0) + value
            departures[(y, origin)] = departures.get((y, origin), 0) + value
        margins = {
            (int(year), state): (int(a), int(d))
            for year, state, a, d in self.marg
        }
        worst = {}
        for key, (published_a, published_d) in margins.items():
            year = key[0]
            error = max(
                abs(arrivals[key] - published_a),
                abs(departures[key] - published_d),
            )
            worst[year] = max(worst.get(year, 0), error)
        for year in range(2016, 2025):
            if year != 2020:
                self.assertEqual(worst[year], 0)
        self.assertLessEqual(max(worst[y] for y in range(2010, 2016)), 500)
        self.assertEqual(worst[2020], 27_626)

    def test_interstate_margins_agree_with_net(self):
        net = {(int(r[0]), r[1]): int(r[4]) for r in self.comp}
        for run_year, state, arrivals, departures in self.marg:
            self.assertEqual(
                int(arrivals) - int(departures), net[(int(run_year), state)],
                f"margins disagree with net at {run_year} {state}")

    def test_interstate_net_closes_across_states(self):
        """The residual is Other Territories exchange plus rounding: small."""
        by_year = {}
        for r in self.comp:
            by_year[int(r[0])] = by_year.get(int(r[0]), 0) + int(r[4])
        for year, total in by_year.items():
            self.assertLessEqual(abs(total), 200, f"{year} net interstate {total}")

    def test_stock_flow_residual_vanishes_after_the_census_rebase(self):
        """Post-2021 estimates are carried forward from the components."""
        _, srows = canonical.read_csv(EXTRACTS / "erp_state_age_sex.csv")
        pop = {}
        for y, s, _sex, _a, n in srows:
            pop[(int(y), s)] = pop.get((int(y), s), 0) + int(n)
        comp = {(int(r[0]), r[1]): (int(r[2]), int(r[3]), int(r[4]))
                for r in self.comp}
        for (run_year, state), (ni, nom, nim) in comp.items():
            if run_year < 2021:
                continue
            change = pop[(run_year + 1, state)] - pop[(run_year, state)]
            self.assertEqual(
                change, ni + nom + nim,
                f"identity should close exactly at {run_year} {state}")


class TestInterstateAgeSex(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "interstate_state_age_sex.csv"
        )

    def test_schema_and_complete_cross_product(self):
        self.assertEqual(
            self.header,
            ["year", "state", "sex", "age_band", "arrivals", "departures"],
        )
        expected = (
            (LAST_YEAR - FIRST_YEAR)
            * len(normalise.STATE_CODES)
            * len(sdmx_csv.SEX_BY_CODE)
            * len(sdmx_csv.NIM_AGE_BAND)
        )
        self.assertEqual(len(self.rows), expected)
        self.assertEqual({int(r[0]) for r in self.rows},
                         set(range(FIRST_YEAR, LAST_YEAR)))

    def test_detailed_sums_reproduce_separate_margins(self):
        detailed = {}
        for year, state, _sex, _age, arrivals, departures in self.rows:
            key = (int(year), state)
            sums = detailed.setdefault(key, [0, 0])
            sums[0] += int(arrivals)
            sums[1] += int(departures)
            self.assertGreaterEqual(int(arrivals), 0)
            self.assertGreaterEqual(int(departures), 0)
        _, margins = canonical.read_csv(EXTRACTS / "interstate_margins.csv")
        for year, state, arrivals, departures in margins:
            actual = detailed[(int(year), state)]
            self.assertLessEqual(abs(actual[0] - int(arrivals)), 100)
            self.assertLessEqual(abs(actual[1] - int(departures)), 100)


class TestOverseasMargins(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(EXTRACTS / "overseas_margins.csv")

    def test_schema_and_completeness(self):
        self.assertEqual(self.header,
                         ["run_year", "state", "arrivals", "departures"])
        expected = set(range(FIRST_YEAR, LAST_YEAR))
        self.assertEqual({int(r[0]) for r in self.rows}, expected)
        self.assertEqual(len(self.rows), len(expected) * len(normalise.STATE_CODES))

    def test_flows_are_non_negative(self):
        for _y, _s, arrivals, departures in self.rows:
            self.assertGreaterEqual(int(arrivals), 0)
            self.assertGreaterEqual(int(departures), 0)

    def test_agrees_with_published_net_in_settled_years(self):
        """Two independently published releases must agree bar rounding."""
        _, comp = canonical.read_csv(EXTRACTS / "components_state.csv")
        net = {(int(r[0]), r[1]): int(r[3]) for r in comp}
        for run_year, state, arrivals, departures in self.rows:
            year = int(run_year)
            if year > 2022:
                continue  # preliminary in both releases, revised separately
            implied = int(arrivals) - int(departures)
            self.assertLess(
                abs(implied - net[(year, state)]), 1000,
                f"overseas net disagrees at {year} {state}")


class TestBirthsState(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(EXTRACTS / "births_state.csv")

    def test_schema_and_complete_registration_years(self):
        self.assertEqual(self.header, ["year", "state", "births"])
        expected = set(range(FIRST_YEAR, LAST_YEAR))
        self.assertEqual({int(r[0]) for r in self.rows}, expected)
        self.assertEqual(len(self.rows), len(expected) * len(normalise.STATE_CODES))
        self.assertEqual({r[1] for r in self.rows}, set(normalise.STATE_CODES))

    def test_counts_and_published_anchors(self):
        totals = {}
        for year, _state, births in self.rows:
            self.assertGreaterEqual(int(births), 0)
            totals[int(year)] = totals.get(int(year), 0) + int(births)
        # Eight-state sums sit just below the Australian totals because the
        # latter include Other Territories: 303,318 and 292,318 respectively.
        self.assertEqual(totals[2010], 303_299)
        self.assertEqual(totals[2024], 292_294)


class TestBirthsSex(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(EXTRACTS / "births_sex.csv")
        _, cls.state_rows = canonical.read_csv(EXTRACTS / "births_state.csv")

    def test_schema_and_complete_national_sex_grid(self):
        self.assertEqual(self.header, ["year", "sex", "births"])
        expected_years = set(range(FIRST_YEAR, LAST_YEAR))
        self.assertEqual({int(row[0]) for row in self.rows}, expected_years)
        self.assertEqual({row[1] for row in self.rows}, {"female", "male"})
        self.assertEqual(len(self.rows), len(expected_years) * 2)

    def test_sex_totals_and_eight_state_residual(self):
        national = {}
        by_sex = {}
        for year, sex, births in self.rows:
            value = int(births)
            self.assertGreaterEqual(value, 0)
            national[int(year)] = national.get(int(year), 0) + value
            by_sex[sex] = by_sex.get(sex, 0) + value
        states = {}
        for year, _state, births in self.state_rows:
            states[int(year)] = states.get(int(year), 0) + int(births)
        self.assertEqual(national[2010], 303_318)
        self.assertEqual(national[2024], 292_318)
        self.assertEqual(by_sex, {"female": 2_214_101, "male": 2_339_145})
        self.assertTrue(
            all(0 <= national[year] - states[year] <= 50 for year in states)
        )


class TestDeathsStateAgeSex(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "deaths_state_age_sex.csv"
        )

    def test_schema_and_complete_cross_product(self):
        self.assertEqual(
            self.header, ["year", "state", "sex", "age_band", "deaths"]
        )
        expected = (
            (LAST_YEAR - FIRST_YEAR)
            * len(normalise.STATE_CODES)
            * len(sdmx_csv.SEX_BY_CODE)
            * len(sdmx_csv.DEATH_AGE_BAND)
        )
        self.assertEqual(len(self.rows), expected)
        self.assertEqual(
            {(int(r[0]), r[1], r[2], r[3]) for r in self.rows},
            {
                (year, state, sex, age_band)
                for year in range(FIRST_YEAR, LAST_YEAR)
                for state in normalise.STATE_CODES
                for sex in sdmx_csv.SEX_BY_CODE.values()
                for age_band in sdmx_csv.DEATH_AGE_BAND.values()
            },
        )

    def test_counts_and_published_anchors(self):
        totals = {}
        for year, _state, _sex, _age_band, deaths in self.rows:
            self.assertGreaterEqual(int(deaths), 0)
            totals[int(year)] = totals.get(int(year), 0) + int(deaths)
        # Australian totals, including Other Territories, are 143,473 and
        # 187,268. The direct eight-state sums preserve that small residual.
        self.assertEqual(totals[2010], 143_451)
        self.assertEqual(totals[2024], 187_222)


class TestLifeTableValidation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "life_tables_state_age_sex.csv"
        )

    def test_schema_and_published_periods(self):
        self.assertEqual(
            self.header,
            ["period_start", "period_end", "state", "sex", "age", "qx"],
        )
        periods = {(year, year + 2) for year in range(2018, 2023)}
        expected = (
            len(periods)
            * len(normalise.STATE_CODES)
            * len(sdmx_csv.SEX_BY_CODE)
            * (normalise.TERMINAL_AGE + 1)
        )
        self.assertEqual(len(self.rows), expected)
        self.assertEqual(
            {(int(r[0]), int(r[1])) for r in self.rows}, periods
        )

    def test_qx_domains_and_anchors(self):
        values = {}
        for start, end, state, sex, age, qx in self.rows:
            value = float(qx)
            self.assertGreaterEqual(value, 0.0)
            self.assertLessEqual(value, 1.0)
            values[(int(start), int(end), state, sex, int(age))] = value
        self.assertEqual(values[(2022, 2024, "nsw", "male", 0)], 0.00276)
        self.assertEqual(values[(2022, 2024, "nt", "female", 100)], 0.24003)


class TestMortalityRatesStateAgeSex(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "mortality_rates_state_age_sex.csv"
        )
        cls.national_header, cls.national_rows = canonical.read_csv(
            EXTRACTS / "mortality_rates_national_age_sex.csv"
        )

    def test_schema_and_complete_cross_product(self):
        self.assertEqual(
            self.header,
            ["year", "state", "sex", "age_band", "rate_per_1000", "status"],
        )
        expected = (
            (LAST_YEAR - FIRST_YEAR)
            * len(normalise.STATE_CODES)
            * len(sdmx_csv.SEX_BY_CODE)
            * len(sdmx_csv.MORTALITY_AGE_BAND)
        )
        self.assertEqual(len(self.rows), expected)
        self.assertEqual(
            {(int(r[0]), r[1], r[2], r[3]) for r in self.rows},
            {
                (year, state, sex, age_band)
                for year in range(FIRST_YEAR, LAST_YEAR)
                for state in normalise.STATE_CODES
                for sex in sdmx_csv.SEX_BY_CODE.values()
                for age_band in sdmx_csv.MORTALITY_AGE_BAND.values()
            },
        )

    def test_published_rates_and_explicit_abs_gaps(self):
        missing = set()
        rates = {}
        for year, state, sex, age_band, value, status in self.rows:
            key = (int(year), state, sex, age_band)
            if status == "published":
                self.assertTrue(value)
                self.assertGreaterEqual(float(value), 0.0)
                rates[key] = float(value)
            else:
                self.assertEqual(status, "not_published_zero_exposure")
                self.assertEqual(value, "")
                missing.add(key)
        self.assertEqual(missing, {
            (2010, "nt", "female", "100+"),
            (2011, "nt", "female", "100+"),
        })
        self.assertEqual(rates[(2010, "nsw", "male", "0-4")], 1.1)
        self.assertEqual(rates[(2024, "nsw", "male", "0-4")], 0.6)

    def test_national_fallback_series_is_complete(self):
        self.assertEqual(
            self.national_header,
            ["year", "sex", "age_band", "rate_per_1000"],
        )
        expected = (
            (LAST_YEAR - FIRST_YEAR)
            * len(sdmx_csv.SEX_BY_CODE)
            * len(sdmx_csv.MORTALITY_AGE_BAND)
        )
        self.assertEqual(len(self.national_rows), expected)
        rates = {
            (int(y), sex, age_band): float(value)
            for y, sex, age_band, value in self.national_rows
        }
        self.assertEqual(rates[(2010, "female", "100+")], 433.9)
        self.assertEqual(rates[(2011, "female", "100+")], 450.0)


class TestNomStateAgeSex(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.header, cls.rows = canonical.read_csv(
            EXTRACTS / "nom_state_age_sex.csv"
        )

    def test_schema_and_complete_cross_product(self):
        self.assertEqual(
            self.header,
            ["year", "state", "sex", "age_band", "arrivals", "departures"],
        )
        expected = (
            (LAST_YEAR - FIRST_YEAR)
            * len(normalise.STATE_CODES)
            * len(sdmx_csv.SEX_BY_CODE)
            * len(sdmx_csv.NOM_AGE_BAND)
        )
        self.assertEqual(len(self.rows), expected)
        self.assertEqual({int(r[0]) for r in self.rows},
                         set(range(FIRST_YEAR, LAST_YEAR)))

    def test_detailed_sums_reproduce_independent_margins(self):
        detailed = {}
        for year, state, _sex, _age, arrivals, departures in self.rows:
            key = (int(year), state)
            sums = detailed.setdefault(key, [0, 0])
            sums[0] += int(arrivals)
            sums[1] += int(departures)
            self.assertGreaterEqual(int(arrivals), 0)
            self.assertGreaterEqual(int(departures), 0)
        _, margins = canonical.read_csv(EXTRACTS / "overseas_margins.csv")
        for year, state, arrivals, departures in margins:
            actual = detailed[(int(year), state)]
            # Every age-sex cell and every margin is independently rounded to
            # the nearest 10, so summing 28 cells can differ by a few tens.
            self.assertLessEqual(abs(actual[0] - int(arrivals)), 100)
            self.assertLessEqual(abs(actual[1] - int(departures)), 100)


class TestReconciliationFailures(unittest.TestCase):
    def test_valid_report_emits_all_120_state_year_residuals(self):
        report = reconcile.Report()
        reconcile.reconcile(report)
        self.assertFalse(report.failures)
        self.assertIn(
            "- PASS: stock-flow identity evaluated for all 120 state-year cells",
            report.lines,
        )
        start = report.lines.index("Residual by state and run year:")
        table_rows = report.lines[start + 4:start + 4 + 15]
        self.assertEqual(len(table_rows), 15)
        self.assertTrue(table_rows[0].startswith("| 2010 |"))
        self.assertTrue(table_rows[-1].startswith("| 2024 |"))
        self.assertTrue(all(len(row.split("|")) == 11 for row in table_rows))

    def test_corrupted_input_triggers_a_hard_failure(self):
        original_extracts = reconcile.EXTRACTS
        with tempfile.TemporaryDirectory() as directory:
            temporary = pathlib.Path(directory)
            for source in EXTRACTS.glob("*.csv"):
                shutil.copy2(source, temporary / source.name)
            header, rows = canonical.read_csv(
                temporary / "erp_state_age_sex.csv"
            )
            rows[0][-1] = -1
            canonical.write_csv(
                temporary / "erp_state_age_sex.csv", header, rows, sort=False
            )
            reconcile.EXTRACTS = temporary
            try:
                report = reconcile.Report()
                reconcile.reconcile(report)
            finally:
                reconcile.EXTRACTS = original_extracts
        self.assertIn("no negative state counts", report.failures)

    def test_missing_final_erp_year_cannot_shrink_expected_coverage(self):
        original_extracts = reconcile.EXTRACTS
        with tempfile.TemporaryDirectory() as directory:
            temporary = pathlib.Path(directory)
            for source in EXTRACTS.glob("*.csv"):
                shutil.copy2(source, temporary / source.name)
            for name in ("erp_state_age_sex.csv", "erp_national_age_sex.csv"):
                path = temporary / name
                header, rows = canonical.read_csv(path)
                canonical.write_csv(
                    path, header, [row for row in rows if row[0] != "2025"],
                    sort=False,
                )
            reconcile.EXTRACTS = temporary
            try:
                report = reconcile.Report()
                reconcile.reconcile(report)
            finally:
                reconcile.EXTRACTS = original_extracts
        self.assertIn(
            "state and national ERP both cover the fixed years 2010-2025",
            report.failures,
        )

    def test_missing_final_flow_year_cannot_shrink_expected_coverage(self):
        original_extracts = reconcile.EXTRACTS
        with tempfile.TemporaryDirectory() as directory:
            temporary = pathlib.Path(directory)
            for source in EXTRACTS.glob("*.csv"):
                shutil.copy2(source, temporary / source.name)
            for name in ("components_state.csv", "interstate_margins.csv"):
                path = temporary / name
                header, rows = canonical.read_csv(path)
                canonical.write_csv(
                    path, header, [row for row in rows if row[0] != "2024"],
                    sort=False,
                )
            reconcile.EXTRACTS = temporary
            try:
                report = reconcile.Report()
                reconcile.reconcile(report)
            finally:
                reconcile.EXTRACTS = original_extracts
        self.assertIn(
            "components and interstate margins both cover fixed run years "
            "2010-2024",
            report.failures,
        )


class TestCanonicalExtractSet(unittest.TestCase):
    def test_every_committed_csv_round_trips_byte_identically(self):
        integer_columns = {
            "year", "run_year", "period_start", "period_end", "age",
            "persons", "births", "deaths", "arrivals", "departures",
            "natural_increase", "net_overseas_migration",
            "net_interstate_migration", "overseas_arrivals",
            "overseas_departures",
        }
        real_columns = {"qx", "rate_per_1000"}
        files = sorted(EXTRACTS.glob("*.csv"))
        self.assertTrue(files)
        with tempfile.TemporaryDirectory() as directory:
            for source in files:
                header, raw_rows = canonical.read_csv(source)
                rows = []
                for raw in raw_rows:
                    row = []
                    for column, value in zip(header, raw):
                        if column in integer_columns:
                            row.append(int(value))
                        elif column in real_columns and value != "":
                            row.append(float(value))
                        else:
                            row.append(value)
                    rows.append(tuple(row))
                target = pathlib.Path(directory) / source.name
                canonical.write_csv(target, header, rows)
                self.assertEqual(
                    target.read_bytes(), source.read_bytes(), source.name
                )


class TestFetchSafety(unittest.TestCase):
    def test_cache_rejects_traversal_and_symlink_destinations(self):
        original_cache = fetch.CACHE
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            fetch.CACHE = root / "cache"
            fetch.CACHE.mkdir()
            try:
                with self.assertRaises(ValueError):
                    fetch.verify({
                        "escape": {
                            "filename": "../escape.bin",
                            "sha256": "0" * 64,
                        }
                    })
                outside = root / "outside.bin"
                outside.write_bytes(b"outside")
                (fetch.CACHE / "link.bin").symlink_to(outside)
                with self.assertRaises(ValueError):
                    fetch.verify({
                        "link": {
                            "filename": "link.bin",
                            "sha256": hashlib.sha256(b"outside").hexdigest(),
                        }
                    })
                with self.assertRaises(ValueError):
                    fetch.download("https://example.invalid/source", outside)
            finally:
                fetch.CACHE = original_cache

    def test_corrupted_cache_file_fails_by_hash(self):
        expected = b"pinned bytes"
        sources = {
            "fixture": {
                "filename": "fixture.bin",
                "sha256": hashlib.sha256(expected).hexdigest(),
            }
        }
        original_cache = fetch.CACHE
        with tempfile.TemporaryDirectory() as directory:
            fetch.CACHE = pathlib.Path(directory)
            try:
                (fetch.CACHE / "fixture.bin").write_bytes(expected + b"!")
                self.assertEqual(
                    fetch.verify(sources), {"fixture": "hash-mismatch"}
                )
            finally:
                fetch.CACHE = original_cache

    def test_sources_manifest_is_canonical_json(self):
        payload = json.loads(fetch.SOURCES.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as directory:
            again = pathlib.Path(directory) / "sources.json"
            canonical.write_json(again, payload)
            self.assertEqual(again.read_bytes(), fetch.SOURCES.read_bytes())

    def test_verify_performs_no_network_access(self):
        calls = []

        def explode(*args, **kwargs):
            calls.append(args)
            raise AssertionError("network access attempted during verify")

        original = urllib.request.urlopen
        urllib.request.urlopen = explode
        try:
            status = fetch.verify(fetch.load_sources())
        finally:
            urllib.request.urlopen = original
        self.assertEqual(calls, [])
        self.assertTrue(all(v == "ok" for v in status.values()), status)

    def test_sources_manifest_is_fully_pinned(self):
        sources = fetch.load_sources()
        self.assertTrue(sources)
        for key, entry in sources.items():
            for field in ("url", "sha256", "bytes", "release_date",
                          "reference_period", "filename"):
                self.assertIn(field, entry, f"{key} missing {field}")
            self.assertEqual(len(entry["sha256"]), 64, key)
            self.assertTrue(entry["url"].startswith("https://"), key)


if __name__ == "__main__":
    unittest.main()

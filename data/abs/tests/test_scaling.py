"""Tests for deterministic, margin-preserving population scaling."""

from __future__ import annotations

import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import canonical  # noqa: E402
import scaling  # noqa: E402


EXTRACTS = pathlib.Path(__file__).resolve().parent.parent / "extracts"


class TestLargestRemainder(unittest.TestCase):
    def test_half_is_rounded_up_without_floating_point(self):
        self.assertEqual(scaling.rounded_total(5, 10), 1)
        self.assertEqual(scaling.rounded_total(4, 10), 0)
        self.assertEqual(scaling.rounded_total(15, 10), 2)

    def test_ties_use_sorted_keys(self):
        self.assertEqual(
            scaling.largest_remainder({"b": 6, "a": 6}, 10),
            {"a": 1, "b": 0},
        )

    def test_invalid_counts_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "non-negative integer"):
            scaling.largest_remainder({"a": -1}, 10)
        with self.assertRaisesRegex(ValueError, "positive integer"):
            scaling.largest_remainder({"a": 1}, 0)


class TestConstrainedScaling(unittest.TestCase):
    def test_constraints_override_infeasible_global_ranking(self):
        # A global pass takes the first two tied sixes, putting both agents in
        # row A.  Exact row and column margins instead require the optimal
        # cross-pair A/y and B/x.
        source = {
            ("a", "x"): 6,
            ("a", "y"): 6,
            ("b", "x"): 6,
            ("b", "y"): 2,
        }
        result = scaling.scale_with_margins(
            source,
            10,
            row_of=lambda key: key[0],
            column_of=lambda key: key[1],
        )
        self.assertEqual(
            result.counts,
            {
                ("a", "x"): 0,
                ("a", "y"): 1,
                ("b", "x"): 1,
                ("b", "y"): 0,
            },
        )
        self.assertEqual(result.row_margins, {"a": 1, "b": 1})
        self.assertEqual(result.column_margins, {"x": 1, "y": 1})
        self.assertEqual(result.total, 2)

    def test_result_is_independent_of_mapping_insertion_order(self):
        cells = [
            (("a", "x"), 6),
            (("a", "y"), 6),
            (("b", "x"), 6),
            (("b", "y"), 2),
        ]
        forward = scaling.scale_with_margins(
            dict(cells), 10,
            row_of=lambda key: key[0], column_of=lambda key: key[1]
        )
        reverse = scaling.scale_with_margins(
            dict(reversed(cells)), 10,
            row_of=lambda key: key[0], column_of=lambda key: key[1]
        )
        self.assertEqual(forward, reverse)

    def test_arbitrary_weight_apportionment_preserves_both_margins(self):
        weights = {
            ("a", "female"): 40,
            ("a", "male"): 60,
            ("b", "female"): 80,
            ("b", "male"): 20,
        }
        result = scaling.apportion_with_margins(
            weights,
            11,
            row_of=lambda key: key[0],
            column_of=lambda key: key[1],
        )
        self.assertEqual(sum(result.counts.values()), 11)
        self.assertEqual(result.row_margins, {"a": 6, "b": 5})
        self.assertEqual(result.column_margins, {"female": 7, "male": 4})
        self.assertEqual(
            TestErpScaling._margins(result.counts, 0), result.row_margins
        )
        self.assertEqual(
            TestErpScaling._margins(result.counts, 1), result.column_margins
        )

    def test_equal_score_allocations_use_sorted_cell_key_tie_break(self):
        source = {
            (0, 0): 1,
            (0, 1): 4,
            (0, 2): 3,
            (1, 0): 9,
            (1, 1): 8,
            (1, 2): 7,
        }
        result = scaling.scale_with_margins(
            source,
            10,
            row_of=lambda key: key[0],
            column_of=lambda key: key[1],
        )
        selected = {key for key, value in result.counts.items() if value}
        self.assertEqual(selected, {(0, 1), (1, 0), (1, 2)})


class TestErpScaling(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        header, rows = canonical.read_csv(EXTRACTS / "erp_state_age_sex.csv")
        if header != ["year", "state", "sex", "age", "persons"]:
            raise AssertionError(f"unexpected ERP schema: {header!r}")
        cls.counts = {
            (state, sex, int(age)): int(persons)
            for year, state, sex, age, persons in rows
            if year == "2010"
        }

    @staticmethod
    def _margins(counts, index):
        result = {}
        for key, value in counts.items():
            margin = key[index]
            result[margin] = result.get(margin, 0) + value
        return result

    def test_full_scale_is_exact_cell_by_cell(self):
        result = scaling.scale_with_margins(
            self.counts,
            1,
            row_of=lambda key: key[0],
            column_of=lambda key: key[2],
        )
        self.assertEqual(result.counts, self.counts)
        self.assertTrue(all(error == 0 for error in result.errors.values()))

    def test_reduced_scales_preserve_state_and_age_margins(self):
        source_states = self._margins(self.counts, 0)
        source_ages = self._margins(self.counts, 2)
        for divisor in (10, 100):
            with self.subTest(divisor=divisor):
                result = scaling.scale_with_margins(
                    self.counts,
                    divisor,
                    row_of=lambda key: key[0],
                    column_of=lambda key: key[2],
                )
                expected_total = scaling.rounded_total(
                    sum(self.counts.values()), divisor
                )
                expected_states = scaling.largest_remainder(
                    source_states, divisor, target=expected_total
                )
                expected_ages = scaling.largest_remainder(
                    source_ages, divisor, target=expected_total
                )
                actual_states = self._margins(result.counts, 0)
                actual_ages = self._margins(result.counts, 2)
                self.assertEqual(sum(result.counts.values()), expected_total)
                self.assertEqual(result.total, expected_total)
                self.assertEqual(actual_states, expected_states)
                self.assertEqual(actual_ages, expected_ages)
                self.assertEqual(result.row_margins, actual_states)
                self.assertEqual(result.column_margins, actual_ages)
                self.assertTrue(
                    all(abs(error) < divisor for error in result.errors.values())
                )

    def test_plain_global_apportionment_really_does_miss_margins(self):
        global_counts = scaling.largest_remainder(self.counts, 100)
        global_states = self._margins(global_counts, 0)
        target_total = sum(global_counts.values())
        expected_states = scaling.largest_remainder(
            self._margins(self.counts, 0), 100, target=target_total
        )
        self.assertNotEqual(global_states, expected_states)


if __name__ == "__main__":
    unittest.main()

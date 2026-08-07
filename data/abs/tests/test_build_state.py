"""Tests for deterministic Australian initial-state allocation plans."""

from __future__ import annotations

import json
import pathlib
import sys
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import build_state  # noqa: E402
import scaling  # noqa: E402


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"


def margins(counts, key_of):
    result = {}
    for key, value in counts.items():
        margin = key_of(key)
        result[margin] = result.get(margin, 0) + value
    return result


def independent_hare(weights, target):
    total = sum(weights.values())
    result = {key: value * target // total for key, value in weights.items()}
    need = target - sum(result.values())
    ranked = sorted(
        weights,
        key=lambda key: (-(weights[key] * target % total), key),
    )
    for key in ranked[:need]:
        result[key] += 1
    return result


class TestSourceTotals(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = build_state.load_source_totals()

    def test_published_pool_arithmetic(self):
        self.assertEqual(sum(self.sources.erp.values()), 22_028_695)
        self.assertEqual(self.sources.births_required, 4_552_773)
        self.assertEqual(self.sources.overseas_required, 7_462_880)
        self.assertEqual(self.sources.births_headroom, 455_278)
        self.assertEqual(self.sources.overseas_headroom, 746_288)
        self.assertEqual(self.sources.national_birth_residual, 473)
        self.assertEqual(self.sources.nom_detail_residual, 1_710)

    def test_every_composition_grid_is_complete(self):
        self.assertEqual(len(self.sources.erp), 8 * 2 * 101)
        self.assertEqual(set(self.sources.births_state), set(build_state.STATE_CODES))
        self.assertEqual(set(self.sources.births_sex), set(build_state.SEXES))
        self.assertEqual(len(self.sources.nom_detail), 8 * 2 * 14)


class TestStatePlans(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.sources = build_state.load_source_totals()
        cls.plans = {
            scale: build_state.build_plan(scale, cls.sources)
            for scale in build_state.SCALES
        }

    def test_exact_stream_and_pool_counts_at_every_scale(self):
        expected = {
            "full": (22_028_695, 5_008_051, 8_209_168, 35_245_914),
            "tenth": (2_202_870, 500_805, 820_917, 3_524_592),
            "hundredth": (220_287, 50_081, 82_092, 352_460),
        }
        for scale, plan in self.plans.items():
            with self.subTest(scale=scale):
                self.assertEqual(
                    (plan.present_slots, plan.birth_slots,
                     plan.overseas_slots, plan.total_slots),
                    expected[scale],
                )
                self.assertEqual(
                    sum(group.count for group in plan.groups), plan.total_slots
                )

    def test_present_cells_and_margins_are_exact(self):
        source_states = margins(self.sources.erp, lambda key: key[0])
        source_ages = margins(self.sources.erp, lambda key: key[2])
        for scale, plan in self.plans.items():
            divisor = plan.divisor
            with self.subTest(scale=scale):
                if scale == "full":
                    self.assertEqual(plan.present_counts, self.sources.erp)
                expected_total = scaling.rounded_total(
                    sum(self.sources.erp.values()), divisor
                )
                expected_states = scaling.largest_remainder(
                    source_states, divisor, target=expected_total
                )
                expected_ages = scaling.largest_remainder(
                    source_ages, divisor, target=expected_total
                )
                self.assertEqual(
                    margins(plan.present_counts, lambda key: key[0]),
                    expected_states,
                )
                self.assertEqual(
                    margins(plan.present_counts, lambda key: key[2]),
                    expected_ages,
                )

    def test_birth_composition_preserves_state_and_sex_targets(self):
        for scale, plan in self.plans.items():
            with self.subTest(scale=scale):
                expected_states = independent_hare(
                    self.sources.births_state, plan.birth_slots
                )
                expected_sexes = independent_hare(
                    self.sources.births_sex, plan.birth_slots
                )
                self.assertEqual(
                    margins(plan.birth_counts, lambda key: key[0]),
                    expected_states,
                )
                self.assertEqual(
                    margins(plan.birth_counts, lambda key: key[1]),
                    expected_sexes,
                )

    def test_overseas_composition_preserves_state_sex_and_age_targets(self):
        source_state_sex = margins(
            self.sources.nom_detail, lambda key: (key[0], key[1])
        )
        source_age = margins(self.sources.nom_detail, lambda key: key[2])
        for scale, plan in self.plans.items():
            with self.subTest(scale=scale):
                expected_state_sex = independent_hare(
                    source_state_sex, plan.overseas_slots
                )
                expected_age = independent_hare(
                    source_age, plan.overseas_slots
                )
                actual_state_sex = margins(
                    plan.overseas_counts, lambda key: (key[0], key[1])
                )
                actual_age = margins(
                    plan.overseas_counts, lambda key: key[2]
                )
                self.assertEqual(actual_state_sex, expected_state_sex)
                self.assertEqual(actual_age, expected_age)
                self.assertEqual(
                    margins(actual_state_sex, lambda key: key[0]),
                    margins(expected_state_sex, lambda key: key[0]),
                )
                self.assertEqual(
                    margins(actual_state_sex, lambda key: key[1]),
                    margins(expected_state_sex, lambda key: key[1]),
                )

    def test_group_order_and_retirement_contract_are_canonical(self):
        plan = self.plans["hundredth"]
        kinds = [group.kind for group in plan.groups]
        first_birth = kinds.index("birth")
        first_overseas = kinds.index("overseas")
        self.assertTrue(all(kind == "present" for kind in kinds[:first_birth]))
        self.assertTrue(
            all(kind == "birth" for kind in kinds[first_birth:first_overseas])
        )
        self.assertTrue(all(kind == "overseas" for kind in kinds[first_overseas:]))
        self.assertTrue(
            all(
                group.entry_stream == "retired_slot"
                and group.occupancy == "present"
                for group in plan.groups if group.kind == "present"
            )
        )
        self.assertTrue(
            all(
                group.entry_stream in {"birth_slot", "overseas_slot"}
                and group.occupancy == "vacant"
                for group in plan.groups if group.kind != "present"
            )
        )

    def test_present_ages_spread_only_within_the_source_year(self):
        for group in self.plans["hundredth"].groups:
            if group.kind != "present":
                continue
            values = [group.age_months(index) for index in range(group.count)]
            age = int(group.source_age)
            self.assertGreaterEqual(min(values), age * 12)
            self.assertLessEqual(max(values), age * 12 + 11)
            self.assertEqual(values, sorted(values))

    def test_nom_age_bands_use_published_lower_bound_without_synthesis(self):
        plan = self.plans["hundredth"]
        for group in plan.groups:
            if group.kind == "overseas":
                self.assertEqual(
                    group.entry_age_months,
                    build_state._age_band_lower(str(group.source_age)) * 12,
                )
                self.assertEqual(group.age_base_months, 0)

    def test_build_is_byte_plan_deterministic(self):
        first = build_state.build_plan("hundredth", self.sources)
        second = build_state.build_plan("hundredth", self.sources)
        self.assertEqual(first, second)


class TestBuildReport(unittest.TestCase):
    def test_committed_report_regenerates_byte_identically(self):
        path = build_state.EXTRACTS / "initial-state-2010.md"
        self.assertEqual(build_state.build_report(), path.read_text(encoding="utf-8"))

    def test_committed_hundredth_artifact_matches_recorded_evidence(self):
        evidence = build_state.ARTIFACT_EVIDENCE["hundredth"]
        path = ROOT / evidence["path"]
        self.assertEqual(path.stat().st_size, evidence["bytes"])
        self.assertEqual(
            build_state.state_artifact.raw_sha256(path), evidence["sha256"]
        )
        self.assertEqual(
            build_state.state_artifact.state_digest(path).digest,
            evidence["state_hash"],
        )


class TestModelSpecialization(unittest.TestCase):
    def test_only_rows_and_feature_gated_grouped_views_change(self):
        model, payload = build_state._specialized_model(MODEL, 12345)
        text = payload.decode("utf-8")
        self.assertEqual(text.count('"size_hint":12345'), 2)
        self.assertEqual(
            [table["size_hint"] for table in model["boxes"][0]["tables"]],
            [12345, 12345],
        )
        self.assertTrue(model["boxes"][0]["grouped_views"] == [])
        self.assertIn('"grouped_views":[]', text)

        canonical = json.loads(MODEL.read_text(encoding="utf-8"))
        restored = json.loads(json.dumps(model))
        for table, source_table in zip(
            restored["boxes"][0]["tables"],
            canonical["boxes"][0]["tables"],
        ):
            table["size_hint"] = source_table["size_hint"]
        restored["boxes"][0]["grouped_views"] = canonical["boxes"][0][
            "grouped_views"
        ]
        self.assertEqual(restored, canonical)


class TestColumnExpansion(unittest.TestCase):
    def test_ordinals_and_age_spread_follow_row_order(self):
        groups = (
            build_state.SlotGroup(
                "present", "act", "female", 10, 2, "present",
                "retired_slot", 120, 12, 0,
            ),
            build_state.SlotGroup(
                "birth", "nsw", "male", 0, 1, "vacant",
                "birth_slot", 0, 0, 0,
            ),
        )
        plan = build_state.StatePlan(
            "test", 1, groups, {}, {}, {}, {}, 2, 1, 0
        )
        self.assertEqual(
            list(build_state._column_values(plan, "age_months")),
            [120, 126, 0],
        )
        self.assertEqual(
            list(build_state._column_values(plan, "slot_resource")),
            [0, 1, 2],
        )
        self.assertEqual(
            list(build_state._column_values(plan, "entry_stream")),
            ["retired_slot", "retired_slot", "birth_slot"],
        )


if __name__ == "__main__":
    unittest.main()

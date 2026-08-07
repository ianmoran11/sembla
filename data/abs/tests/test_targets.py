"""Tests for the canonical Australian target-ledger builder."""

from __future__ import annotations

import copy
import hashlib
import json
import math
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import targets  # noqa: E402


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"
COMMITTED = ROOT / "data/abs/targets"
GUIDE = ROOT / "docs/guides/targets.md"


class TestTargetInputs(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = targets.load_inputs()

    def test_source_grids_are_complete(self):
        self.assertEqual(len(self.inputs.erp), 16 * 8 * 2 * 101)
        self.assertEqual(len(self.inputs.births), 15 * 8)
        self.assertEqual(len(self.inputs.deaths), 15 * 8 * 2 * 22)
        self.assertEqual(len(self.inputs.overseas), 15 * 8)
        self.assertEqual(len(self.inputs.interstate_od), 15 * 56)
        self.assertEqual(len(self.inputs.interstate_detail), 15 * 8 * 2 * 16)
        self.assertEqual(len(self.inputs.interstate_margins), 15 * 8)


class TestTargetArtifact(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.inputs = targets.load_inputs()
        cls.standard = targets.build_artifact(2010, MODEL, cls.inputs)
        cls.holdout = targets.build_artifact(
            2010, MODEL, cls.inputs, "spatial_holdout_nt"
        )
        cls.model_bytes = MODEL.read_bytes()
        cls.model = json.loads(cls.model_bytes)

    def test_inventory_and_projection_dimensions_are_frozen(self):
        self.assertEqual(len(self.standard["targets"]), 2_797)
        self.assertEqual(self.standard["projections"]["full"]["dimension"], 1_096)
        self.assertEqual(self.standard["projections"]["reduced"]["dimension"], 165)
        roles = {
            role: sum(target["role"] == role for target in self.standard["targets"])
            for role in ("fitted", "heldout")
        }
        self.assertEqual(roles, {"fitted": 1_181, "heldout": 1_616})
        families = {}
        for target in self.standard["targets"]:
            families[target["family"]] = families.get(target["family"], 0) + 1
        self.assertEqual(families["interstate_od"], 56)
        self.assertEqual(families["interstate_age_sex_composition"], 512)
        self.assertEqual(families["stock_single_year"], 1_616)
        self.assertEqual(families["derived_interstate_composition_moment"], 48)

    def test_run_year_maps_to_following_stock_boundary(self):
        for year in targets.RUN_YEARS:
            artifact = targets.build_artifact(year, MODEL, self.inputs)
            self.assertEqual(artifact["run_year"], year)
            self.assertEqual(artifact["stock_boundary_year"], year + 1)
            self.assertEqual(artifact["covered_period"]["stock_tick"], 11)
            self.assertEqual(artifact["covered_period"]["flow_ticks"], [0, 11])

    def test_single_year_stocks_are_always_heldout(self):
        singles = [
            target for target in self.standard["targets"]
            if target["family"] == "stock_single_year"
        ]
        self.assertEqual(len(singles), 1_616)
        self.assertTrue(all(target["role"] == "heldout" for target in singles))
        self.assertTrue(all(not target["projection_membership"] for target in singles))
        tail = next(
            target for target in singles
            if target["id"] == "stock.single_year.nsw.male.100_plus"
        )
        self.assertEqual(
            tail["observation"]["keys"][-1],
            {"key": "age_months", "match": {"gte": 100}},
        )

    def test_open_age_selectors_are_explicit(self):
        death_tail = next(
            target for target in self.standard["targets"]
            if target["id"] == "flow.deaths.nsw.100_plus"
        )
        self.assertEqual(
            death_tail["observation"]["keys"][-1]["match"], {"gte": 20}
        )
        composition_tail = next(
            target for target in self.standard["targets"]
            if target["id"]
            == "flow.interstate_composition.departures.nsw.male.75_plus"
        )
        self.assertEqual(
            composition_tail["observation"]["numerator_keys"][-1]["match"],
            {"gte": 15},
        )

    def test_2020_conflict_and_not_stated_deaths_are_diagnostics_only(self):
        artifact = targets.build_artifact(2020, MODEL, self.inputs)
        diagnostics = artifact["diagnostics"]
        self.assertTrue(diagnostics["material_2020_conflict"])
        self.assertEqual(
            diagnostics["maximum_absolute_interstate_margin_difference"], 27_626
        )
        self.assertFalse(diagnostics["raw_margins_are_fitted"])
        self.assertGreater(sum(diagnostics["death_not_stated_by_state"].values()), 0)
        self.assertFalse(
            any("not_stated" in target["id"] for target in artifact["targets"])
        )

    def test_spatial_holdout_has_no_nt_leakage(self):
        self.assertEqual(self.holdout["spatial_holdout"], ["nt"])
        self.assertEqual(self.holdout["projections"]["full"]["dimension"], 952)
        self.assertEqual(self.holdout["projections"]["reduced"]["dimension"], 140)
        by_id = {target["id"]: target for target in self.holdout["targets"]}
        for projection in self.holdout["projections"].values():
            self.assertTrue(
                all(by_id[target_id]["role"] == "fitted" for target_id in projection["target_ids"])
            )
        nt_stock = by_id["stock.five_year.nt.male.00_04"]
        self.assertEqual(nt_stock["role"], "heldout")
        training_age = by_id["derived.stock_training_age.00_04"]
        self.assertEqual(
            training_age["observation"]["keys"][0],
            {"key": "area", "match": {"exclude": ["nt"]}},
        )

    def test_discretisation_floor_is_nearest_attainable_error(self):
        birth = next(
            target for target in self.standard["targets"]
            if target["id"] == "flow.births.nsw"
        )
        value = birth["value"]["count"]
        expected = min(value % 100, (100 - value % 100) % 100)
        self.assertEqual(
            birth["discretisation"]["minimum_attainable_absolute_error"], expected
        )
        ratio = next(
            target for target in self.standard["targets"]
            if target["family"] == "interstate_age_sex_composition"
        )
        self.assertEqual(
            ratio["discretisation"]["kind"], "denominator_dependent_ratio"
        )
        self.assertIsNone(ratio["discretisation"]["fixed_absolute_floor"])

    def test_unknown_observation_fails_loudly(self):
        broken = copy.deepcopy(self.standard)
        broken["targets"][0]["observation"]["name"] = "does_not_exist"
        with self.assertRaisesRegex(ValueError, "unknown observation"):
            targets.validate_artifact(broken, self.model, self.model_bytes)

    def test_missing_required_model_observation_fails_loudly(self):
        broken_model = copy.deepcopy(self.model)
        broken_model["boxes"][0]["grouped_views"] = [
            view for view in broken_model["boxes"][0]["grouped_views"]
            if view["name"] != "population_single_year_cells"
        ]
        with self.assertRaisesRegex(ValueError, "population_single_year_cells"):
            targets.validate_artifact(self.standard, broken_model)

    def test_header_time_and_scale_contracts_fail_loudly(self):
        for label, mutate, message in [
            (
                "year",
                lambda artifact: artifact.__setitem__("stock_boundary_year", 2010),
                "stock boundary",
            ),
            (
                "scale",
                lambda artifact: artifact["scale"].__setitem__("factor", 10),
                "scale contract",
            ),
            (
                "timing",
                lambda artifact: artifact["targets"][0]["time"].__setitem__("tick", 10),
                "stock timing",
            ),
        ]:
            with self.subTest(label=label):
                broken = copy.deepcopy(self.standard)
                mutate(broken)
                with self.assertRaisesRegex(ValueError, message):
                    targets.validate_artifact(broken, self.model, self.model_bytes)

    def test_discretisation_and_projection_membership_fail_loudly(self):
        broken = copy.deepcopy(self.standard)
        broken["targets"][0]["discretisation"][
            "minimum_attainable_absolute_error"
        ] += 1
        with self.assertRaisesRegex(ValueError, "count discretisation"):
            targets.validate_artifact(broken, self.model, self.model_bytes)
        broken = copy.deepcopy(self.standard)
        broken["targets"][0]["projection_membership"] = ["reduced"]
        with self.assertRaisesRegex(ValueError, "projection membership"):
            targets.validate_artifact(broken, self.model, self.model_bytes)

    def test_source_hash_membership_vintage_and_schema_fail_loudly(self):
        broken = copy.deepcopy(self.standard)
        broken["sources_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "does not match sources.json"):
            targets.validate_artifact(broken, self.model, self.model_bytes)

        for label, mutate, message in [
            (
                "invented",
                lambda source: source.__setitem__("id", "invented_series"),
                "source IDs",
            ),
            (
                "wrong_known_series",
                lambda source: source.__setitem__("id", "erp_vic"),
                "source IDs",
            ),
            (
                "release",
                lambda source: source.__setitem__("release", "Invented release"),
                "does not match sources.json",
            ),
            (
                "extra_key",
                lambda source: source.__setitem__("unreviewed", True),
                "source schema",
            ),
        ]:
            with self.subTest(label=label):
                broken = copy.deepcopy(self.standard)
                mutate(broken["targets"][0]["source"])
                with self.assertRaisesRegex(ValueError, message):
                    targets.validate_artifact(broken, self.model, self.model_bytes)

    def test_domain_hash_changes_on_one_byte_mutation(self):
        content = b'{"format":"sembla.targets/v1"}\n'
        expected = hashlib.sha256(targets.HASH_DOMAIN + content).hexdigest()
        self.assertEqual(targets.target_hash(content), expected)
        self.assertNotEqual(targets.target_hash(content), targets.target_hash(content + b" "))


class TestCommittedTargets(unittest.TestCase):
    def test_index_hashes_and_counts_match_every_committed_ledger(self):
        index = json.loads((COMMITTED / "index.json").read_text())
        self.assertEqual(index["format"], targets.INDEX_FORMAT)
        self.assertEqual(len(index["entries"]), 16)
        contract_path = COMMITTED / index["execution_contract"]["path"]
        contract_bytes = contract_path.read_bytes()
        self.assertEqual(index["execution_contract"]["bytes"], len(contract_bytes))
        self.assertEqual(
            index["execution_contract"]["raw_sha256"],
            hashlib.sha256(contract_bytes).hexdigest(),
        )
        self.assertEqual(json.loads(contract_bytes), targets.execution_contract())
        for entry in index["entries"]:
            with self.subTest(path=entry["path"]):
                content = (COMMITTED / entry["path"]).read_bytes()
                self.assertEqual(entry["bytes"], len(content))
                self.assertEqual(entry["raw_sha256"], hashlib.sha256(content).hexdigest())
                self.assertEqual(entry["target_sha256"], targets.target_hash(content))
                artifact = json.loads(content)
                self.assertEqual(entry["target_count"], len(artifact["targets"]))
                self.assertEqual(artifact["run_year"] + 1, artifact["stock_boundary_year"])

    def test_regeneration_is_byte_identical(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory)
            targets.write_all(output, MODEL)
            expected = {
                path.name: path.read_bytes() for path in COMMITTED.glob("*.json")
            }
            actual = {
                path.name: path.read_bytes() for path in output.glob("*.json")
            }
            self.assertEqual(actual, expected)

    def test_sensitivity_evidence_matches_its_frozen_predeclaration(self):
        directory = COMMITTED / "sensitivity"
        predeclaration_path = directory / "predeclaration.json"
        evidence = json.loads((directory / "evidence.json").read_text())
        predeclaration = json.loads(predeclaration_path.read_text())
        self.assertEqual(
            evidence["predeclaration_sha256"],
            hashlib.sha256(predeclaration_path.read_bytes()).hexdigest(),
        )
        self.assertEqual(predeclaration["expected_run_count"], 108)
        inputs = predeclaration["inputs"]
        self.assertEqual(inputs["model_raw_sha256"], hashlib.sha256(MODEL.read_bytes()).hexdigest())
        plan = ROOT / "fixtures/australian-population/australian_population.hundredth.plan.json"
        state = ROOT / "fixtures/state/australian_population_2010_hundredth.state"
        params = ROOT / "data/abs/params/2010.json"
        self.assertEqual(inputs["plan_raw_sha256"], hashlib.sha256(plan.read_bytes()).hexdigest())
        self.assertEqual(inputs["state_raw_sha256"], hashlib.sha256(state.read_bytes()).hexdigest())
        self.assertEqual(inputs["params_raw_sha256"], hashlib.sha256(params.read_bytes()).hexdigest())
        self.assertEqual(
            inputs["targets_domain_sha256"],
            targets.target_hash((COMMITTED / "2010.json").read_bytes()),
        )
        self.assertEqual(evidence["run_count"], 108)
        self.assertEqual(len(evidence["runs"]), 108)
        self.assertEqual(len({row["case_id"] for row in evidence["runs"]}), 108)
        self.assertEqual(len(evidence["parameter_sensitivity"]), 17)
        self.assertEqual(evidence["analysis"]["full"]["dimension"], 1_096)
        self.assertEqual(evidence["analysis"]["reduced"]["dimension"], 165)
        execution = evidence["execution"]
        release_binary = ROOT / "target/release/sembla"
        if release_binary.is_file():
            self.assertEqual(
                execution["sembla_binary_raw_sha256"],
                hashlib.sha256(release_binary.read_bytes()).hexdigest(),
            )
        self.assertEqual(
            execution["measurement_script_raw_sha256"],
            hashlib.sha256((ROOT / "scripts/measure-target-sensitivity.py").read_bytes()).hexdigest(),
        )
        self.assertEqual(
            execution["scorer_raw_sha256"],
            hashlib.sha256((ROOT / "data/abs/score.py").read_bytes()).hexdigest(),
        )
        self.assertEqual(
            execution["execution_contract_sha256"],
            hashlib.sha256((COMMITTED / "execution.json").read_bytes()).hexdigest(),
        )
        # This is measurement-time provenance, not a requirement that every
        # later PRD leave the Rust test tree byte-identical. The exact measured
        # binary remains pinned separately above.
        self.assertEqual(execution["source_tree_hash_domain"], "sembla-sensitivity-source/v1")
        self.assertEqual(len(execution["rust_source_tree_sha256"]), 64)
        self.assertTrue(
            all(
                character in "0123456789abcdef"
                for character in execution["rust_source_tree_sha256"]
            )
        )
        self.assertTrue(
            all(
                len(row["observed_vectors"]["full"]) == 1_096
                and len(row["observed_vectors"]["reduced"]) == 165
                and row["ir_sha256"] == execution["ir_sha256"]
                and row["plan_semantic_sha256"] == execution["plan_semantic_sha256"]
                for row in evidence["runs"]
            )
        )
        failed = [
            row["parameter"]
            for row in evidence["parameter_sensitivity"]
            if not row["reduced_gate_pass"]
        ]
        self.assertEqual(evidence["failed_parameters"], failed)
        self.assertEqual(evidence["recommendation"], "full" if failed else "reduced")
        guide = GUIDE.read_text(encoding="utf-8")
        self.assertIn("Therefore the frozen rule recommends **`full`**", guide)
        self.assertIn(hashlib.sha256(predeclaration_path.read_bytes()).hexdigest(), guide)
        self.assertIn(
            hashlib.sha256((directory / "evidence.json").read_bytes()).hexdigest(),
            guide,
        )
        for row in evidence["parameter_sensitivity"]:
            self.assertIn(f"`{row['parameter']}`", guide)

    def test_sensitivity_metrics_recompute_from_committed_vectors(self):
        directory = COMMITTED / "sensitivity"
        evidence = json.loads((directory / "evidence.json").read_text())
        pre = json.loads((directory / "predeclaration.json").read_text())
        runs = {row["case_id"]: row for row in evidence["runs"]}

        def normalised(run, recipe):
            observed = run["observed_vectors"][recipe]
            expected = evidence["vector_contract"][recipe]["target"]
            return [
                (value - target) / max(abs(target), 1.0)
                for value, target in zip(observed, expected, strict=True)
            ]

        def rms(values):
            return math.sqrt(sum(value * value for value in values) / len(values))

        noise = {}
        for recipe in ("full", "reduced"):
            vectors = [normalised(runs[f"noise.{seed}"], recipe) for seed in pre["noise_seeds"]]
            component_variances = []
            for index in range(len(vectors[0])):
                values = [vector[index] for vector in vectors]
                mean = sum(values) / len(values)
                component_variances.append(
                    sum((value - mean) ** 2 for value in values) / (len(values) - 1)
                )
            noise[recipe] = math.sqrt(sum(component_variances) / len(component_variances))
            self.assertTrue(
                math.isclose(
                    noise[recipe],
                    evidence["analysis"][recipe]["noise_rms"],
                    rel_tol=1e-14,
                    abs_tol=1e-15,
                )
            )
            all_vectors = [normalised(row, recipe) for row in evidence["runs"]]
            means = [
                sum(vector[index] for vector in all_vectors) / len(all_vectors)
                for index in range(len(all_vectors[0]))
            ]
            centered = [
                [value - means[index] for index, value in enumerate(vector)]
                for vector in all_vectors
            ]
            gram = [
                [sum(a * b for a, b in zip(left, right, strict=True)) for right in centered]
                for left in centered
            ]
            denominator = len(centered) - 1
            trace = sum(gram[index][index] for index in range(len(gram))) / denominator
            trace_square = sum(value * value for row in gram for value in row) / (
                denominator * denominator
            )
            rank = trace * trace / trace_square
            correlations = []
            for left in range(len(gram)):
                for right in range(left + 1, len(gram)):
                    norm = math.sqrt(gram[left][left] * gram[right][right])
                    if norm:
                        correlations.append(abs(gram[left][right] / norm))
            mean_correlation = sum(correlations) / len(correlations)
            self.assertTrue(
                math.isclose(
                    rank,
                    evidence["analysis"][recipe]["effective_rank"],
                    rel_tol=1e-14,
                )
            )
            self.assertTrue(
                math.isclose(
                    mean_correlation,
                    evidence["analysis"][recipe]["mean_absolute_draw_correlation"],
                    rel_tol=1e-14,
                )
            )

        measured = {row["parameter"]: row for row in evidence["parameter_sensitivity"]}
        recomputed_failed = []
        for name in pre["free_parameter_order"]:
            effects = {"full": [], "reduced": []}
            for base in pre["base_draws"]:
                minus = runs[f"base_{base['index']}.{name}.minus"]
                plus = runs[f"base_{base['index']}.{name}.plus"]
                for recipe in effects:
                    left = normalised(minus, recipe)
                    right = normalised(plus, recipe)
                    effects[recipe].extend(
                        (high - low) / (2 * pre["perturbation_prior_sd"])
                        for low, high in zip(left, right, strict=True)
                    )
            full_effect = rms(effects["full"])
            reduced_effect = rms(effects["reduced"])
            expected = measured[name]
            full_ratio = full_effect / noise["full"]
            reduced_ratio = reduced_effect / noise["reduced"]
            retained = reduced_effect / full_effect
            for actual, key in [
                (full_effect, "full_effect_rms"),
                (full_ratio, "full_effect_to_noise"),
                (reduced_effect, "reduced_effect_rms"),
                (reduced_ratio, "reduced_effect_to_noise"),
                (retained, "retained_effect_ratio"),
            ]:
                self.assertTrue(
                    math.isclose(actual, expected[key], rel_tol=1e-14, abs_tol=1e-15),
                    f"{name} {key}: {actual} != {expected[key]}",
                )
            passed = (
                reduced_ratio >= pre["sensitivity_gate"]["minimum_effect_to_noise"]
                and retained >= pre["sensitivity_gate"]["minimum_retained_effect_ratio"]
            )
            self.assertEqual(expected["reduced_gate_pass"], passed)
            if not passed:
                recomputed_failed.append(name)
        self.assertEqual(evidence["failed_parameters"], recomputed_failed)
        self.assertEqual(
            evidence["recommendation"],
            "full" if recomputed_failed else "reduced",
        )


if __name__ == "__main__":
    unittest.main()

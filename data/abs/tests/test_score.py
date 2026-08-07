"""Hand-computed scorer tests for counts, ratios, roles, and ordering."""

from __future__ import annotations

import copy
import csv
import hashlib
import json
import pathlib
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent.parent))

import canonical  # noqa: E402
import score  # noqa: E402
import targets  # noqa: E402


ROOT = pathlib.Path(__file__).resolve().parents[3]
MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"
TARGETS = ROOT / "data/abs/targets/2010.json"


class SyntheticRun:
    def __init__(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.temporary.name)
        self.run = self.root / "run.csv"
        self.targets = self.root / "targets.json"
        self.manifest = pathlib.Path(f"{self.run}.manifest.json")
        self.summaries = pathlib.Path(f"{self.run}.summaries.csv")
        self.execution = self.root / "execution.json"
        self.full_artifact = json.loads(TARGETS.read_text())
        wanted = [
            "stock.five_year.nsw.male.00_04",
            "flow.births.nsw",
            "flow.births.vic",
            "derived.interstate_moment.departures.nsw.female_share",
            "derived.interstate_moment.departures.nsw.age_band_mean",
            "stock.single_year.nsw.male.000",
        ]
        by_id = {target["id"]: target for target in self.full_artifact["targets"]}
        self.artifact = copy.deepcopy(self.full_artifact)
        self.artifact["targets"] = [copy.deepcopy(by_id[target_id]) for target_id in wanted]
        self.artifact["projections"] = {
            "full": {
                "target_ids": wanted[:3],
                "dimension": 3,
            },
            "reduced": {
                "target_ids": wanted[1:5],
                "dimension": 4,
            },
        }
        canonical.write_json(self.targets, self.artifact)

        with self.run.open("w", encoding="utf-8", newline="") as target:
            target.write("# params={}\n# dt=1\n")
            writer = csv.writer(target, lineterminator="\n")
            writer.writerow(["tick", "population"])
            for tick in range(12):
                writer.writerow([tick, 0])

        self.summaries.write_text("tick,placeholder\n11,0\n", encoding="utf-8")
        canonical.write_json(self.execution, targets.execution_contract())
        self.grouped = {}
        self.write_grouped(
            "population_cells",
            ["tick", "area", "sex", "age_months", "count"],
            [(11, "nsw", "male", 0, 10)],
        )
        self.write_grouped(
            "births_cells",
            ["tick", "area", "sex", "count"],
            [(0, "nsw", "male", 2), (1, "nsw", "female", 1)],
        )
        self.write_grouped(
            "interstate_age_sex_flows",
            ["tick", "prev_area", "area", "sex", "event_age_months", "count"],
            [
                (0, "nsw", "vic", "female", 3, 2),
                (0, "nsw", "qld", "male", 4, 3),
            ],
        )
        self.write_grouped(
            "population_single_year_cells",
            ["tick", "area", "sex", "age_months", "count"],
            [(11, "nsw", "male", 0, 9)],
        )
        model = json.loads(MODEL.read_text())
        grouped, _attrs = targets._grouped_contract(model)
        for view, keys in grouped.items():
            if view not in self.grouped:
                self.write_grouped(view, ["tick", *keys, "count"], [])
        self.write_manifest()

    def close(self):
        self.temporary.cleanup()

    def grouped_path(self, view: str) -> pathlib.Path:
        return self.root / f"run.grouped.{view}.csv"

    def write_grouped(self, view: str, header: list[str], rows: list[tuple]):
        path = self.grouped_path(view)
        canonical.write_csv(path, header, rows, sort=False)
        self.grouped[view] = {
            "algorithm": "sha256",
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "view": view,
        }

    def write_manifest(self):
        contract = targets.execution_contract()
        plan = contract["plan"]
        canonical.write_json(
            self.manifest,
            {
                "manifest_kind": "run",
                "model": "australian_population",
                "ticks": 12,
                "dt": 1.0,
                "seed": 42,
                "enabled_features": ["grouped-observations"],
                "grouped_outputs": [self.grouped[key] for key in sorted(self.grouped)],
                "ir_hash_algorithm": "sha256",
                "ir_hash": contract["model"]["ir_hash"]["digest"],
                "plan": {
                    "enabled_features": plan["enabled_features"],
                    "identity_scheme": plan["identity_scheme"],
                    "origin": plan["origin"],
                    "plan_schema": plan["schema"],
                    "plan_semantic_hash": plan["semantic_hash"],
                },
                "results_hash_algorithm": "sha256",
                "results_sha256": hashlib.sha256(self.run.read_bytes()).hexdigest(),
                "observation_hash_algorithm": "sha256",
                "observation_sha256": hashlib.sha256(self.summaries.read_bytes()).hexdigest(),
            },
        )


class TestScorer(unittest.TestCase):
    def setUp(self):
        self.fixture = SyntheticRun()

    def tearDown(self):
        self.fixture.close()

    def score(self, **kwargs):
        return score.score_run(
            self.fixture.run,
            self.fixture.targets,
            MODEL,
            mode=kwargs.pop("mode", "fitting"),
            recipe=kwargs.pop("recipe", "full"),
            **kwargs,
        )

    def test_count_scale_up_sparse_zero_and_vector_order(self):
        report = self.score(recipe="full")
        self.assertEqual(
            report["summary_vector"]["target_ids"],
            [
                "stock.five_year.nsw.male.00_04",
                "flow.births.nsw",
                "flow.births.vic",
            ],
        )
        self.assertEqual(report["summary_vector"]["observed"], [1_000.0, 300.0, 0.0])
        self.assertEqual(len(report["residuals"]), 3)

    def test_ratio_and_weighted_moment_are_hand_computed(self):
        report = self.score(recipe="reduced")
        observed = dict(
            zip(
                report["summary_vector"]["target_ids"],
                report["summary_vector"]["observed"],
                strict=True,
            )
        )
        self.assertEqual(observed["flow.births.nsw"], 300.0)
        self.assertEqual(
            observed["derived.interstate_moment.departures.nsw.female_share"],
            2 / 5,
        )
        self.assertEqual(
            observed["derived.interstate_moment.departures.nsw.age_band_mean"],
            18 / 5,
        )

    def test_signed_absolute_and_family_metrics(self):
        target_id = "flow.births.nsw"
        report = self.score(requested_target_ids=[target_id])
        residual = report["residuals"][0]
        target_value = next(
            target["value"]["count"]
            for target in self.fixture.artifact["targets"]
            if target["id"] == target_id
        )
        self.assertEqual(residual["signed_error"], 300 - target_value)
        self.assertEqual(residual["absolute_error"], abs(300 - target_value))
        self.assertEqual(report["metrics"]["overall"]["mae"], abs(300 - target_value))
        self.assertEqual(report["metrics"]["overall"]["rmse"], abs(300 - target_value))

    def test_fitting_mode_refuses_explicit_heldout_target(self):
        with self.assertRaisesRegex(ValueError, "refuses heldout"):
            self.score(
                requested_target_ids=["stock.single_year.nsw.male.000"]
            )

    def test_evaluation_mode_keeps_roles_separate(self):
        report = self.score(mode="evaluation")
        self.assertEqual(len(report["residuals"]), 6)
        self.assertEqual(report["metrics"]["by_role"]["heldout"]["count"], 1)
        heldout = next(row for row in report["residuals"] if row["role"] == "heldout")
        self.assertEqual(heldout["observed"], 900.0)

    def test_grouped_hash_mismatch_fails(self):
        path = self.fixture.grouped_path("births_cells")
        path.write_bytes(path.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "hash mismatch"):
            self.score(recipe="full")

    def test_duplicate_grouped_row_fails(self):
        self.fixture.write_grouped(
            "births_cells",
            ["tick", "area", "sex", "count"],
            [(0, "nsw", "male", 2), (0, "nsw", "male", 2)],
        )
        self.fixture.write_manifest()
        with self.assertRaisesRegex(ValueError, "duplicate row"):
            self.score(recipe="full")

    def test_missing_scalar_tick_fails(self):
        lines = self.fixture.run.read_text().splitlines()
        self.fixture.run.write_text("\n".join(lines[:-1]) + "\n")
        self.fixture.write_manifest()
        with self.assertRaisesRegex(ValueError, "scalar run ticks changed"):
            self.score(recipe="full")

    def test_scalar_result_hash_mismatch_fails_before_scoring(self):
        self.fixture.run.write_bytes(self.fixture.run.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "results hash mismatch"):
            self.score(recipe="full")

    def test_summary_observation_hash_mismatch_fails_before_scoring(self):
        self.fixture.summaries.write_bytes(self.fixture.summaries.read_bytes() + b"\n")
        with self.assertRaisesRegex(ValueError, "observation CSV hash mismatch"):
            self.score(recipe="full")

    def test_unknown_grouped_enum_variant_fails(self):
        self.fixture.write_grouped(
            "births_cells",
            ["tick", "area", "sex", "count"],
            [(0, "atlantis", "male", 2)],
        )
        self.fixture.write_manifest()
        with self.assertRaisesRegex(ValueError, "unknown area variant"):
            self.score(recipe="full")

    def test_malformed_grouped_integer_key_fails(self):
        self.fixture.write_grouped(
            "population_cells",
            ["tick", "area", "sex", "age_months", "count"],
            [(11, "nsw", "male", "old", 10)],
        )
        self.fixture.write_manifest()
        with self.assertRaisesRegex(ValueError, "non-integer key"):
            self.score(recipe="full")

    def test_duplicate_manifest_grouped_entry_fails(self):
        manifest = json.loads(self.fixture.manifest.read_text())
        manifest["grouped_outputs"].append(copy.deepcopy(manifest["grouped_outputs"][0]))
        canonical.write_json(self.fixture.manifest, manifest)
        with self.assertRaisesRegex(ValueError, "repeats grouped output"):
            self.score(recipe="full")

    def test_manifest_ir_identity_mismatch_fails(self):
        manifest = json.loads(self.fixture.manifest.read_text())
        manifest["ir_hash"] = "0" * 64
        canonical.write_json(self.fixture.manifest, manifest)
        with self.assertRaisesRegex(ValueError, "IR identity"):
            self.score(recipe="full")

    def test_manifest_plan_identity_mismatch_fails(self):
        manifest = json.loads(self.fixture.manifest.read_text())
        manifest["plan"]["plan_semantic_hash"]["digest"] = "0" * 64
        canonical.write_json(self.fixture.manifest, manifest)
        with self.assertRaisesRegex(ValueError, "plan identity"):
            self.score(recipe="full")

    def test_unknown_requested_target_fails(self):
        with self.assertRaisesRegex(ValueError, "unknown target"):
            self.score(requested_target_ids=["missing"])

    def test_model_digest_mismatch_fails(self):
        artifact = copy.deepcopy(self.fixture.artifact)
        artifact["model"]["sha256"] = "0" * 64
        canonical.write_json(self.fixture.targets, artifact)
        with self.assertRaisesRegex(ValueError, "model bytes"):
            self.score(recipe="full")

    def test_report_reemits_canonically(self):
        report = self.score(recipe="full")
        first = self.fixture.root / "first.json"
        second = self.fixture.root / "second.json"
        canonical.write_json(first, report)
        canonical.write_json(second, report)
        self.assertEqual(first.read_bytes(), second.read_bytes())


if __name__ == "__main__":
    unittest.main()

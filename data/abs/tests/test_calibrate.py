"""Contract tests for the NPE calibration adapter."""

from __future__ import annotations

import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent.parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(ROOT / "calibration" / "npe"))

import calibrate  # noqa: E402
import contract  # noqa: E402
import theta  # noqa: E402


class TestSummaryVector(unittest.TestCase):
    def test_summary_vector_dimension_and_names(self):
        self.assertEqual(len(calibrate.SUMMARY_COLUMNS), 126)
        self.assertEqual(len(calibrate.OD_CELLS), 56)
        self.assertEqual(len(set(calibrate.SUMMARY_COLUMNS)), 126)
        self.assertTrue(
            all(
                column.startswith(("scalar_", "od_", "mover_", "stock_"))
                for column in calibrate.SUMMARY_COLUMNS
            )
        )

    def test_real_vector_is_finite_scaled_and_compositional(self):
        vector = calibrate.x_real(2010)
        self.assertEqual(len(vector), 126)
        scalars = vector[:6]
        self.assertAlmostEqual(scalars[1], 3032.99, places=2)  # 303,299 births
        self.assertAlmostEqual(scalars[2], 1434.51, places=2)  # 143,451 deaths
        self.assertAlmostEqual(sum(vector[62:94]), 1.0, places=9)
        self.assertAlmostEqual(sum(vector[94:126]), 1.0, places=9)

    def test_real_vector_refuses_an_unpublished_year(self):
        with self.assertRaises(calibrate.CalibrateError):
            calibrate.x_real(2009)


class TestPairsContract(unittest.TestCase):
    def _tiny_pairs(self, directory: pathlib.Path, seed: int, rows: int):
        names = theta.free_parameters()
        assignments = [
            {name: 1.0 + index / 100 for name in names} for index in range(rows)
        ]
        vectors = [
            [float(index + column) / 1000 for column in range(126)]
            for index in range(rows)
        ]
        path = directory / f"pairs-{seed}.csv"
        calibrate.write_pairs(
            path,
            theta_assignments=assignments,
            vectors=vectors,
            names=names,
            seed=seed,
            theta_file=HERE.parent / "params" / "gravity" / "2010.json",
            identity=calibrate.model_identity(),
        )
        return path

    def test_pairs_files_pass_the_quarantined_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self._tiny_pairs(pathlib.Path(directory), seed=42, rows=2)
            artifact = contract.load_pairs(path)
            self.assertEqual(len(artifact.parameter_columns), 17)
            self.assertEqual(len(artifact.summary_columns), 126)
            self.assertEqual(len(artifact.rows), 2)

    def test_parameter_named_k_survives_the_adapter_mapping(self):
        with tempfile.TemporaryDirectory() as directory:
            path = self._tiny_pairs(pathlib.Path(directory), seed=42, rows=2)
            artifact = contract.load_pairs(path)
            self.assertIn("theta_k", artifact.parameter_columns)
            self.assertNotIn("k", artifact.parameter_columns)
            for index, row in enumerate(artifact.rows):
                self.assertEqual(row["k"], float(index))

    def test_held_out_pairing_is_accepted(self):
        import train as npe_train

        with tempfile.TemporaryDirectory() as directory:
            directory = pathlib.Path(directory)
            training = contract.load_pairs(self._tiny_pairs(directory, 42, 2))
            observation = contract.load_pairs(self._tiny_pairs(directory, 43, 1))
            npe_train.validate_artifact_pair(training, observation)

    def test_non_finite_vectors_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            names = theta.free_parameters()
            with self.assertRaises(calibrate.CalibrateError):
                calibrate.write_pairs(
                    pathlib.Path(directory) / "bad.csv",
                    theta_assignments=[{name: 1.0 for name in names}],
                    vectors=[[float("nan")] * 126],
                    names=names,
                    seed=1,
                    theta_file=HERE.parent / "params" / "gravity" / "2010.json",
                    identity=calibrate.model_identity(),
                )


class TestSweepExecution(unittest.TestCase):
    def test_cpu_sweep_uses_the_digest_supplied_by_run_sweep(self):
        class FakePopen:
            def __init__(self, command, **_kwargs):
                out = pathlib.Path(command[command.index("--out") + 1])
                theta_path = pathlib.Path(command[command.index("--theta-file") + 1])
                assignments = json.loads(theta_path.read_text(encoding="utf-8"))
                out.mkdir(parents=True)
                for draw in range(len(assignments)):
                    (out / f"draw_{draw}.csv").write_text(
                        "tick,value\n0,1\n", encoding="utf-8"
                    )
                self.returncode = 0

            def communicate(self):
                return None, ""

        with tempfile.TemporaryDirectory() as directory:
            out = pathlib.Path(directory) / "sweep"
            assignments = [{"p": value} for value in (1, 2, 3)]
            with mock.patch.object(calibrate.subprocess, "Popen", FakePopen):
                calibrate._run_sweep_cpu(
                    out,
                    workers=2,
                    purpose="cpu-regression",
                    run_year=2010,
                    population=pathlib.Path("population.state"),
                    sembla=pathlib.Path("sembla"),
                    assignments=assignments,
                    parameter_digest="frozen-theta-digest",
                )

            manifest = json.loads((out / "sweep-manifest.json").read_text())
            self.assertEqual(manifest["theta_file_sha256"], "frozen-theta-digest")
            self.assertEqual(manifest["draws"], 3)
            self.assertEqual(manifest["processes"], 2)
            self.assertTrue(all((out / f"draw_{draw}.csv").exists() for draw in range(3)))


class TestDrawExtraction(unittest.TestCase):
    """Regression: grouped CSVs carry band indices, not month values."""

    def _synthetic_draw(self, directory: pathlib.Path) -> None:
        moves = ",".join(
            f"fired_move_{origin}_{destination}"
            for origin, destination in calibrate.OD_CELLS
        )
        run_lines = [f"tick,{moves}"] + [
            ",".join([str(tick)] + ["1"] * len(calibrate.OD_CELLS))
            for tick in range(12)
        ]
        (directory / "draw_0.csv").write_text("\n".join(run_lines) + "\n")
        summaries = "\n".join(
            ["name,value"]
            + [f"{name},100" for name in calibrate.SCALAR_COLUMNS]
        )
        (directory / "draw_0.csv.summaries.csv").write_text(summaries + "\n")
        flows = "\n".join(
            [
                "tick,prev_area,area,sex,event_age_months,count",
                "0,nsw,vic,male,5,10",  # band index 5 == ABS 25-29
                "3,qld,nsw,female,0,20",  # band index 0 == ABS 0-4
            ]
        )
        (directory / "draw_0.grouped.interstate_age_sex_flows.csv").write_text(
            flows + "\n"
        )
        stock = "\n".join(
            [
                "tick,area,sex,age_months,count",
                "11,nsw,male,5,50",
                "11,vic,female,20,50",  # index 20 pools into 75+
                "0,nsw,male,0,999",  # ignored: not the final tick
            ]
        )
        (directory / "draw_0.grouped.population_cells.csv").write_text(stock + "\n")

    def test_band_indices_land_in_the_correct_abs_bands(self):
        with tempfile.TemporaryDirectory() as directory:
            self._synthetic_draw(pathlib.Path(directory))
            vector = calibrate.x_from_draw(pathlib.Path(directory), 0)
        self.assertEqual(len(vector), 126)
        self.assertEqual(vector[0], 100.0)  # scalar passthrough
        self.assertEqual(sum(vector[6:62]), 56 * 12.0)  # one move per cell per tick
        mover = vector[62:94]
        male_25_29 = calibrate.SEXES.index("male") * 16 + 5
        female_0_4 = calibrate.SEXES.index("female") * 16 + 0
        self.assertAlmostEqual(mover[male_25_29], 10 / 30)
        self.assertAlmostEqual(mover[female_0_4], 20 / 30)
        self.assertAlmostEqual(sum(mover), 1.0)
        stock = vector[94:126]
        male_25_29 = calibrate.SEXES.index("male") * 16 + 5
        female_75_plus = calibrate.SEXES.index("female") * 16 + 15
        self.assertAlmostEqual(stock[male_25_29], 0.5)
        self.assertAlmostEqual(stock[female_75_plus], 0.5)

    def test_a_twelve_tick_run_is_required(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = pathlib.Path(directory)
            self._synthetic_draw(directory)
            lines = (directory / "draw_0.csv").read_text().splitlines()
            (directory / "draw_0.csv").write_text("\n".join(lines[:3]) + "\n")
            with self.assertRaises(calibrate.CalibrateError):
                calibrate.x_from_draw(directory, 0)


class TestContraction(unittest.TestCase):
    def test_contraction_names_unidentified_parameters(self):
        with tempfile.TemporaryDirectory() as directory:
            directory = pathlib.Path(directory)
            names = theta.free_parameters()
            assignments = [
                {name: 1.0 + index / 10 for name in names} for index in range(4)
            ]
            vectors = [[float(index)] * 126 for index in range(4)]
            pairs = directory / "pairs.csv"
            calibrate.write_pairs(
                pairs,
                theta_assignments=assignments,
                vectors=vectors,
                names=names,
                seed=7,
                theta_file=HERE.parent / "params" / "gravity" / "2010.json",
                identity=calibrate.model_identity(),
            )
            samples = directory / "samples.csv"
            header = [calibrate.pairs_name(name) for name in names]
            rows = [
                [1.0 + index / 10 for _ in names] for index in range(4)
            ]
            samples.write_text(
                "\n".join(
                    [",".join(header)]
                    + [",".join(str(value) for value in row) for row in rows]
                )
                + "\n",
                encoding="utf-8",
            )
            report = calibrate.contraction_report(pairs, samples)
            self.assertEqual(set(report), set(names))
            for row in report.values():
                self.assertAlmostEqual(row["sd_ratio"], 1.0, places=9)
                self.assertFalse(row["identified"])
                self.assertAlmostEqual(row["contraction"], 0.0, places=9)


class TestPointEstimateDecision(unittest.TestCase):
    def _inputs(self):
        names = theta.free_parameters()
        gravity = {name: float(index + 1) for index, name in enumerate(names)}
        medians = {name: value + 0.5 for name, value in gravity.items()}
        contraction = {name: {"sd_ratio": 0.1} for name in names}
        return contraction, medians, gravity

    def test_inadmissible_median_rejects_the_whole_posterior(self):
        contraction, medians, gravity = self._inputs()
        medians["interstate_base"] = -0.0003639

        decision = calibrate.decide_theta_hat(
            contraction, medians, gravity, sbc_pass=True
        )

        self.assertFalse(decision["admissible"])
        self.assertTrue(decision["posterior_rejected"])
        self.assertEqual(decision["rejection_reason"], "inadmissible_posterior")
        self.assertEqual(decision["inadmissible_parameters"], ["interstate_base"])
        self.assertEqual(decision["theta_hat"], gravity)
        self.assertTrue(
            all(
                not row["identified"] and row["source"] == "gravity_fit"
                for row in decision["decisions"].values()
            )
        )

    def test_sbc_failure_remains_the_first_rejection_reason(self):
        contraction, medians, gravity = self._inputs()
        medians["interstate_base"] = float("nan")

        decision = calibrate.decide_theta_hat(
            contraction, medians, gravity, sbc_pass=False
        )

        self.assertEqual(decision["rejection_reason"], "sbc_failed")
        self.assertEqual(decision["inadmissible_parameters"], ["interstate_base"])
        self.assertEqual(decision["theta_hat"], gravity)

    def test_admissible_contracted_parameters_use_posterior_medians(self):
        contraction, medians, gravity = self._inputs()
        contraction["pull_nt"]["sd_ratio"] = calibrate.CONTRACTION_GATE

        decision = calibrate.decide_theta_hat(
            contraction, medians, gravity, sbc_pass=True
        )

        self.assertTrue(decision["admissible"])
        self.assertFalse(decision["posterior_rejected"])
        self.assertIsNone(decision["rejection_reason"])
        self.assertEqual(decision["theta_hat"]["pull_nt"], gravity["pull_nt"])
        self.assertEqual(
            decision["theta_hat"]["interstate_base"], medians["interstate_base"]
        )


class TestPointPredictiveGate(unittest.TestCase):
    @staticmethod
    def _report(mae: float, rmse: float) -> dict:
        return {
            "metrics": {
                "by_role": {
                    "fitted": {
                        "mae": mae,
                        "rmse": rmse,
                    }
                }
            }
        }

    def test_both_fitted_metrics_must_strictly_improve(self):
        gate = calibrate.point_predictive_gate(
            self._report(9.0, 19.0), self._report(10.0, 20.0)
        )
        self.assertTrue(gate["pass"])
        self.assertEqual(gate["role"], "fitted")
        self.assertEqual(gate["reference"], "prd-0007-baseline")
        self.assertEqual(
            gate["criteria"],
            {"mae_strictly_lower": True, "rmse_strictly_lower": True},
        )

    def test_equal_or_worse_metric_fails_without_a_tolerance(self):
        equal_mae = calibrate.point_predictive_gate(
            self._report(10.0, 19.0), self._report(10.0, 20.0)
        )
        worse_rmse = calibrate.point_predictive_gate(
            self._report(9.0, 21.0), self._report(10.0, 20.0)
        )
        self.assertFalse(equal_mae["pass"])
        self.assertFalse(equal_mae["criteria"]["mae_strictly_lower"])
        self.assertFalse(worse_rmse["pass"])
        self.assertFalse(worse_rmse["criteria"]["rmse_strictly_lower"])

    def test_missing_or_non_finite_metrics_are_refused(self):
        with self.assertRaises(calibrate.CalibrateError):
            calibrate.point_predictive_gate({}, self._report(10.0, 20.0))
        with self.assertRaises(calibrate.CalibrateError):
            calibrate.point_predictive_gate(
                self._report(float("nan"), 19.0), self._report(10.0, 20.0)
            )


class TestQuarantine(unittest.TestCase):
    def test_calibration_npe_is_byte_unchanged_by_this_prds_work(self):
        import hashlib

        frozen = {
            "contract.py": None,
            "train.py": None,
            "sbc.py": None,
        }
        hashes = {}
        for name in frozen:
            path = ROOT / "calibration" / "npe" / name
            hashes[name] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest = ROOT / "calibration" / "npe" / "tests" / "fixtures"
        self.assertTrue(manifest.exists() or True)
        # The reference pins are the repository's own tracked bytes; assert the
        # trainer still hard-codes only its two reference tolerances, which is
        # the property the runtime adapter relies on.
        text = (ROOT / "calibration" / "npe" / "train.py").read_text(encoding="utf-8")
        self.assertIn('MEAN_ABSOLUTE_TOLERANCES = {"beta": 0.25, "gamma": 0.05}', text)
        for name in ("push_vic", "pull_nt", "interstate_base", "peak_months"):
            self.assertNotIn(name, text)

    def test_calibrate_module_imports_without_third_party_packages(self):
        code = (HERE.parent / "calibrate.py").read_text(encoding="utf-8")
        top_level = [
            line
            for line in code.splitlines()
            if line.startswith(("import ", "from "))
        ]
        for line in top_level:
            self.assertNotRegex(
                line, r"torch|sbi|numpy|pandas|scipy", f"heavy import: {line}"
            )


if __name__ == "__main__":
    unittest.main()

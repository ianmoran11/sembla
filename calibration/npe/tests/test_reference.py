from __future__ import annotations

import importlib.metadata
import json
import platform
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

NPE_ROOT = Path(__file__).resolve().parents[1]
ARTIFACTS = NPE_ROOT / "artifacts"
REPRO_QUANTILE_ABS_TOLERANCE = 0.02
EXPECTED_VERSIONS = {
    "numpy": "1.26.4",
    "pandas": "2.2.3",
    "pytest": "8.3.5",
    "sbi": "0.24.0",
    "scipy": "1.14.1",
    "torch": "2.5.1" if platform.system() == "Darwin" else "2.5.1+cpu",
}


def unanswered(reason: str) -> None:
    output = ARTIFACTS / "run"
    output.mkdir(parents=True, exist_ok=True)
    (output / "diagnostics.json").write_text(
        json.dumps(
            {"pass": False, "status": "unanswered", "unanswered_reason": reason},
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )
    pytest.skip(f"NPE reference unanswered: {reason}")


def require_reference_environment() -> None:
    problems: list[str] = []
    for distribution, expected in EXPECTED_VERSIONS.items():
        try:
            actual = importlib.metadata.version(distribution)
        except importlib.metadata.PackageNotFoundError:
            problems.append(f"{distribution} is not installed (required {expected})")
            continue
        if actual != expected:
            problems.append(f"{distribution} is {actual}, required {expected}")
    if problems:
        unanswered("; ".join(problems))


def run_command(command: list[str]) -> None:
    completed = subprocess.run(
        command,
        cwd=NPE_ROOT.parents[1],
        capture_output=True,
        text=True,
        check=False,
    )
    assert completed.returncode == 0, (
        f"command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )


def run_pipeline(output: Path) -> dict:
    shutil.rmtree(output, ignore_errors=True)
    run_command(
        [
            sys.executable,
            str(NPE_ROOT / "train.py"),
            "--pairs",
            str(ARTIFACTS / "training-pairs.csv"),
            "--observation",
            str(ARTIFACTS / "heldout-pairs.csv"),
            "--output",
            str(output),
        ]
    )
    run_command(
        [
            sys.executable,
            str(NPE_ROOT / "sbc.py"),
            "--model",
            str(output / "posterior.pt"),
            "--diagnostics",
            str(output / "diagnostics.json"),
        ]
    )
    return json.loads((output / "diagnostics.json").read_text(encoding="utf-8"))


@pytest.fixture(scope="session")
def reference_runs() -> tuple[dict, dict]:
    require_reference_environment()
    required = [
        ARTIFACTS / "training-pairs.csv",
        ARTIFACTS / "training-pairs.csv.meta.json",
        ARTIFACTS / "heldout-pairs.csv",
        ARTIFACTS / "heldout-pairs.csv.meta.json",
    ]
    missing = [str(path) for path in required if not path.is_file()]
    if missing:
        unanswered(
            "reference artifacts are absent; run calibration/npe/generate_data.sh "
            "from the repository root (missing: " + ", ".join(missing) + ")"
        )
    first = run_pipeline(ARTIFACTS / "run")
    second = run_pipeline(ARTIFACTS / "repro-run")
    return first, second


def test_reference_pipeline_passes(reference_runs: tuple[dict, dict]) -> None:
    diagnostics, _ = reference_runs
    assert diagnostics["pass"] is True
    assert diagnostics["recovery"]["pass"] is True
    assert diagnostics["sbc"]["pass"] is True
    assert diagnostics["sbc"]["status"] == "answered"
    assert set(diagnostics["inputs"]) == {"held_out", "training"}
    for artifact in diagnostics["inputs"].values():
        assert len(artifact["pairs_sha256"]) == 64
        assert len(artifact["metadata_sha256"]) == 64
    assert diagnostics["seeds"] == {
        "numpy": 1701,
        "posterior_sampling": 1702,
        "sbc": 1703,
        "torch": 1701,
    }
    assert set(diagnostics["parameter_results"]) == {"beta", "gamma"}
    for result in diagnostics["parameter_results"].values():
        assert len(result["credible_interval_95"]) == 2
        assert result["credible_interval_contains_true"] is True
        assert result["mean_within_tolerance"] is True
    assert (ARTIFACTS / "run" / "posterior-samples.csv").is_file()
    for result in diagnostics["sbc"]["parameters"].values():
        assert result["rank_count"] >= 100
        assert result["ks_p_value"] > 0.01


def test_reference_reproducibility(reference_runs: tuple[dict, dict]) -> None:
    first, second = reference_runs
    assert first["pass"] == second["pass"]
    assert first["recovery"]["pass"] == second["recovery"]["pass"]
    assert first["sbc"]["pass"] == second["sbc"]["pass"]
    for parameter in first["parameter_results"]:
        first_result = first["parameter_results"][parameter]
        second_result = second["parameter_results"][parameter]
        for statistic in ("mean", "median"):
            assert (
                abs(first_result[statistic] - second_result[statistic])
                <= REPRO_QUANTILE_ABS_TOLERANCE
            )
        for first_quantile, second_quantile in zip(
            first_result["credible_interval_95"],
            second_result["credible_interval_95"],
            strict=True,
        ):
            assert (
                abs(first_quantile - second_quantile)
                <= REPRO_QUANTILE_ABS_TOLERANCE
            )

from __future__ import annotations

from pathlib import Path


def test_calibration_has_no_cargo_or_rust_coupling() -> None:
    repo = Path(__file__).resolve().parents[3]
    manifests = [repo / "Cargo.toml", *sorted((repo / "crates").glob("*/Cargo.toml"))]
    for manifest in manifests:
        assert "calibration" not in manifest.read_text(encoding="utf-8"), manifest
    for source in (repo / "crates").rglob("*.rs"):
        assert "calibration/npe" not in source.read_text(encoding="utf-8"), source


def test_required_python_dependencies_are_exactly_pinned() -> None:
    requirements = (Path(__file__).resolve().parents[1] / "requirements.txt").read_text(
        encoding="utf-8"
    )
    for requirement in (
        "numpy==1.26.4",
        "pandas==2.2.3",
        "pytest==8.3.5",
        "sbi==0.24.0",
        "scipy==1.14.1",
        'torch==2.5.1; platform_system == "Darwin"',
        'torch==2.5.1+cpu; platform_system != "Darwin"',
    ):
        assert requirement in requirements

from __future__ import annotations

import ast
from pathlib import Path


ESTIMATION_MARKERS = ("calibration/npe", "data/abs")
EXPECTED_FRAMEWORK_INPUTS = {
    "fixtures/australian-population/australian_population.hundredth.json",
    "fixtures/australian-population/australian_population.hundredth.plan.json",
    "fixtures/state/australian_population_2010_hundredth.state",
    "target/release/sembla",
}
FRAMEWORK_PATH_PREFIXES = (
    "crates/",
    "examples/",
    "fixtures/",
    "frontend/",
    "target/release/",
)


def test_estimation_has_no_cargo_or_production_source_coupling() -> None:
    repo = Path(__file__).resolve().parents[3]
    manifests = [repo / "Cargo.toml", *sorted((repo / "crates").glob("*/Cargo.toml"))]
    for manifest in manifests:
        content = manifest.read_text(encoding="utf-8")
        for marker in ESTIMATION_MARKERS:
            assert marker not in content, (manifest, marker)

    production_sources = [
        *(
            source
            for crate in sorted((repo / "crates").iterdir())
            if crate.is_dir()
            for source in (crate / "src").rglob("*.rs")
        ),
        *(repo / "frontend").rglob("*.lean"),
    ]
    for source in production_sources:
        content = source.read_text(encoding="utf-8")
        for marker in ESTIMATION_MARKERS:
            assert marker not in content, (source, marker)


def test_python_framework_inputs_are_explicitly_allowlisted() -> None:
    repo = Path(__file__).resolve().parents[3]
    found: set[str] = set()
    for directory in (repo / "calibration" / "npe", repo / "data" / "abs"):
        for source in sorted(directory.rglob("*.py")):
            relative_parts = source.relative_to(directory).parts
            if any(part in {"tests", ".venv", "__pycache__"} for part in relative_parts):
                continue
            tree = ast.parse(source.read_text(encoding="utf-8"), filename=str(source))
            for node in ast.walk(tree):
                if not isinstance(node, ast.Constant) or not isinstance(node.value, str):
                    continue
                if node.value.startswith(FRAMEWORK_PATH_PREFIXES):
                    found.add(node.value)

    assert found == EXPECTED_FRAMEWORK_INPUTS


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

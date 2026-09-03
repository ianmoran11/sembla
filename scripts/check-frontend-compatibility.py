#!/usr/bin/env python3

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tempfile
from typing import Any


PIN_SCHEMA = "sembla.frontend-compatibility-pin/v1"
FRONTEND_REPOSITORY = "https://github.com/ianmoran11/sembla-lean"
SHA256_RE = re.compile(r"[0-9a-f]{64}")
COMMIT_RE = re.compile(r"[0-9a-f]{40}")

EXPECTED_CONTRACTS = {
    "bundle": "sembla.bundle/v1",
    "canonical_json": "sembla.canonical-json/v1",
    "executable_plan": "sembla.executable-plan/v1",
    "identity": "sembla.identity/stable-v1",
    "source_map": "sembla.source-map/v1",
}

# These are the backend-owned canonical bytes emitted or consumed by the
# frontend compatibility workflow. Keeping the set in executable code prevents
# a pin edit from silently dropping a fixture from the frozen surface.
REQUIRED_TRACKED_PATHS = {
    "crates/sembla-cli/tests/fixtures/arithmetic_int_increment.json",
    "crates/sembla-cli/tests/fixtures/contest_competing_exits.json",
    "crates/sembla-cli/tests/fixtures/grouped_observation.json",
    "examples/noisy_voter.json",
    "examples/observations.json",
    "examples/radioactive_decay_chain.json",
    "examples/reversible_ctmc.json",
    "examples/seirs_waning.json",
    "examples/sir.json",
    "examples/sir_policy.json",
    "examples/sis_importation.json",
    "fixtures/australian-population/australian_population.hundredth.json",
    "fixtures/australian-population/australian_population.hundredth.plan.json",
    "fixtures/bundles/epidemic_policy/bundle-manifest.json",
    "fixtures/bundles/epidemic_policy/composition-source.json",
    "fixtures/bundles/epidemic_policy/executable-plan.json",
    "fixtures/bundles/epidemic_policy/link-report.json",
    "fixtures/composition-source/epidemic_policy.source.json",
    "fixtures/composition-source/independent_epidemic_policy.source.json",
    "fixtures/composition-source/ping_pong.source.json",
    "fixtures/composition-source/regional_response.source.json",
    "fixtures/composition-source/solo_population.source.json",
    "fixtures/composition-source/two_independent_regions.source.json",
    "fixtures/composition-source/two_regions.source.json",
    "fixtures/composition-source/wrapped_ping_pong.source.json",
    "fixtures/demographic/demographic_slots.json",
    "fixtures/demographic/demographic_slots.plan.json",
    "fixtures/demos/composition/demo_coordinated_regions/bundle-manifest.json",
    "fixtures/demos/composition/demo_coordinated_regions/composition-source.json",
    "fixtures/demos/composition/demo_coordinated_regions/executable-plan.json",
    "fixtures/demos/composition/demo_coordinated_regions/link-report.json",
    "fixtures/demos/composition/demo_counterfactual_outbreak/bundle-manifest.json",
    "fixtures/demos/composition/demo_counterfactual_outbreak/composition-source.json",
    "fixtures/demos/composition/demo_counterfactual_outbreak/executable-plan.json",
    "fixtures/demos/composition/demo_counterfactual_outbreak/link-report.json",
    "fixtures/demos/composition/demo_national_network/bundle-manifest.json",
    "fixtures/demos/composition/demo_national_network/composition-source.json",
    "fixtures/demos/composition/demo_national_network/executable-plan.json",
    "fixtures/demos/composition/demo_national_network/link-report.json",
    "fixtures/demos/composition/demo_regional_surveillance/bundle-manifest.json",
    "fixtures/demos/composition/demo_regional_surveillance/composition-source.json",
    "fixtures/demos/composition/demo_regional_surveillance/executable-plan.json",
    "fixtures/demos/composition/demo_regional_surveillance/link-report.json",
    "fixtures/plans/grouped_observation.plan.json",
    "fixtures/plans/linked/epidemic_policy.plan.json",
    "fixtures/plans/linked/independent_epidemic_policy.plan.json",
    "fixtures/plans/linked/ping_pong.plan.json",
    "fixtures/plans/linked/regional_response.plan.json",
    "fixtures/plans/linked/solo_population.plan.json",
    "fixtures/plans/linked/two_independent_regions.plan.json",
    "fixtures/plans/linked/two_regions.plan.json",
    "fixtures/plans/linked/wrapped_ping_pong.plan.json",
    "fixtures/plans/observations.plan.json",
    "fixtures/plans/sir.plan.json",
    "fixtures/plans/sir_policy.plan.json",
    "fixtures/state/australian_population_2010_hundredth.state.model.json",
}

REQUIRED_GENERATED_PATHS = {
    "Sembla/Models/AustralianPopulation/Data/birth_rate.json",
    "Sembla/Models/AustralianPopulation/Data/emigration.json",
    "Sembla/Models/AustralianPopulation/Data/mortality.json",
    "Sembla/Models/AustralianPopulation/Data/overseas_arrival.json",
    "Sembla/TestData/AustralianPopulation/AustralianPopulationParameters.lean",
}


class CheckError(RuntimeError):
    pass


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_path(root: Path, raw_path: str) -> Path:
    logical = PurePosixPath(raw_path)
    if logical.is_absolute() or not logical.parts:
        raise CheckError(f"unsafe compatibility path: {raw_path!r}")
    if any(part in {"", ".", ".."} for part in logical.parts):
        raise CheckError(f"unsafe compatibility path: {raw_path!r}")
    if "\\" in raw_path:
        raise CheckError(f"unsafe compatibility path: {raw_path!r}")

    candidate = root.joinpath(*logical.parts)
    try:
        candidate.resolve().relative_to(root.resolve())
    except ValueError as error:
        raise CheckError(f"compatibility path escapes root: {raw_path}") from error
    return candidate


def validate_hash_map(
    value: Any, label: str, required_paths: set[str]
) -> dict[str, str]:
    if not isinstance(value, dict) or not all(
        isinstance(path, str) and isinstance(digest, str)
        for path, digest in value.items()
    ):
        raise CheckError(f"{label} must be an object mapping paths to SHA-256 digests")
    if list(value) != sorted(value):
        raise CheckError(f"{label} paths must be sorted")

    actual_paths = set(value)
    missing = sorted(required_paths - actual_paths)
    extra = sorted(actual_paths - required_paths)
    if missing:
        raise CheckError(f"{label} is missing required paths: {', '.join(missing)}")
    if extra:
        raise CheckError(f"{label} has unexpected paths: {', '.join(extra)}")
    for path, digest in value.items():
        safe_path(Path("."), path)
        if SHA256_RE.fullmatch(digest) is None:
            raise CheckError(f"{label} has invalid SHA-256 for {path}: {digest!r}")
    return value


def validate_pin(
    pin: Any,
    required_tracked: set[str] = REQUIRED_TRACKED_PATHS,
    required_generated: set[str] = REQUIRED_GENERATED_PATHS,
) -> tuple[str, dict[str, str], dict[str, str]]:
    if not isinstance(pin, dict):
        raise CheckError("frontend compatibility pin must be a JSON object")
    expected_keys = {
        "schema",
        "repository",
        "commit",
        "contracts",
        "backend_surface",
    }
    if set(pin) != expected_keys:
        raise CheckError("frontend compatibility pin has unexpected or missing fields")
    if pin["schema"] != PIN_SCHEMA:
        raise CheckError(f"unexpected frontend compatibility schema: {pin['schema']!r}")
    if pin["repository"] != FRONTEND_REPOSITORY:
        raise CheckError(f"unexpected frontend repository: {pin['repository']!r}")
    commit = pin["commit"]
    if not isinstance(commit, str) or COMMIT_RE.fullmatch(commit) is None:
        raise CheckError("frontend compatibility commit must be a full lowercase Git SHA")
    if pin["contracts"] != EXPECTED_CONTRACTS:
        raise CheckError("frontend compatibility contracts do not match the accepted set")

    surface = pin["backend_surface"]
    if not isinstance(surface, dict) or set(surface) != {
        "tracked_sha256",
        "generated_sha256",
    }:
        raise CheckError("backend_surface must contain tracked_sha256 and generated_sha256")
    tracked = validate_hash_map(
        surface["tracked_sha256"], "tracked_sha256", required_tracked
    )
    generated = validate_hash_map(
        surface["generated_sha256"], "generated_sha256", required_generated
    )
    return commit, tracked, generated


def verify_files(root: Path, expected: dict[str, str], label: str) -> None:
    for raw_path, expected_digest in expected.items():
        path = safe_path(root, raw_path)
        if not path.is_file():
            raise CheckError(f"{label} file does not exist: {raw_path}")
        actual_digest = sha256(path)
        if actual_digest != expected_digest:
            raise CheckError(
                f"{label} drift for {raw_path}: expected {expected_digest}, "
                f"got {actual_digest}"
            )


def export_generated_files(root: Path, output_root: Path) -> None:
    exporter = root / "scripts" / "export-frontend-data.sh"
    if not exporter.is_file():
        raise CheckError("missing scripts/export-frontend-data.sh")
    result = subprocess.run(
        [str(exporter), str(output_root)],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        check=False,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CheckError(f"frontend data export failed: {detail}")


def parse_args() -> argparse.Namespace:
    default_root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(
        description="check the frozen backend surface pinned to the Lean frontend"
    )
    parser.add_argument("--root", type=Path, default=default_root)
    parser.add_argument("--pin", type=Path)
    parser.add_argument(
        "--generated-root",
        type=Path,
        help="verify an existing export instead of generating a temporary one",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    pin_path = args.pin or root / "compat" / "frontend.json"
    try:
        pin = json.loads(pin_path.read_text(encoding="utf-8"))
        commit, tracked, generated = validate_pin(pin)
        verify_files(root, tracked, "tracked frontend contract")
        if args.generated_root is not None:
            verify_files(args.generated_root.resolve(), generated, "generated frontend data")
        else:
            with tempfile.TemporaryDirectory(prefix="sembla-frontend-compat-") as temporary:
                generated_root = Path(temporary)
                export_generated_files(root, generated_root)
                verify_files(generated_root, generated, "generated frontend data")
    except (CheckError, OSError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    print(
        "frontend compatibility surface passed: "
        f"{len(tracked)} tracked fixtures, {len(generated)} generated artifacts, "
        f"frontend {commit[:12]}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Structural and policy checks for the Linux/Python-3.12 NPE lock."""

from __future__ import annotations

import hashlib
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[3]
DIRECT = ROOT / "calibration/npe/requirements.txt"
LOCK = ROOT / "calibration/npe/requirements-ci.lock"

EXPECTED_OPTIONS = {
    "--index-url https://pypi.org/simple",
    "--find-links https://download.pytorch.org/whl/cpu/torch/",
}
EXPECTED_BUILD_TOOLS = {
    "setuptools": "75.8.0",
    "wheel": "0.45.1",
}
EXPECTED_TORCH = "2.5.1+cpu"
EXPECTED_TORCH_HASH = (
    "--hash=sha256:4856f9d6925121d13c2df07aa7580b767f449dfe71ae5acde9c27535d5da4840"
)
EXPECTED_METADATA = {
    "# Target: Linux/amd64, CPython 3.12.8, manylinux_2_17.",
    "# Container: python:3.12.8-slim-bookworm@sha256:8859bd6ca943079262c27e38b7119cdacede77c463139a15651dd340087a6cc9",
    "# Generator: uv==0.5.18 (wheel sha256:04e6c62d8947f62f1ec3255b5743cc775950b6203b06bf9c4d50682dcd68f340).",
    "# Resolution-Cutoff: 2026-07-23T00:00:00Z",
    "# Build-Prerequisites: setuptools==75.8.0, wheel==0.45.1.",
    "# Source-Policy: PyPI index plus the torch-only PyTorch CPU find-links page; no extra index.",
}
HASH_RE = re.compile(r"--hash=sha256:[0-9a-f]{64}\Z")
NAME_RE = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")


def canonical_name(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def logical_statements(text: str) -> list[str]:
    statements: list[str] = []
    current: list[str] = []
    for raw in text.splitlines():
        stripped = raw.strip()
        if not stripped or stripped.startswith("#"):
            continue
        continued = stripped.endswith("\\")
        if continued:
            stripped = stripped[:-1].rstrip()
        current.append(stripped)
        if not continued:
            statements.append(" ".join(current))
            current = []
    if current:
        statements.append(" ".join(current))
    return statements


def direct_versions(text: str) -> dict[str, set[str]]:
    versions: dict[str, set[str]] = {}
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith(("#", "--")):
            continue
        requirement = line.split(";", 1)[0].strip()
        match = re.fullmatch(r"([A-Za-z0-9][A-Za-z0-9._-]*)==([^\s]+)", requirement)
        if not match:
            raise ValueError(f"direct input is not exactly pinned: {line}")
        versions.setdefault(canonical_name(match.group(1)), set()).add(match.group(2))
    return versions


def validate(direct_text: str, lock_text: str) -> list[str]:
    errors: list[str] = []
    lower = lock_text.lower()
    forbidden_fragments = (
        "--extra-index-url",
        "--editable",
        "-e ",
        "git+",
        "hg+",
        "svn+",
        "bzr+",
        " @ ",
    )
    for fragment in forbidden_fragments:
        if fragment in lower:
            errors.append(f"forbidden lock syntax: {fragment.strip()}")

    expected_input_hash = hashlib.sha256(direct_text.encode("utf-8")).hexdigest()
    if f"# Input-SHA256: {expected_input_hash}" not in lock_text:
        errors.append("lock Input-SHA256 does not match requirements.txt")
    lock_lines = set(lock_text.splitlines())
    missing_metadata = sorted(EXPECTED_METADATA - lock_lines)
    if missing_metadata:
        errors.append(f"lock metadata contract missing: {missing_metadata}")

    options: list[str] = []
    packages: dict[str, str] = {}
    package_hashes: dict[str, list[str]] = {}
    for statement in logical_statements(lock_text):
        if statement.startswith("--"):
            options.append(statement)
            continue
        match = re.fullmatch(
            r"([A-Za-z0-9][A-Za-z0-9._-]*)==([^\s;]+)(?:\s+(.*))?",
            statement,
        )
        if not match:
            errors.append(f"requirement is not an exact name==version pin: {statement}")
            continue
        name = canonical_name(match.group(1))
        version = match.group(2)
        if name in packages:
            errors.append(f"duplicate locked requirement: {name}")
        packages[name] = version
        hash_tokens = (match.group(3) or "").split()
        package_hashes[name] = hash_tokens
        if not hash_tokens or any(not HASH_RE.fullmatch(token) for token in hash_tokens):
            errors.append(f"requirement is not fully SHA-256 hashed: {name}=={version}")

    if set(options) != EXPECTED_OPTIONS or len(options) != len(EXPECTED_OPTIONS):
        errors.append(
            "lock sources must be exactly the PyPI index and torch-only CPU find-links page"
        )

    direct_options = {
        line.strip() for line in direct_text.splitlines() if line.strip().startswith("--")
    }
    if direct_options != EXPECTED_OPTIONS:
        errors.append(
            "direct input sources must be exactly the PyPI index and torch-only CPU find-links page"
        )

    try:
        direct = direct_versions(direct_text)
    except ValueError as error:
        errors.append(str(error))
        direct = {}
    for name, allowed_versions in direct.items():
        if name not in packages:
            errors.append(f"direct-input package missing from lock: {name}")
        elif packages[name] not in allowed_versions:
            errors.append(
                f"locked {name}=={packages[name]} does not match direct input "
                f"{sorted(allowed_versions)}"
            )

    for name, version in EXPECTED_BUILD_TOOLS.items():
        if packages.get(name) != version:
            errors.append(f"lock must include build prerequisite {name}=={version}")
    if packages.get("torch") != EXPECTED_TORCH:
        errors.append(f"lock must contain CPU torch=={EXPECTED_TORCH}")
    if package_hashes.get("torch") != [EXPECTED_TORCH_HASH]:
        errors.append("lock must contain the verified Linux/CPython-3.12 CPU torch wheel hash")
    cuda_packages = sorted(
        name for name in packages if name == "triton" or name.startswith("nvidia-")
    )
    if cuda_packages:
        errors.append(f"CUDA packages are forbidden in the CPU lock: {', '.join(cuda_packages)}")
    return errors


class RequirementsCiLockTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.direct = DIRECT.read_text(encoding="utf-8")
        cls.lock = LOCK.read_text(encoding="utf-8")

    def assert_rejected(self, mutated: str, fragment: str) -> None:
        errors = validate(self.direct, mutated)
        self.assertTrue(errors, "mutated lock unexpectedly passed")
        self.assertTrue(
            any(fragment in error for error in errors),
            f"expected {fragment!r} in errors: {errors}",
        )

    def test_checked_lock_passes(self) -> None:
        self.assertEqual(validate(self.direct, self.lock), [])

    def test_rejects_unpinned_requirement(self) -> None:
        self.assert_rejected(
            self.lock.replace("numpy==1.26.4", "numpy>=1.26.4", 1),
            "not an exact",
        )

    def test_rejects_unhashed_requirement(self) -> None:
        self.assert_rejected(
            re.sub(
                r"(torch==2\.5\.1\+cpu) \\\n\s+--hash=sha256:[0-9a-f]{64}",
                r"\1",
                self.lock,
                count=1,
            ),
            "not fully SHA-256 hashed",
        )

    def test_rejects_unexpected_index(self) -> None:
        self.assert_rejected(
            self.lock + "\n--extra-index-url https://example.invalid/simple\n",
            "forbidden lock syntax",
        )

    def test_rejects_editable_vcs_requirement(self) -> None:
        self.assert_rejected(
            self.lock + "\n-e git+https://example.invalid/repo.git#egg=bad\n",
            "forbidden lock syntax",
        )

    def test_rejects_missing_direct_package(self) -> None:
        self.assert_rejected(
            self.lock.replace("numpy==1.26.4", "not-numpy==1.26.4", 1),
            "direct-input package missing",
        )

    def test_rejects_cuda_package(self) -> None:
        self.assert_rejected(
            self.lock
            + "\nnvidia-example==1.0 \\\n"
            + "    --hash=sha256:"
            + "0" * 64
            + "\n",
            "CUDA packages are forbidden",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)

#!/usr/bin/env python3
"""Enforce the Rust workspace dependency layers and core-library boundaries."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[1]
WORKSPACE_EDGES = {
    "sembla-ir": set(),
    "sembla-runtime": {"sembla-ir"},
    "sembla-cuda": {"sembla-ir", "sembla-runtime"},
    "sembla-cli": {"sembla-cuda", "sembla-ir", "sembla-runtime"},
}
CORE_SOURCE_DIRS = (
    Path("crates/sembla-ir/src"),
    Path("crates/sembla-runtime/src"),
)
CUDA_VOCABULARY = re.compile(r"\b(?:cuda|nvrtc|cudarc)\b", re.IGNORECASE)
REPORTING_MACRO = re.compile(r"\b(?:print|println|eprint|eprintln)!\s*\(")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=DEFAULT_ROOT,
        help="repository root (defaults to the script's parent repository)",
    )
    parser.add_argument(
        "--metadata-file",
        type=Path,
        help="validate saved Cargo metadata instead of invoking Cargo",
    )
    return parser.parse_args()


def load_metadata(root: Path, metadata_file: Path | None) -> dict[str, object]:
    if metadata_file is None:
        raw = subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=root,
            text=True,
        )
    else:
        raw = metadata_file.read_text(encoding="utf-8")
    metadata = json.loads(raw)
    if not isinstance(metadata, dict):
        raise ValueError("Cargo metadata root must be a JSON object")
    return metadata


def validate_dependency_edges(metadata: dict[str, object]) -> list[str]:
    packages = metadata.get("packages")
    member_ids = metadata.get("workspace_members")
    if not isinstance(packages, list) or not isinstance(member_ids, list):
        raise ValueError("Cargo metadata must contain package and workspace member lists")

    packages_by_id = {
        package.get("id"): package
        for package in packages
        if isinstance(package, dict) and isinstance(package.get("id"), str)
    }
    try:
        members = [packages_by_id[member_id] for member_id in member_ids]
    except KeyError as error:
        raise ValueError(f"workspace member is absent from packages: {error.args[0]}") from error

    members_by_name = {str(package.get("name")): package for package in members}
    errors: list[str] = []
    if set(members_by_name) != set(WORKSPACE_EDGES):
        errors.append(
            "architecture matrix covers exactly "
            f"{', '.join(sorted(WORKSPACE_EDGES))}; workspace contains "
            f"{', '.join(sorted(members_by_name))}"
        )
        return errors

    workspace_names = set(WORKSPACE_EDGES)
    for package_name, expected in WORKSPACE_EDGES.items():
        dependencies = members_by_name[package_name].get("dependencies")
        if not isinstance(dependencies, list):
            raise ValueError(f"{package_name}: dependencies must be a list")
        actual = {
            str(dependency.get("name"))
            for dependency in dependencies
            if isinstance(dependency, dict)
            and dependency.get("name") in workspace_names
        }
        if actual != expected:
            errors.append(
                f"{package_name}: workspace dependencies must be "
                f"[{', '.join(sorted(expected))}]; found [{', '.join(sorted(actual))}]"
            )
    return errors


def source_hits(root: Path, pattern: re.Pattern[str]) -> list[tuple[Path, int, str]]:
    hits: list[tuple[Path, int, str]] = []
    for relative_directory in CORE_SOURCE_DIRS:
        directory = root / relative_directory
        if not directory.is_dir():
            raise ValueError(f"missing core source directory: {relative_directory}")
        for path in sorted(directory.rglob("*.rs")):
            for line_number, line in enumerate(
                path.read_text(encoding="utf-8").splitlines(), start=1
            ):
                if pattern.search(line):
                    hits.append((path.relative_to(root), line_number, line.strip()))
    return hits


def validate_source_boundaries(root: Path) -> list[str]:
    errors: list[str] = []
    for path, line_number, line in source_hits(root, CUDA_VOCABULARY):
        errors.append(
            f"{path}:{line_number}: backend-specific CUDA vocabulary is forbidden "
            f"in sembla-ir/sembla-runtime: {line}"
        )
    for path, line_number, line in source_hits(root, REPORTING_MACRO):
        errors.append(
            f"{path}:{line_number}: direct stdout/stderr reporting is forbidden "
            f"in library crates; return data or errors to the CLI: {line}"
        )
    return errors


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    try:
        metadata = load_metadata(root, args.metadata_file)
        errors = validate_dependency_edges(metadata)
        errors.extend(validate_source_boundaries(root))
    except (OSError, subprocess.CalledProcessError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if errors:
        for error in sorted(errors):
            print(f"error: {error}", file=sys.stderr)
        return 1

    edge_count = sum(len(edges) for edges in WORKSPACE_EDGES.values())
    source_count = sum(
        1
        for directory in CORE_SOURCE_DIRS
        for _ in (root / directory).rglob("*.rs")
    )
    print(
        f"Rust architecture checks passed: {edge_count} allowed workspace edges, "
        f"{source_count} core source files"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

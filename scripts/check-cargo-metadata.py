#!/usr/bin/env python3
"""Validate publication and package metadata for the Cargo workspace."""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_LICENSE = "MIT OR Apache-2.0"
EXPECTED_REPOSITORY = "https://github.com/ianmoran11/sembla"
EXPECTED_PACKAGES = {
    "sembla-cli",
    "sembla-cuda",
    "sembla-ir",
    "sembla-runtime",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--metadata-file",
        type=Path,
        help="validate saved Cargo metadata instead of invoking Cargo",
    )
    return parser.parse_args()


def rust_version() -> str:
    toolchain = (ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = re.search(
        r'^\s*channel\s*=\s*"(?P<version>[0-9]+\.[0-9]+\.[0-9]+)"\s*$',
        toolchain,
        re.MULTILINE,
    )
    if match is None:
        raise ValueError("rust-toolchain.toml must pin a numeric x.y.z channel")
    return match.group("version")


def load_metadata(metadata_file: Path | None) -> dict[str, object]:
    if metadata_file is None:
        raw_metadata = subprocess.check_output(
            [
                "cargo",
                "metadata",
                "--locked",
                "--no-deps",
                "--format-version",
                "1",
            ],
            cwd=ROOT,
            text=True,
        )
    else:
        raw_metadata = metadata_file.read_text(encoding="utf-8")

    metadata = json.loads(raw_metadata)
    if not isinstance(metadata, dict):
        raise ValueError("Cargo metadata root must be a JSON object")
    return metadata


def rendered(value: object) -> str:
    return json.dumps(value, sort_keys=True)


def validate(metadata: dict[str, object]) -> list[str]:
    packages = metadata["packages"]
    member_ids = metadata["workspace_members"]
    if not isinstance(packages, list) or not isinstance(member_ids, list):
        raise ValueError("Cargo metadata must contain package and workspace member lists")

    packages_by_id = {
        package["id"]: package for package in packages if isinstance(package, dict)
    }
    members = [packages_by_id[member_id] for member_id in member_ids]
    member_names = {package.get("name") for package in members}
    errors: list[str] = []

    if member_names != EXPECTED_PACKAGES:
        errors.append(
            "workspace packages must be "
            f"{', '.join(sorted(EXPECTED_PACKAGES))}; "
            f"found {', '.join(sorted(str(name) for name in member_names))}"
        )

    required_fields = {
        "license": EXPECTED_LICENSE,
        "repository": EXPECTED_REPOSITORY,
        "rust_version": rust_version(),
        "publish": [],
    }
    for package in sorted(members, key=lambda item: str(item.get("name"))):
        name = str(package.get("name", "<unnamed>"))
        for field, expected in required_fields.items():
            actual = package.get(field)
            if actual != expected:
                errors.append(
                    f"{name}: {field} must be {rendered(expected)}; "
                    f"found {rendered(actual)}"
                )

        description = package.get("description")
        if not isinstance(description, str) or not description.strip():
            errors.append(
                f"{name}: description must be a non-empty string; "
                f"found {rendered(description)}"
            )

    return sorted(errors)


def main() -> int:
    args = parse_args()
    try:
        metadata = load_metadata(args.metadata_file)
        errors = validate(metadata)
    except (KeyError, OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"error: unable to load Cargo package metadata: {error}", file=sys.stderr)
        return 2

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        print(
            f"error: Cargo package metadata policy failed with {len(errors)} issue(s)",
            file=sys.stderr,
        )
        return 1

    print(
        "checked Cargo publication metadata for "
        f"{len(metadata['workspace_members'])} workspace packages"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

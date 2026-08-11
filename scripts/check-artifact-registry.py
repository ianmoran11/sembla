#!/usr/bin/env python3
"""Check that versioned artifact/protocol identifiers have architectural owners."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_REGISTRY = Path("docs/architecture/artifact-registry.json")
IDENTIFIER_PATTERN = re.compile(
    r"\bsembla(?:[.-][a-z0-9_-]+)+/[a-z0-9_-]*v[0-9]+\b"
)
ALLOWED_KINDS = {
    "artifact-schema",
    "data-contract",
    "encoding",
    "embedded-version",
    "hash-domain",
    "identity-scheme",
    "protocol",
    "test-identifier",
}
ALLOWED_STABILITY = {"frozen", "internal", "legacy", "versioned"}
REQUIRED_ENTRY_KEYS = {
    "identifier",
    "kind",
    "owner",
    "producers",
    "consumers",
    "stability",
    "source_discovery",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    parser.add_argument("--registry", type=Path, default=DEFAULT_REGISTRY)
    return parser.parse_args()


def source_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for crate in sorted((root / "crates").glob("*/src")):
        files.extend(sorted(crate.rglob("*.rs")))
    lean_root = root / "frontend" / "Sembla"
    if lean_root.is_dir():
        files.extend(
            path
            for path in sorted(lean_root.rglob("*.lean"))
            if not path.name.endswith("Tests.lean")
        )
    for python_root in (root / "calibration" / "npe", root / "data" / "abs"):
        if not python_root.is_dir():
            continue
        for path in sorted(python_root.rglob("*.py")):
            relative_parts = path.relative_to(python_root).parts
            if any(part in {"tests", ".venv", "__pycache__"} for part in relative_parts):
                continue
            files.append(path)
    return files


def discover_identifiers(root: Path) -> dict[str, list[str]]:
    discovered: dict[str, list[str]] = {}
    for path in source_files(root):
        for line_number, line in enumerate(
            path.read_text(encoding="utf-8").splitlines(), start=1
        ):
            for identifier in IDENTIFIER_PATTERN.findall(line):
                discovered.setdefault(identifier, []).append(
                    f"{path.relative_to(root)}:{line_number}"
                )
    return discovered


def load_registry(path: Path) -> dict[str, object]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        raise ValueError("artifact registry root must be a JSON object")
    return payload


def validate_registry(root: Path, registry: dict[str, object]) -> list[str]:
    errors: list[str] = []
    if registry.get("registry_schema") != "sembla.architecture-artifact-registry/v1":
        errors.append(
            "registry_schema must be 'sembla.architecture-artifact-registry/v1'"
        )
    entries = registry.get("entries")
    if not isinstance(entries, list):
        raise ValueError("artifact registry entries must be a list")

    entries_by_identifier: dict[str, dict[str, object]] = {}
    for index, raw_entry in enumerate(entries):
        if not isinstance(raw_entry, dict):
            errors.append(f"entry {index}: must be a JSON object")
            continue
        missing = REQUIRED_ENTRY_KEYS - set(raw_entry)
        if missing:
            errors.append(
                f"entry {index}: missing keys {', '.join(sorted(missing))}"
            )
            continue
        identifier = raw_entry.get("identifier")
        if not isinstance(identifier, str) or not identifier:
            errors.append(f"entry {index}: identifier must be a non-empty string")
            continue
        if identifier in entries_by_identifier:
            errors.append(f"duplicate registry identifier: {identifier}")
            continue
        entries_by_identifier[identifier] = raw_entry

        kind = raw_entry.get("kind")
        if kind not in ALLOWED_KINDS:
            errors.append(f"{identifier}: unsupported kind {kind!r}")
        stability = raw_entry.get("stability")
        if stability not in ALLOWED_STABILITY:
            errors.append(f"{identifier}: unsupported stability {stability!r}")
        if not isinstance(raw_entry.get("source_discovery"), bool):
            errors.append(f"{identifier}: source_discovery must be boolean")

        for field in ("owner", "producers", "consumers"):
            values = (
                [raw_entry[field]]
                if field == "owner"
                else raw_entry[field]
            )
            if not isinstance(values, list) or not values or not all(
                isinstance(value, str) and value for value in values
            ):
                errors.append(f"{identifier}: {field} must name one or more paths")
                continue
            if len(values) != len(set(values)):
                errors.append(f"{identifier}: {field} contains duplicate paths")
            for value in values:
                if not (root / value).exists():
                    errors.append(f"{identifier}: {field} path does not exist: {value}")

    discovered = discover_identifiers(root)
    discoverable = {
        identifier
        for identifier, entry in entries_by_identifier.items()
        if entry.get("source_discovery") is True
    }
    for identifier in sorted(set(discovered) - discoverable):
        locations = ", ".join(discovered[identifier][:3])
        errors.append(f"unregistered source identifier {identifier}: {locations}")
    for identifier in sorted(discoverable - set(discovered)):
        errors.append(f"stale source-discovery registry entry: {identifier}")
    return errors


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    registry_path = args.registry
    if not registry_path.is_absolute():
        registry_path = root / registry_path
    try:
        registry = load_registry(registry_path)
        errors = validate_registry(root, registry)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in sorted(errors):
            print(f"error: {error}", file=sys.stderr)
        return 1
    entries = registry["entries"]
    discovered_count = len(discover_identifiers(root))
    print(
        f"artifact registry passed: {len(entries)} entries, "
        f"{discovered_count} source-discovered identifiers"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

#!/usr/bin/env python3
"""Enforce the high-value Lean import boundaries and production/test split."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parents[1]
IMPORT_PATTERN = re.compile(r"^import\s+(?P<module>\S+)\s*$", re.MULTILINE)
CONTRACT_MODULES = {
    "Sembla.IR",
    "Sembla.Hash",
    "Sembla.Json",
    "Sembla.Plan",
    "Sembla.PlanJson",
    "Sembla.PlanExport",
    "Sembla.ParameterTable",
}
HIGH_LEVEL_PREFIXES = (
    "Sembla.DSL",
    "Sembla.Demos",
    "Sembla.Models",
    "Sembla.Tutorial",
    "Sembla.WidgetDisplay",
    "Sembla.Widgets",
    "Sembla.Composition.Surface",
    "Sembla.Composition.Widget",
)
COMPOSITION_CORE_MODULES = {
    "Sembla.Composition.Bundle",
    "Sembla.Composition.Errors",
    "Sembla.Composition.Json",
    "Sembla.Composition.Link",
    "Sembla.Composition.Source",
    "Sembla.Composition.SourceMap",
    "Sembla.Composition.SpecObservation",
    "Sembla.Composition.SpecStatements",
    "Sembla.Composition.SpecStatic",
}
SEMANTICS_RAW_COMPOSITION_IMPORTS = {
    "Sembla.Composition.Source",
    "Sembla.Composition.SourceMap",
}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT)
    return parser.parse_args()


def module_for(root: Path, path: Path) -> str:
    relative = path.relative_to(root).with_suffix("")
    return ".".join(relative.parts)


def is_test_module(module: str) -> bool:
    return module == "SemblaTests" or any(
        part.endswith("Tests") for part in module.split(".")
    ) or module.endswith(".Validation")


def import_graph(root: Path) -> dict[str, set[str]]:
    paths = [root / "Sembla.lean", root / "SemblaTests.lean", root / "Main.lean", root / "LinkMain.lean"]
    paths.extend(sorted((root / "Sembla").rglob("*.lean")))
    graph: dict[str, set[str]] = {}
    for path in paths:
        if not path.is_file():
            continue
        graph[module_for(root, path)] = set(
            IMPORT_PATTERN.findall(path.read_text(encoding="utf-8"))
        )
    return graph


def has_prefix(module: str, prefixes: tuple[str, ...]) -> bool:
    return any(module == prefix or module.startswith(prefix + ".") for prefix in prefixes)


def validate_lake_contract(root: Path) -> list[str]:
    errors: list[str] = []
    lakefile = root / "lakefile.toml"
    if not lakefile.is_file():
        return ["missing lakefile.toml"]
    content = lakefile.read_text(encoding="utf-8")
    default_match = re.search(r"^defaultTargets\s*=\s*\[(?P<targets>[^]]*)\]", content, re.MULTILINE)
    if default_match is None or '"SemblaTests"' not in default_match.group("targets"):
        errors.append("lakefile.toml defaultTargets must include SemblaTests")
    libraries = re.findall(
        r"\[\[lean_lib\]\]\s*\nname\s*=\s*\"(?P<name>[^\"]+)\"",
        content,
    )
    if "Sembla" not in libraries or "SemblaTests" not in libraries:
        errors.append("lakefile.toml must declare separate Sembla and SemblaTests libraries")
    return errors


def validate_graph(graph: dict[str, set[str]]) -> list[str]:
    errors: list[str] = []
    if "Sembla" not in graph:
        errors.append("missing production umbrella Sembla.lean")
    if "SemblaTests" not in graph:
        errors.append("missing test umbrella SemblaTests.lean")
    elif "Sembla" not in graph["SemblaTests"]:
        errors.append("SemblaTests must import the production Sembla umbrella")

    for module, dependencies in sorted(graph.items()):
        if not is_test_module(module):
            for dependency in sorted(dependencies):
                if is_test_module(dependency):
                    errors.append(
                        f"{module}: production module must not import test module {dependency}"
                    )

        if module in CONTRACT_MODULES:
            for dependency in sorted(dependencies):
                if has_prefix(dependency, HIGH_LEVEL_PREFIXES):
                    errors.append(
                        f"{module}: contract module must not import higher-level {dependency}"
                    )

        if module.startswith("Sembla.Semantics.") and not is_test_module(module):
            for dependency in sorted(dependencies):
                if dependency.startswith("Sembla.Composition."):
                    allowed = (
                        module == "Sembla.Semantics.Raw"
                        and dependency in SEMANTICS_RAW_COMPOSITION_IMPORTS
                    )
                    if not allowed:
                        errors.append(
                            f"{module}: semantics may not import composition module {dependency}"
                        )
                if has_prefix(dependency, HIGH_LEVEL_PREFIXES):
                    errors.append(
                        f"{module}: semantics may not import higher-level {dependency}"
                    )

        if module in COMPOSITION_CORE_MODULES:
            for dependency in sorted(dependencies):
                if has_prefix(dependency, HIGH_LEVEL_PREFIXES) or dependency == "Sembla.Composition.Fixtures":
                    errors.append(
                        f"{module}: composition core must not import surface/test dependency {dependency}"
                    )

    for entrypoint in ("Main", "LinkMain", "Sembla"):
        if "SemblaTests" in graph.get(entrypoint, set()):
            errors.append(f"{entrypoint}: production entrypoint must not import SemblaTests")
    return errors


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    try:
        graph = import_graph(root)
        errors = validate_lake_contract(root)
        errors.extend(validate_graph(graph))
    except OSError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    if errors:
        for error in sorted(set(errors)):
            print(f"error: {error}", file=sys.stderr)
        return 1
    edge_count = sum(len(dependencies) for dependencies in graph.values())
    print(f"Lean import architecture passed: {len(graph)} modules, {edge_count} import edges")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

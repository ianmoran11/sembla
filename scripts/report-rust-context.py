#!/usr/bin/env python3
"""Report and optionally enforce the backend Rust context budget.

The report deliberately uses a dependency-free lexical metric. It is not a
replacement for compiler analysis, but it is deterministic, cheap enough for
every Rust check, and useful for ratcheting source and decision complexity
downward. Test-only source files are excluded; production files should keep
large ``#[cfg(test)]`` modules in adjacent ``*_tests.rs`` files.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from dataclasses import asdict, dataclass
from pathlib import Path


DEFAULT_ROOT = Path(__file__).resolve().parents[1]
BACKEND_CRATES = (
    "sembla-ir",
    "sembla-runtime",
    "sembla-cpu",
    "sembla-cuda",
    "sembla-cli",
)
TEST_FILE_NAMES = {"tests.rs"}
TOKEN = re.compile(
    r"::|->|=>|&&|\|\||==|!=|<=|>=|\.\.=|\.\.|"
    r"[A-Za-z_][A-Za-z0-9_]*|[0-9]+(?:\.[0-9]+)?|[^\s]"
)
FUNCTION = re.compile(
    r"\b(?:pub(?:\s*\([^)]*\))?\s+)?"
    r"(?:async\s+)?(?:const\s+)?(?:unsafe\s+)?"
    r'(?:extern(?:\s+"[^"]+")?\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)'
)
DECISIONS = {"if", "match", "for", "while", "loop", "&&", "||"}


@dataclass(frozen=True)
class FunctionMetric:
    path: str
    name: str
    start_line: int
    lines: int
    complexity: int


@dataclass(frozen=True)
class FileMetric:
    path: str
    code_lines: int
    tokens: int


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=DEFAULT_ROOT,
        help="repository root (defaults to the script's parent repository)",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="write the complete report as JSON",
    )
    parser.add_argument(
        "--top",
        type=int,
        default=10,
        help="number of large/complex functions to show in the text report",
    )
    parser.add_argument(
        "--check",
        type=Path,
        metavar="BUDGET",
        help="fail when the report exceeds limits in a budget JSON file",
    )
    return parser.parse_args()


def mask_non_code(source: str) -> str:
    """Replace comments and literals with spaces while preserving newlines."""

    output: list[str] = []
    index = 0
    block_depth = 0
    state = "code"
    raw_hashes = 0
    while index < len(source):
        char = source[index]
        following = source[index + 1] if index + 1 < len(source) else ""

        if state == "line-comment":
            if char == "\n":
                output.append("\n")
                state = "code"
            else:
                output.append(" ")
            index += 1
            continue

        if state == "block-comment":
            if char == "/" and following == "*":
                output.extend((" ", " "))
                block_depth += 1
                index += 2
            elif char == "*" and following == "/":
                output.extend((" ", " "))
                block_depth -= 1
                index += 2
                if block_depth == 0:
                    state = "code"
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue

        if state == "string":
            if char == "\\":
                output.append(" ")
                if following:
                    output.append("\n" if following == "\n" else " ")
                    index += 2
                else:
                    index += 1
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
                if char == '"':
                    state = "code"
            continue

        if state == "raw-string":
            terminator = '"' + ("#" * raw_hashes)
            if source.startswith(terminator, index):
                output.extend(" " * len(terminator))
                index += len(terminator)
                state = "code"
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
            continue

        if char == "/" and following == "/":
            output.extend((" ", " "))
            index += 2
            state = "line-comment"
        elif char == "/" and following == "*":
            output.extend((" ", " "))
            index += 2
            block_depth = 1
            state = "block-comment"
        elif char == '"':
            output.append(" ")
            index += 1
            state = "string"
        elif char == "r":
            raw_match = re.match(r'r(#{0,255})"', source[index:])
            if raw_match is None:
                output.append(char)
                index += 1
            else:
                raw_hashes = len(raw_match.group(1))
                output.extend(" " * len(raw_match.group(0)))
                index += len(raw_match.group(0))
                state = "raw-string"
        elif char == "'":
            # Mask character literals but leave Rust lifetimes intact.
            char_match = re.match(r"'(?:\\.|[^\\'\n])'", source[index:])
            if char_match is None:
                output.append(char)
                index += 1
            else:
                output.extend(" " * len(char_match.group(0)))
                index += len(char_match.group(0))
        else:
            output.append(char)
            index += 1

    return "".join(output)


def matching_brace(source: str, opening: int) -> int | None:
    depth = 0
    for index in range(opening, len(source)):
        if source[index] == "{":
            depth += 1
        elif source[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    return None


def function_metrics(relative_path: Path, masked: str) -> list[FunctionMetric]:
    metrics: list[FunctionMetric] = []
    for match in FUNCTION.finditer(masked):
        brace = masked.find("{", match.end())
        semicolon = masked.find(";", match.end())
        if brace < 0 or (semicolon >= 0 and semicolon < brace):
            continue
        closing = matching_brace(masked, brace)
        if closing is None:
            continue
        start_line = masked.count("\n", 0, match.start()) + 1
        end_line = masked.count("\n", 0, closing) + 1
        body_tokens = TOKEN.findall(masked[brace + 1 : closing])
        complexity = 1 + sum(token in DECISIONS for token in body_tokens)
        metrics.append(
            FunctionMetric(
                path=str(relative_path),
                name=match.group(1),
                start_line=start_line,
                lines=end_line - start_line + 1,
                complexity=complexity,
            )
        )
    return metrics


def is_production_source(path: Path, source_directory: Path) -> bool:
    relative = path.relative_to(source_directory)
    return (
        path.name not in TEST_FILE_NAMES
        and not path.stem.endswith("_tests")
        and "tests" not in relative.parts
    )


def build_report(root: Path) -> dict[str, object]:
    crate_reports: dict[str, dict[str, int]] = {}
    files: list[FileMetric] = []
    functions: list[FunctionMetric] = []

    for crate in BACKEND_CRATES:
        source_directory = root / "crates" / crate / "src"
        if not source_directory.is_dir():
            raise ValueError(f"missing backend source directory: {source_directory}")
        crate_files: list[FileMetric] = []
        crate_functions: list[FunctionMetric] = []
        for path in sorted(source_directory.rglob("*.rs")):
            if not is_production_source(path, source_directory):
                continue
            source = path.read_text(encoding="utf-8")
            masked = mask_non_code(source)
            relative_path = path.relative_to(root)
            metric = FileMetric(
                path=str(relative_path),
                code_lines=sum(bool(line.strip()) for line in masked.splitlines()),
                tokens=len(TOKEN.findall(masked)),
            )
            crate_files.append(metric)
            crate_functions.extend(function_metrics(relative_path, masked))

        files.extend(crate_files)
        functions.extend(crate_functions)
        crate_reports[crate] = {
            "files": len(crate_files),
            "code_lines": sum(metric.code_lines for metric in crate_files),
            "tokens": sum(metric.tokens for metric in crate_files),
            "max_file_code_lines": max(metric.code_lines for metric in crate_files),
        }

    largest_file = max(files, key=lambda metric: metric.code_lines)
    longest_function = max(functions, key=lambda metric: metric.lines)
    most_complex_function = max(functions, key=lambda metric: metric.complexity)
    return {
        "metric": {
            "tokens": "dependency-free Rust lexical tokens",
            "complexity": "1 + if/match/for/while/loop/&&/|| token count",
            "scope": "backend crate src files excluding tests.rs and *_tests.rs",
        },
        "totals": {
            "files": len(files),
            "code_lines": sum(metric.code_lines for metric in files),
            "tokens": sum(metric.tokens for metric in files),
        },
        "crates": crate_reports,
        "maxima": {
            "file_code_lines": asdict(largest_file),
            "function_lines": asdict(longest_function),
            "function_complexity": asdict(most_complex_function),
        },
        "files": [asdict(metric) for metric in files],
        "functions": [asdict(metric) for metric in functions],
    }


def check_budget(report: dict[str, object], budget: dict[str, object]) -> list[str]:
    limits = budget.get("limits")
    if not isinstance(limits, dict):
        raise ValueError("context budget must contain a limits object")
    crates = report["crates"]
    maxima = report["maxima"]
    actual = {
        "total_tokens": report["totals"]["tokens"],
        "max_file_code_lines": maxima["file_code_lines"]["code_lines"],
        "max_function_lines": maxima["function_lines"]["lines"],
        "max_function_complexity": maxima["function_complexity"]["complexity"],
    }
    errors = [
        f"{name}: {actual[name]} exceeds budget {limit}"
        for name, limit in limits.items()
        if name in actual and actual[name] > limit
    ]

    crate_token_limits = budget.get("crate_token_limits", {})
    if not isinstance(crate_token_limits, dict):
        raise ValueError("crate_token_limits must be an object")
    for crate, limit in crate_token_limits.items():
        if crate not in crates:
            errors.append(f"unknown crate in context budget: {crate}")
        elif crates[crate]["tokens"] > limit:
            errors.append(
                f"{crate} tokens: {crates[crate]['tokens']} exceeds budget {limit}"
            )
    crate_file_limits = budget.get("crate_file_line_limits", {})
    if not isinstance(crate_file_limits, dict):
        raise ValueError("crate_file_line_limits must be an object")
    for crate, limit in crate_file_limits.items():
        if crate not in crates:
            errors.append(f"unknown crate in context budget: {crate}")
        elif crates[crate]["max_file_code_lines"] > limit:
            errors.append(
                f"{crate} max file lines: {crates[crate]['max_file_code_lines']} "
                f"exceeds budget {limit}"
            )
    return errors


def text_report(report: dict[str, object], top: int) -> str:
    lines = ["Backend Rust context report (production src only)", ""]
    lines.append(
        f"{'crate':<18} {'files':>5} {'code lines':>11} "
        f"{'tokens':>10} {'max file':>9}"
    )
    for crate in BACKEND_CRATES:
        metric = report["crates"][crate]
        lines.append(
            f"{crate:<18} {metric['files']:>5} "
            f"{metric['code_lines']:>11} {metric['tokens']:>10} "
            f"{metric['max_file_code_lines']:>9}"
        )
    totals = report["totals"]
    lines.append(
        f"{'TOTAL':<18} {totals['files']:>5} "
        f"{totals['code_lines']:>11} {totals['tokens']:>10} {'-':>9}"
    )

    functions = report["functions"]
    complex_functions = sorted(
        functions,
        key=lambda metric: (metric["complexity"], metric["lines"]),
        reverse=True,
    )[:top]
    long_functions = sorted(
        functions,
        key=lambda metric: (metric["lines"], metric["complexity"]),
        reverse=True,
    )[:top]
    lines.extend(("", "Highest decision complexity:"))
    for metric in complex_functions:
        lines.append(
            f"  {metric['complexity']:>3}  {metric['lines']:>4} lines  "
            f"{metric['path']}:{metric['start_line']} {metric['name']}"
        )
    lines.extend(("", "Longest functions:"))
    for metric in long_functions:
        lines.append(
            f"  {metric['lines']:>4} lines  complexity {metric['complexity']:>3}  "
            f"{metric['path']}:{metric['start_line']} {metric['name']}"
        )
    return "\n".join(lines)


def main() -> int:
    args = parse_args()
    try:
        report = build_report(args.root.resolve())
        errors: list[str] = []
        if args.check is not None:
            budget_path = args.check
            if not budget_path.is_absolute():
                budget_path = args.root / budget_path
            budget = json.loads(budget_path.read_text(encoding="utf-8"))
            errors = check_budget(report, budget)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if args.json:
        print(json.dumps(report, indent=2, sort_keys=True))
    else:
        print(text_report(report, args.top))
    for error in errors:
        print(f"error: Rust context budget exceeded: {error}", file=sys.stderr)
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())

"""Canonical CSV and JSON writers.

Every derived artifact in this pipeline must regenerate byte for byte, so the
formatting rules are frozen here rather than restated at each call site:
LF line endings, a header row, rows sorted by the full key tuple, integers
rendered without separators, and reals at a fixed precision.
"""

from __future__ import annotations

import json
import pathlib

# Reals are rendered at this many decimal places. Life-table qx values need
# more precision than counts do, and a single fixed width keeps the output
# stable across platforms and Python versions.
REAL_PRECISION = 9


def format_value(value) -> str:
    if isinstance(value, bool):
        raise TypeError("booleans are not a CSV value type in this pipeline")
    if isinstance(value, int):
        return str(value)
    if isinstance(value, float):
        if value != value or value in (float("inf"), float("-inf")):
            raise ValueError(f"non-finite value {value!r}")
        text = f"{value:.{REAL_PRECISION}f}"
        # Trim trailing zeros but keep at least one decimal digit so a real is
        # always visually distinct from an integer.
        if "." in text:
            text = text.rstrip("0")
            if text.endswith("."):
                text += "0"
        return text
    return str(value)


def _quote(field: str) -> str:
    if any(ch in field for ch in (",", '"', "\n", "\r")):
        return '"' + field.replace('"', '""') + '"'
    return field


def write_csv(path, header: list[str], rows, sort: bool = True) -> int:
    """Write ``rows`` under ``header`` in canonical form; return the row count."""
    materialised = [tuple(row) for row in rows]
    for row in materialised:
        if len(row) != len(header):
            raise ValueError(
                f"row {row!r} does not match header width {len(header)}"
            )
    if sort:
        materialised.sort(key=lambda row: tuple(_sort_key(v) for v in row))

    lines = [",".join(_quote(h) for h in header)]
    lines.extend(
        ",".join(_quote(format_value(v)) for v in row) for row in materialised
    )
    text = "\n".join(lines) + "\n"

    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="")
    return len(materialised)


def _sort_key(value):
    # Numbers sort numerically, strings lexicographically, and the two never
    # mix within a column in this pipeline.
    if isinstance(value, (int, float)):
        return (0, value, "")
    return (1, 0, str(value))


def read_csv(path) -> tuple[list[str], list[list[str]]]:
    """Read a canonical CSV back as ``(header, rows)`` of raw strings."""
    text = pathlib.Path(path).read_text(encoding="utf-8")
    lines = text.split("\n")
    if lines and lines[-1] == "":
        lines.pop()
    parsed = [_split(line) for line in lines]
    if not parsed:
        return [], []
    return parsed[0], parsed[1:]


def _split(line: str) -> list[str]:
    out, field, in_quotes, i = [], [], False, 0
    while i < len(line):
        ch = line[i]
        if in_quotes:
            if ch == '"':
                if i + 1 < len(line) and line[i + 1] == '"':
                    field.append('"')
                    i += 1
                else:
                    in_quotes = False
            else:
                field.append(ch)
        elif ch == '"':
            in_quotes = True
        elif ch == ",":
            out.append("".join(field))
            field = []
        else:
            field.append(ch)
        i += 1
    out.append("".join(field))
    return out


def write_json(path, payload) -> None:
    """Write canonical JSON: sorted keys, compact separators, one newline."""
    text = json.dumps(payload, sort_keys=True, indent=2, ensure_ascii=False)
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text + "\n", encoding="utf-8", newline="")

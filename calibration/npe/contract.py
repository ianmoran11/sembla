"""Validation for the PRD-0006 training-pairs contract.

This module intentionally uses only the Python standard library. Contract refusal
therefore remains testable even when the optional NPE dependencies are absent.
"""

from __future__ import annotations

import csv
import hashlib
import json
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Any

SUPPORTED_PAIRS_SCHEMA = 1


class ContractError(ValueError):
    """Raised when an input is not an acceptable PRD-0006 pairs artifact."""


@dataclass(frozen=True)
class PairsArtifact:
    path: Path
    metadata_path: Path
    metadata: dict[str, Any]
    rows: tuple[dict[str, float], ...]
    parameter_columns: tuple[str, ...]
    summary_columns: tuple[str, ...]
    pairs_sha256: str
    metadata_sha256: str


def metadata_path_for(pairs_path: Path) -> Path:
    return Path(f"{pairs_path}.meta.json")


def _require_object(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ContractError(f"invalid pairs metadata: '{field}' must be an object")
    return value


def _require_string_list(metadata: dict[str, Any], field: str) -> tuple[str, ...]:
    value = metadata.get(field)
    if not isinstance(value, list) or not value or not all(
        isinstance(item, str) and item for item in value
    ):
        raise ContractError(
            f"invalid pairs metadata: '{field}' must be a non-empty string array"
        )
    if len(set(value)) != len(value):
        raise ContractError(f"invalid pairs metadata: '{field}' contains duplicates")
    return tuple(value)


def load_pairs(
    pairs_path: str | Path, metadata_path: str | Path | None = None
) -> PairsArtifact:
    """Read and validate exactly one CSV plus its canonical metadata sidecar."""

    path = Path(pairs_path)
    sidecar = Path(metadata_path) if metadata_path else metadata_path_for(path)
    try:
        csv_bytes = path.read_bytes()
    except OSError as error:
        raise ContractError(f"cannot read pairs CSV '{path}': {error}") from error
    try:
        metadata_bytes = sidecar.read_bytes()
    except OSError as error:
        raise ContractError(f"cannot read pairs metadata '{sidecar}': {error}") from error

    try:
        metadata_value = json.loads(metadata_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ContractError(f"invalid pairs metadata JSON '{sidecar}': {error}") from error
    metadata = _require_object(metadata_value, "root")

    schemas = _require_object(metadata.get("schema_versions"), "schema_versions")
    schema_major = schemas.get("pairs")
    if schema_major != SUPPORTED_PAIRS_SCHEMA:
        raise ContractError(
            "unsupported pairs schema major "
            f"{schema_major!r}; supported major is {SUPPORTED_PAIRS_SCHEMA}"
        )

    noise_mode = metadata.get("noise_mode")
    if noise_mode == "crn":
        raise ContractError(
            "refusing CRN-mode pairs: noise_mode 'crn' is unsuitable for NPE "
            "training (DECISIONS.md §G5)"
        )
    if noise_mode != "independent":
        raise ContractError(
            f"unsupported noise_mode {noise_mode!r}; expected 'independent'"
        )

    hash_algorithm = metadata.get("pairs_hash_algorithm")
    if hash_algorithm != "sha256":
        raise ContractError(
            f"unsupported pairs hash algorithm {hash_algorithm!r}; expected 'sha256'"
        )
    actual_hash = hashlib.sha256(csv_bytes).hexdigest()
    expected_hash = metadata.get("pairs_sha256")
    if expected_hash != actual_hash:
        raise ContractError(
            "pairs_sha256 mismatch: "
            f"metadata records {expected_hash!r}, CSV bytes hash to '{actual_hash}'"
        )

    parameters = _require_string_list(metadata, "parameter_columns")
    summaries = _require_string_list(metadata, "summary_columns")
    if tuple(sorted(parameters)) != parameters:
        raise ContractError(
            "invalid pairs metadata: parameter_columns must be sorted by name"
        )
    overlap = set(parameters).intersection(summaries)
    if overlap:
        raise ContractError(
            "invalid pairs metadata: parameter and summary columns overlap: "
            + ", ".join(sorted(overlap))
        )

    try:
        text = csv_bytes.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ContractError(f"pairs CSV '{path}' is not UTF-8: {error}") from error
    reader = csv.DictReader(text.splitlines())
    expected_header = ("k", *parameters, *summaries)
    if tuple(reader.fieldnames or ()) != expected_header:
        raise ContractError(
            "pairs CSV column contract mismatch: expected "
            f"{list(expected_header)!r}, got {reader.fieldnames!r}"
        )

    rows: list[dict[str, float]] = []
    for row_index, raw_row in enumerate(reader):
        if None in raw_row:
            raise ContractError(f"pairs CSV row {row_index} has extra fields")
        parsed: dict[str, float] = {}
        try:
            draw = int(raw_row["k"])
        except (TypeError, ValueError) as error:
            raise ContractError(
                f"pairs CSV row {row_index} has invalid draw index {raw_row.get('k')!r}"
            ) from error
        if draw != row_index:
            raise ContractError(
                f"pairs CSV draw indices must be contiguous from zero; row "
                f"{row_index} records {draw}"
            )
        parsed["k"] = float(draw)
        for column in (*parameters, *summaries):
            try:
                value = float(raw_row[column])
            except (TypeError, ValueError) as error:
                raise ContractError(
                    f"pairs CSV row {row_index} column '{column}' is not numeric"
                ) from error
            if not math.isfinite(value):
                raise ContractError(
                    f"pairs CSV row {row_index} column '{column}' is not finite"
                )
            parsed[column] = value
        rows.append(parsed)

    draws = metadata.get("draws")
    if not isinstance(draws, int) or draws < 1 or draws != len(rows):
        raise ContractError(
            f"pairs draw-count mismatch: metadata records {draws!r}, CSV has {len(rows)} rows"
        )

    return PairsArtifact(
        path=path,
        metadata_path=sidecar,
        metadata=metadata,
        rows=tuple(rows),
        parameter_columns=parameters,
        summary_columns=summaries,
        pairs_sha256=actual_hash,
        metadata_sha256=hashlib.sha256(metadata_bytes).hexdigest(),
    )

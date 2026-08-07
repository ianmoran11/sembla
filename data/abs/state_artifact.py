"""Streaming writer for the frozen ``sembla.state/v1`` artifact format.

The model JSON is the schema authority.  Table order, column order, primitive
types, enum variants and reference targets are read from that export rather
than repeated in Python.  Values are streamed one column at a time so writing a
full-scale state does not require a second artifact-sized allocation.
"""

from __future__ import annotations

from collections.abc import Callable, Iterable, Mapping
from dataclasses import dataclass
import hashlib
import json
import math
import os
import pathlib
import struct
import tempfile


STATE_MAGIC = b"SEMBLA_STATE"
STATE_SCHEMA = "sembla.state/v1"
STATE_HASH_DOMAIN = "sembla.state-artifact/v1"
CHUNK_VALUES = 8_192

TableKey = tuple[str, str]
ColumnKey = tuple[str, str, str]
ValueSource = Iterable[object] | Callable[[], Iterable[object]]


@dataclass(frozen=True)
class StateDigest:
    algorithm: str
    domain: str
    digest: str

    def record(self) -> str:
        return f"state {self.algorithm} {self.domain} {self.digest}"


def load_model(path) -> dict:
    """Load an exported model JSON object, preserving declaration order."""
    path = pathlib.Path(path)
    payload = json.loads(path.read_text(encoding="utf-8"))
    _model_tables(payload)
    return payload


def companion_path(artifact_path) -> pathlib.Path:
    """Return the existing ``synth-state`` companion naming convention."""
    return pathlib.Path(str(pathlib.Path(artifact_path)) + ".model.json")


def write_companion_bytes(artifact_path, payload: bytes) -> pathlib.Path:
    """Write already-canonical exported model bytes beside an artifact."""
    model = json.loads(payload.decode("utf-8"))
    _model_tables(model)
    path = companion_path(artifact_path)
    _atomic_write(path, payload)
    return path


def resized_model(model: Mapping, row_counts: Mapping[TableKey, int]) -> dict:
    """Return a declaration-preserving copy with selected table sizes changed."""
    copied = json.loads(json.dumps(model, ensure_ascii=False))
    tables = _model_tables(copied)
    unknown = set(row_counts) - set(tables)
    if unknown:
        raise ValueError(f"row counts name unknown model tables: {sorted(unknown)!r}")
    for key, count in row_counts.items():
        if isinstance(count, bool) or not isinstance(count, int) or count < 0:
            raise ValueError(f"row count for {key!r} must be a non-negative integer")
        tables[key]["size_hint"] = count
    return copied


def state_digest(path) -> StateDigest:
    """Compute the exact domain-separated digest printed by ``state-hash``."""
    digest = hashlib.sha256()
    digest.update(STATE_HASH_DOMAIN.encode("utf-8"))
    digest.update(b"\0")
    with pathlib.Path(path).open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return StateDigest("sha256", STATE_HASH_DOMAIN, digest.hexdigest())


def raw_sha256(path) -> str:
    """Return ordinary SHA-256 over the artifact bytes for file pinning."""
    digest = hashlib.sha256()
    with pathlib.Path(path).open("rb") as source:
        while chunk := source.read(1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def write_state(
    path,
    model: Mapping,
    columns: Mapping[ColumnKey, ValueSource],
) -> StateDigest:
    """Write one canonical artifact and return its domain-separated digest.

    Every model attribute needs exactly one value source.  A source may be an
    iterable or a zero-argument factory returning an iterable; factories are
    convenient for large deterministic columns.  Empty-schema tables need no
    source but retain their model-declared row count in the header.
    """
    tables = _model_tables(model)
    required = {
        (box_name, table["name"], attr["name"])
        for (box_name, _), table in tables.items()
        for attr in table["attrs"]
    }
    supplied = set(columns)
    if supplied != required:
        missing = sorted(required - supplied)
        extra = sorted(supplied - required)
        raise ValueError(f"column sources do not match model; missing={missing!r}, extra={extra!r}")

    header = _state_header(model)
    header_bytes = json.dumps(
        header,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    if len(header_bytes) > 0xFFFF_FFFF:
        raise ValueError("state artifact header exceeds u32 length")

    destination = pathlib.Path(path)
    destination.parent.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.NamedTemporaryFile(
        mode="wb", dir=destination.parent,
        prefix=f".{destination.name}.", suffix=".tmp", delete=False,
    )
    temp_path = pathlib.Path(temporary.name)
    try:
        with temporary as output:
            output.write(STATE_MAGIC)
            output.write(struct.pack("<I", len(header_bytes)))
            output.write(header_bytes)
            for box in model["boxes"]:
                box_name = box["name"]
                for table in box["tables"]:
                    row_count = table["size_hint"]
                    for attr in table["attrs"]:
                        key = (box_name, table["name"], attr["name"])
                        source = columns[key]
                        values = source() if callable(source) else source
                        _write_column(
                            output,
                            values,
                            row_count,
                            attr,
                            tables,
                            box_name,
                            table["name"],
                        )
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp_path, destination)
        os.chmod(destination, 0o644)
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise
    return state_digest(destination)


def _model_tables(model: Mapping) -> dict[TableKey, dict]:
    if not isinstance(model, Mapping) or not isinstance(model.get("boxes"), list):
        raise ValueError("exported model JSON must contain a boxes array")
    result: dict[TableKey, dict] = {}
    for box in model["boxes"]:
        if not isinstance(box, dict) or not isinstance(box.get("name"), str):
            raise ValueError("every model box needs a string name")
        if not isinstance(box.get("tables"), list):
            raise ValueError(f"box {box['name']!r} needs a tables array")
        for table in box["tables"]:
            if not isinstance(table, dict) or not isinstance(table.get("name"), str):
                raise ValueError(f"box {box['name']!r} has a table without a string name")
            count = table.get("size_hint")
            if isinstance(count, bool) or not isinstance(count, int) or count < 0:
                raise ValueError(
                    f"table {(box['name'], table['name'])!r} needs a non-negative size_hint"
                )
            if not isinstance(table.get("attrs"), list):
                raise ValueError(f"table {(box['name'], table['name'])!r} needs an attrs array")
            key = (box["name"], table["name"])
            if key in result:
                raise ValueError(f"duplicate model table {key!r}")
            names = set()
            for attr in table["attrs"]:
                if not isinstance(attr, dict) or not isinstance(attr.get("name"), str):
                    raise ValueError(f"table {key!r} has an attribute without a string name")
                if attr["name"] in names:
                    raise ValueError(f"duplicate model column {key + (attr['name'],)!r}")
                names.add(attr["name"])
                _column_kind(attr)
            result[key] = table
    return result


def _column_kind(attr: Mapping) -> str:
    ty = attr.get("ty")
    kind = ty.get("kind") if isinstance(ty, Mapping) else None
    if kind not in {"real", "int", "enum", "ref"}:
        raise ValueError(f"unsupported model column type for {attr.get('name')!r}: {kind!r}")
    if kind == "enum":
        variants = ty.get("variants")
        if not isinstance(variants, list) or not variants or not all(
            isinstance(value, str) for value in variants
        ):
            raise ValueError(f"enum column {attr.get('name')!r} needs variants")
        if len(variants) > 0x1_0000:
            raise ValueError(f"enum column {attr.get('name')!r} exceeds u16 variants")
    if kind == "ref" and not isinstance(ty.get("table"), str):
        raise ValueError(f"ref column {attr.get('name')!r} needs a target table")
    return kind


def _state_header(model: Mapping) -> dict:
    tables = []
    for box in model["boxes"]:
        for table in box["tables"]:
            columns = []
            for attr in table["attrs"]:
                kind = _column_kind(attr)
                column = {"name": attr["name"], "type": kind}
                if kind == "enum":
                    column["variant_count"] = len(attr["ty"]["variants"])
                elif kind == "ref":
                    column["ref_target"] = {
                        "box": box["name"],
                        "table": attr["ty"]["table"],
                    }
                columns.append(column)
            tables.append({
                "box": box["name"],
                "columns": columns,
                "row_count": table["size_hint"],
                "table": table["name"],
            })
    return {"schema_version": STATE_SCHEMA, "tables": tables}


def _write_column(
    output,
    values: Iterable[object],
    row_count: int,
    attr: Mapping,
    tables: Mapping[TableKey, Mapping],
    box_name: str,
    table_name: str,
) -> None:
    kind = _column_kind(attr)
    context = f"{box_name}.{table_name}.{attr['name']}"
    target_rows = None
    variants = None
    if kind == "ref":
        target = (box_name, attr["ty"]["table"])
        if target not in tables:
            raise ValueError(f"{context} has unresolved reference target {target!r}")
        target_rows = tables[target]["size_hint"]
    elif kind == "enum":
        variants = attr["ty"]["variants"]

    chunk = bytearray()
    actual = 0
    for actual, value in enumerate(values, start=1):
        if actual > row_count:
            raise ValueError(f"{context} has more than {row_count} values")
        chunk.extend(_encode_value(value, kind, context, actual - 1, variants, target_rows))
        if actual % CHUNK_VALUES == 0:
            output.write(chunk)
            chunk.clear()
    if actual != row_count:
        raise ValueError(f"{context} has {actual} values; expected {row_count}")
    if chunk:
        output.write(chunk)


def _encode_value(value, kind: str, context: str, row: int,
                  variants: list[str] | None, target_rows: int | None) -> bytes:
    if kind == "real":
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            raise ValueError(f"{context} row {row} is not a real")
        real = float(value)
        if not math.isfinite(real):
            raise ValueError(f"{context} row {row} is not finite")
        return struct.pack("<d", real)
    if kind == "int":
        _require_int(value, -(1 << 63), (1 << 63) - 1, context, row)
        return struct.pack("<q", value)
    if kind == "enum":
        if isinstance(value, str):
            try:
                value = variants.index(value)  # type: ignore[union-attr]
            except ValueError as error:
                raise ValueError(
                    f"{context} row {row} has unknown enum variant {value!r}"
                ) from error
        _require_int(value, 0, len(variants) - 1, context, row)  # type: ignore[arg-type]
        return struct.pack("<H", value)
    if kind == "ref":
        _require_int(value, 0, target_rows - 1, context, row)  # type: ignore[operator]
        return struct.pack("<I", value)
    raise AssertionError(f"unexpected column kind {kind!r}")


def _require_int(value, minimum: int, maximum: int,
                 context: str, row: int) -> None:
    if (
        isinstance(value, bool)
        or not isinstance(value, int)
        or value < minimum
        or value > maximum
    ):
        raise ValueError(
            f"{context} row {row} must be an integer in [{minimum}, {maximum}]"
        )


def _atomic_write(path: pathlib.Path, payload: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.NamedTemporaryFile(
        mode="wb", dir=path.parent,
        prefix=f".{path.name}.", suffix=".tmp", delete=False,
    )
    temp_path = pathlib.Path(temporary.name)
    try:
        with temporary as output:
            output.write(payload)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temp_path, path)
        os.chmod(path, 0o644)
    except BaseException:
        temp_path.unlink(missing_ok=True)
        raise

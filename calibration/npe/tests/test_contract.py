from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from contract import ContractError, load_pairs


def write_artifact(directory: Path) -> tuple[Path, Path]:
    pairs = directory / "pairs.csv"
    csv_bytes = b"k,beta,gamma,peak_I,peak_tick\n0,0.8,0.1,250,12\n"
    pairs.write_bytes(csv_bytes)
    metadata = {
        "draws": 1,
        "noise_mode": "independent",
        "pairs_hash_algorithm": "sha256",
        "pairs_sha256": hashlib.sha256(csv_bytes).hexdigest(),
        "parameter_columns": ["beta", "gamma"],
        "schema_versions": {"pairs": 1},
        "summary_columns": ["peak_I", "peak_tick"],
    }
    sidecar = Path(f"{pairs}.meta.json")
    sidecar.write_text(json.dumps(metadata), encoding="utf-8")
    return pairs, sidecar


def test_accepts_valid_pairs_contract(tmp_path: Path) -> None:
    pairs, sidecar = write_artifact(tmp_path)
    artifact = load_pairs(pairs, sidecar)
    assert artifact.parameter_columns == ("beta", "gamma")
    assert artifact.summary_columns == ("peak_I", "peak_tick")
    assert artifact.rows[0]["peak_I"] == 250.0


def test_refuses_crn_mode_with_reason(tmp_path: Path) -> None:
    pairs, sidecar = write_artifact(tmp_path)
    metadata = json.loads(sidecar.read_text(encoding="utf-8"))
    metadata["noise_mode"] = "crn"
    sidecar.write_text(json.dumps(metadata), encoding="utf-8")
    with pytest.raises(ContractError, match="refusing CRN-mode pairs"):
        load_pairs(pairs, sidecar)


def test_refuses_tampered_pairs_hash(tmp_path: Path) -> None:
    pairs, sidecar = write_artifact(tmp_path)
    pairs.write_bytes(pairs.read_bytes() + b"1,0.9,0.2,300,10\n")
    with pytest.raises(ContractError, match="pairs_sha256 mismatch"):
        load_pairs(pairs, sidecar)


def test_refuses_unsupported_schema_major(tmp_path: Path) -> None:
    pairs, sidecar = write_artifact(tmp_path)
    metadata = json.loads(sidecar.read_text(encoding="utf-8"))
    metadata["schema_versions"]["pairs"] = 2
    sidecar.write_text(json.dumps(metadata), encoding="utf-8")
    with pytest.raises(ContractError, match="unsupported pairs schema major 2"):
        load_pairs(pairs, sidecar)

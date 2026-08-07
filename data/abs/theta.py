"""The theta vector and its draw files.

`params/priors.json` is the sole authority for which parameters are free
(`DECISIONS.md` §N6): seventeen migration parameters are free and the other 360
are fixed ABS-derived rates. This module builds theta as exactly those seventeen
and writes draw files in the shape `sembla sweep --theta-file` already accepts,
holding every fixed parameter at its per-year value so the sweep can never
sample a rate that the data has already determined.

Draws are centred on the offline gravity fit (PRD 0008 §1) rather than on the
generic prior, so the simulator is exercised in the region the published O-D
table already indicates.

Randomness is a counter-based SHA-256 stream rather than `random`, so a draw
file depends only on its semantic coordinates and never on interpreter version
or call order.

Standard library only.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import pathlib
import struct

import canonical

HERE = pathlib.Path(__file__).resolve().parent
PARAMS = HERE / "params"
GRAVITY = PARAMS / "gravity"

DRAW_DOMAIN = b"sembla.australian-population-theta/v1\0"
DRAW_FORMAT = "sembla.australian-population-theta-draws/v1"


class ThetaError(ValueError):
    """A theta contract failed."""


def _load(path: pathlib.Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise ThetaError(f"cannot read {path}: {error}") from error


def free_parameters(params_dir: pathlib.Path = PARAMS) -> tuple[str, ...]:
    """Theta is exactly the free parameters named by the prior registry."""
    priors = _load(params_dir / "priors.json")
    names = tuple(priors["free_parameters"])
    classified = tuple(
        sorted(
            name
            for name, entry in priors["parameters"].items()
            if entry["classification"] == "free"
        )
    )
    if names != classified:
        raise ThetaError(
            "priors.json free_parameters disagrees with the parameter classifications"
        )
    return names


def prior_spreads(params_dir: pathlib.Path = PARAMS) -> dict[str, float]:
    priors = _load(params_dir / "priors.json")
    spreads = {}
    for name in free_parameters(params_dir):
        prior = priors["parameters"][name]["lean_prior_2010"]
        if prior["family"] != "log_normal":
            raise ThetaError(f"{name} is not log-normal; the draw stream assumes it is")
        spreads[name] = prior["spread"]
    return spreads


def _uniform_stream(key: bytes, count: int):
    """Counter-based uniforms in (0, 1)."""
    produced = 0
    counter = 0
    while produced < count:
        block = hashlib.sha256(key + struct.pack(">Q", counter)).digest()
        counter += 1
        for offset in (0, 8, 16, 24):
            if produced >= count:
                return
            word = struct.unpack(">Q", block[offset : offset + 8])[0]
            # Map to (0, 1): never exactly zero, so log() is always finite.
            yield (word + 0.5) / 18446744073709551616.0
            produced += 1


def _normals(key: bytes, count: int) -> list[float]:
    """Box-Muller normals from the counter stream."""
    pairs = (count + 1) // 2
    uniforms = list(_uniform_stream(key, pairs * 2))
    values: list[float] = []
    for index in range(pairs):
        first = uniforms[2 * index]
        second = uniforms[2 * index + 1]
        radius = math.sqrt(-2.0 * math.log(first))
        angle = 2.0 * math.pi * second
        values.append(radius * math.cos(angle))
        values.append(radius * math.sin(angle))
    return values[:count]


def draw_key(
    model_identity: str,
    scale: str,
    run_year: int,
    parameter_digest: str,
    draws: int,
    purpose: str,
) -> bytes:
    """Semantic coordinates only — never list position, path or wall-clock."""
    coordinate = {
        "draws": draws,
        "model_identity": model_identity,
        "parameter_digest": parameter_digest,
        "purpose": purpose,
        "run_year": run_year,
        "scale": scale,
    }
    payload = json.dumps(coordinate, sort_keys=True, separators=(",", ":"))
    return hashlib.sha256(DRAW_DOMAIN + payload.encode("utf-8")).digest()


def build_draws(
    centre: dict[str, float],
    *,
    draws: int,
    key: bytes,
    spreads: dict[str, float],
    names: tuple[str, ...],
) -> list[dict[str, float]]:
    """Log-normal draws around the fitted centre, with fixed values untouched."""
    if draws < 1:
        raise ThetaError("a theta file needs at least one draw")
    normals = _normals(key, draws * len(names))
    assignments = []
    for index in range(draws):
        assignment = dict(centre)
        for position, name in enumerate(names):
            deviate = normals[index * len(names) + position]
            assignment[name] = centre[name] * math.exp(spreads[name] * deviate)
        assignments.append(assignment)
    return assignments


def write_draws(
    path: pathlib.Path,
    run_year: int,
    *,
    draws: int,
    model_identity: str,
    scale: str,
    params_dir: pathlib.Path = GRAVITY,
    priors_dir: pathlib.Path = PARAMS,
    purpose: str = "npe-training",
) -> dict:
    """Write one theta file plus the provenance record that explains it."""
    centre_path = params_dir / f"{run_year}.json"
    centre_bytes = centre_path.read_bytes()
    centre = json.loads(centre_bytes.decode("utf-8"))
    names = free_parameters(priors_dir)
    spreads = prior_spreads(priors_dir)
    missing = [name for name in names if name not in centre]
    if missing:
        raise ThetaError(f"{centre_path} is missing free parameters: {missing}")
    digest = hashlib.sha256(centre_bytes).hexdigest()
    key = draw_key(model_identity, scale, run_year, digest, draws, purpose)
    assignments = build_draws(
        centre, draws=draws, key=key, spreads=spreads, names=names
    )
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(assignments, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
        newline="",
    )
    fixed = [name for name in sorted(centre) if name not in set(names)]
    for assignment in assignments:
        for name in fixed:
            if assignment[name] != centre[name]:
                raise ThetaError(f"draw varied fixed parameter {name}")
    return {
        "draw_key_sha256": key.hex(),
        "draws": draws,
        "fixed_parameter_count": len(fixed),
        "format": DRAW_FORMAT,
        "free_parameters": list(names),
        "model_identity": model_identity,
        "parameter_digest": digest,
        "purpose": purpose,
        "run_year": run_year,
        "scale": scale,
        "theta_file_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-year", type=int, required=True)
    parser.add_argument("--draws", type=int, required=True)
    parser.add_argument("--scale", default="hundredth")
    parser.add_argument("--model-identity", required=True)
    parser.add_argument("--params-dir", type=pathlib.Path, default=GRAVITY)
    parser.add_argument("--purpose", default="npe-training")
    parser.add_argument("--out", type=pathlib.Path, required=True)
    parser.add_argument("--record", type=pathlib.Path)
    args = parser.parse_args(argv)
    record = write_draws(
        args.out,
        args.run_year,
        draws=args.draws,
        model_identity=args.model_identity,
        scale=args.scale,
        params_dir=args.params_dir,
        purpose=args.purpose,
    )
    if args.record:
        canonical.write_json(args.record, record)
    print(
        f"wrote {record['draws']} draws over {len(record['free_parameters'])} "
        f"free parameters for {record['run_year']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

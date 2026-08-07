"""Per-year NPE orchestration for the Australian population calibration.

This is the §3 harness: it composes existing commands rather than changing any
of them. `theta.py` builds the draw files, `sembla sweep` simulates them, this
module reduces every draw to the ordered summary vector, and the quarantined
`calibration/npe` trainer produces the posterior.

Two contracts bind and are both honoured:

- `calibration/npe` is byte-unchanged and learns nothing about Australian
  geography. The training entry point below runs *under the calibration venv
  interpreter* and only then imports the quarantined trainer, injecting
  per-parameter recovery tolerances at runtime because the reference trainer
  hard-codes tolerances for its two reference parameters. Runtime injection
  leaves every quarantined byte untouched.
- `data/abs` itself stays standard-library-only: every heavy import is lazy
  and only ever executes under the venv interpreter. All pipeline checks run
  with the system Python and no third-party package.

The summary vector carries the normalized interstate age-sex composition and
the stock age structure — the evidence that can identify `peak_months` and
`k` — alongside the published O-D counts and the headline demographic totals,
exactly as PRD 0008 §3 requires.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import pathlib
import subprocess
import sys

import canonical
import theta

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parent.parent
EXTRACTS = HERE / "extracts"
PARAMS = HERE / "params"
GRAVITY = PARAMS / "gravity"
EXECUTION = HERE / "targets" / "execution.json"
NPE_DIR = ROOT / "calibration" / "npe"
NPE_PYTHON = NPE_DIR / ".venv" / "bin" / "python"
MODEL = ROOT / "fixtures/australian-population/australian_population.hundredth.json"
PLAN = (
    ROOT / "fixtures/australian-population/australian_population.hundredth.plan.json"
)
STATE_2010 = ROOT / "fixtures/state/australian_population_2010_hundredth.state"

STATES = ("nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act")
SEXES = ("female", "male")
ABS_BANDS = (
    "0-4", "5-9", "10-14", "15-19", "20-24", "25-29", "30-34", "35-39",
    "40-44", "45-49", "50-54", "55-59", "60-64", "65-69", "70-74", "75+",
)
OD_CELLS = tuple(
    (origin, destination)
    for origin in STATES
    for destination in STATES
    if origin != destination
)
SCALAR_COLUMNS = (
    "final_population",
    "births_total",
    "deaths_total",
    "interstate_moves_total",
    "overseas_arrivals_total",
    "overseas_departures_total",
)
SUMMARY_COLUMNS = tuple(
    [f"scalar_{name}" for name in SCALAR_COLUMNS]
    + [f"od_{origin}_{destination}" for origin, destination in OD_CELLS]
    + [f"mover_{sex}_{band}" for sex in SEXES for band in ABS_BANDS]
    + [f"stock_{sex}_{band}" for sex in SEXES for band in ABS_BANDS]
)
PILOT_FORMAT = "sembla.australian-population-npe-pilot/v1"


class CalibrateError(ValueError):
    """A calibration contract failed."""


def _load(path: pathlib.Path) -> dict:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise CalibrateError(f"cannot read {path}: {error}") from error


def model_identity() -> str:
    return _load(EXECUTION)["model"]["ir_hash"]["digest"]


def _model_band(grouped_band_index: int) -> int:
    """Map a grouped-view band index to the ABS five-year band index.

    Grouped CSVs emit band *indices*, not attribute values: `band age_months 60`
    produces 0, 1, 2, ... for the 0-4, 5-9, 10-14, ... year bands. Indices at or
    above the ABS 75+ band (15) are pooled into it.
    """
    return min(grouped_band_index, len(ABS_BANDS) - 1)


def _erp_band(age_years: int) -> int:
    return min(age_years // 5, len(ABS_BANDS) - 1)


def _shares(counts: list[float]) -> list[float]:
    total = sum(counts)
    if total <= 0.0:
        return [0.0] * len(counts)
    return [value / total for value in counts]


# ---------------------------------------------------------------------------
# Summary vectors
# ---------------------------------------------------------------------------


def x_from_draw(sweep_dir: pathlib.Path, draw: int) -> list[float]:
    """Reduce one simulated draw to the ordered summary vector."""
    sweep_dir = pathlib.Path(sweep_dir)
    run_path = sweep_dir / f"draw_{draw}.csv"
    try:
        lines = run_path.read_text(encoding="utf-8").splitlines()
    except OSError as error:
        raise CalibrateError(f"cannot read {run_path}: {error}") from error
    data_lines = [line for line in lines if not line.startswith("#")]
    rows = list(csv.DictReader(data_lines))
    if len(rows) != 12:
        raise CalibrateError(f"{run_path} has {len(rows)} ticks, expected 12")
    od_counts = []
    for origin, destination in OD_CELLS:
        column = f"fired_move_{origin}_{destination}"
        od_counts.append(sum(float(row[column]) for row in rows))

    summaries = {
        row["name"]: float(row["value"])
        for row in csv.DictReader(
            (sweep_dir / f"draw_{draw}.csv.summaries.csv").open(encoding="utf-8")
        )
    }
    scalars = [summaries[name] for name in SCALAR_COLUMNS]

    mover_counts = [0.0] * (len(SEXES) * len(ABS_BANDS))
    flow_path = sweep_dir / f"draw_{draw}.grouped.interstate_age_sex_flows.csv"
    for row in csv.DictReader(flow_path.open(encoding="utf-8")):
        band = _model_band(int(row["event_age_months"]))
        index = SEXES.index(row["sex"]) * len(ABS_BANDS) + band
        mover_counts[index] += float(row["count"])

    stock_counts = [0.0] * (len(SEXES) * len(ABS_BANDS))
    stock_path = sweep_dir / f"draw_{draw}.grouped.population_cells.csv"
    for row in csv.DictReader(stock_path.open(encoding="utf-8")):
        if int(row["tick"]) != 11:
            continue
        band = _model_band(int(row["age_months"]))
        index = SEXES.index(row["sex"]) * len(ABS_BANDS) + band
        stock_counts[index] += float(row["count"])

    return scalars + od_counts + _shares(mover_counts) + _shares(stock_counts)


def _extract_rows(name: str) -> list[dict[str, str]]:
    with (EXTRACTS / name).open(encoding="utf-8", newline="") as handle:
        return list(csv.DictReader(handle))


def x_real(run_year: int, scale_factor: int = 100) -> list[float]:
    """The published summary vector for one run year, scaled to the model."""
    flows = [row for row in _extract_rows("interstate_flows.csv")
             if int(row["year"]) == run_year]
    by_cell = {(row["origin"], row["destination"]): int(row["persons"])
               for row in flows}
    if set(by_cell) != set(OD_CELLS):
        raise CalibrateError(f"interstate_flows.csv lacks complete {run_year} cells")
    od_counts = [by_cell[cell] / scale_factor for cell in OD_CELLS]

    births = sum(
        int(row["births"])
        for row in _extract_rows("births_state.csv")
        if int(row["year"]) == run_year
    )
    deaths = sum(
        int(row["deaths"])
        for row in _extract_rows("deaths_state_age_sex.csv")
        if int(row["year"]) == run_year
    )
    margins = [row for row in _extract_rows("overseas_margins.csv")
               if int(row["run_year"]) == run_year]
    arrivals = sum(int(row["arrivals"]) for row in margins)
    departures = sum(int(row["departures"]) for row in margins)
    erp = [
        row
        for row in _extract_rows("erp_state_age_sex.csv")
        if int(row["year"]) == run_year + 1
    ]
    final_population = sum(int(row["persons"]) for row in erp)
    scalars = [
        final_population / scale_factor,
        births / scale_factor,
        deaths / scale_factor,
        sum(by_cell.values()) / scale_factor,
        arrivals / scale_factor,
        departures / scale_factor,
    ]

    mover_counts = [0.0] * (len(SEXES) * len(ABS_BANDS))
    for row in _extract_rows("interstate_state_age_sex.csv"):
        if int(row["year"]) != run_year:
            continue
        index = SEXES.index(row["sex"]) * len(ABS_BANDS) + ABS_BANDS.index(
            row["age_band"]
        )
        mover_counts[index] += int(row["departures"])

    stock_counts = [0.0] * (len(SEXES) * len(ABS_BANDS))
    for row in erp:
        band = _erp_band(int(row["age"]))
        index = SEXES.index(row["sex"]) * len(ABS_BANDS) + band
        stock_counts[index] += int(row["persons"])

    return scalars + od_counts + _shares(mover_counts) + _shares(stock_counts)


def pairs_name(parameter: str) -> str:
    """Adapter mapping for the pairs contract's `k` draw-index column.

    One free parameter is literally named `k`, which collides with the
    contract's reserved draw-index column; every parameter column is therefore
    prefixed uniformly rather than special-casing one name.
    """
    return f"theta_{parameter}"


def model_name(pairs_column: str) -> str:
    if not pairs_column.startswith("theta_"):
        raise CalibrateError(f"not a parameter column: {pairs_column}")
    return pairs_column[len("theta_") :]


# ---------------------------------------------------------------------------
# Pairs artifacts in the calibration/npe contract shape
# ---------------------------------------------------------------------------


def write_pairs(
    path: pathlib.Path,
    *,
    theta_assignments: list[dict[str, float]],
    vectors: list[list[float]],
    names: tuple[str, ...],
    seed: int,
    theta_file: pathlib.Path,
    identity: str,
) -> None:
    """Write a contract-shaped pairs CSV plus its metadata sidecar."""
    if len(theta_assignments) != len(vectors) or not vectors:
        raise CalibrateError("pairs need an equal nonzero number of thetas and vectors")
    for vector in vectors:
        if len(vector) != len(SUMMARY_COLUMNS):
            raise CalibrateError(
                f"summary vector has {len(vector)} entries, "
                f"expected {len(SUMMARY_COLUMNS)}"
            )
        if any(not math.isfinite(value) for value in vector):
            raise CalibrateError("summary vector contains a non-finite value")
    path = pathlib.Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    columns = [pairs_name(name) for name in names]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle, lineterminator="\n")
        writer.writerow(["k", *columns, *SUMMARY_COLUMNS])
        for index, (assignment, vector) in enumerate(
            zip(theta_assignments, vectors, strict=True)
        ):
            writer.writerow(
                [index]
                + [format(assignment[name], ".17g") for name in names]
                + [format(value, ".17g") for value in vector]
            )
    metadata = {
        "component_versions": {
            "sembla-cli": "0.3.0",
            "sembla-ir": "0.3.0",
            "sembla-runtime": "0.3.0",
        },
        "determinism_level": "A",
        "draws": len(vectors),
        "dt": 1.0,
        "ir_hash": identity,
        "ir_hash_algorithm": "sha256",
        "model": "australian_population",
        "noise_mode": "independent",
        "pairs_hash_algorithm": "sha256",
        "pairs_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "parameter_columns": columns,
        "schema_versions": {"pairs": 1},
        "seed": seed,
        "summary_columns": list(SUMMARY_COLUMNS),
        "theta_source": {
            "algorithm": "sha256",
            "kind": "file",
            "sha256": hashlib.sha256(
                pathlib.Path(theta_file).read_bytes()
            ).hexdigest(),
        },
        "ticks": 12,
    }
    canonical.write_json(pathlib.Path(f"{path}.meta.json"), metadata)


# ---------------------------------------------------------------------------
# Sweep and capacity
# ---------------------------------------------------------------------------


def sweep_seed(run_year: int, parameter_digest: str, purpose: str) -> int:
    key = theta.draw_key(
        model_identity(), "hundredth", run_year, parameter_digest, 0, purpose
    )
    return int.from_bytes(key[:8], "big") & ((1 << 63) - 1)


def run_sweep(
    theta_file: pathlib.Path,
    out_dir: pathlib.Path,
    *,
    workers: int = 8,
    purpose: str = "npe-sweep",
    run_year: int = 2010,
    sembla: pathlib.Path = ROOT / "target/release/sembla",
) -> None:
    """Simulate every draw, using one sweep process per worker.

    `sembla sweep` is the prescribed command, but its internal draw-worker pool
    requires the CUDA backend; on CPU a sweep is serial. The draw file is
    therefore sharded into one chunk per worker and the chunks run as
    concurrent sweep *processes* (one worker each), each with its own semantic
    seed, and the chunk outputs are flattened into the documented
    `draw_<global>.*` layout. Every chunk's theta file, seed and command are
    recorded in `sweep-manifest.json`.
    """
    out_dir = pathlib.Path(out_dir)
    if out_dir.exists():
        raise CalibrateError(f"sweep output directory already exists: {out_dir}")
    theta_file = pathlib.Path(theta_file)
    assignments = json.loads(theta_file.read_text(encoding="utf-8"))
    draws = len(assignments)
    processes = max(1, min(workers, draws))
    chunk_size = (draws + processes - 1) // processes
    chunks = [
        assignments[start : start + chunk_size]
        for start in range(0, draws, chunk_size)
    ]
    parameter_digest = hashlib.sha256(theta_file.read_bytes()).hexdigest()

    out_dir.mkdir(parents=True)
    commands = []
    chunk_records = []
    for chunk_index, chunk in enumerate(chunks):
        chunk_theta = out_dir / f"theta-chunk-{chunk_index}.json"
        chunk_theta.write_text(
            json.dumps(chunk, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
            newline="",
        )
        chunk_seed = sweep_seed(
            run_year, parameter_digest, f"{purpose}-chunk-{chunk_index}"
        )
        chunk_out = out_dir / f"chunk-{chunk_index}"
        command = [
            str(sembla), "sweep", str(PLAN),
            "--population", str(STATE_2010),
            "--seed", str(chunk_seed),
            "--theta-file", str(chunk_theta),
            "--ticks", "12",
            "--out", str(chunk_out),
            "--backend", "cpu",
            "--noise", "independent",
            "--draw-workers", "1",
            "--export-pairs", str(chunk_out / "pairs.csv"),
            "--enable", "grouped-observations",
        ]
        commands.append(command)
        chunk_records.append(
            {
                "chunk": chunk_index,
                "command": command,
                "draw_count": len(chunk),
                "global_draw_start": chunk_index * chunk_size,
                "seed": chunk_seed,
                "theta_file": chunk_theta.name,
                "theta_file_sha256": hashlib.sha256(
                    chunk_theta.read_bytes()
                ).hexdigest(),
            }
        )

    running = [
        subprocess.Popen(
            command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True
        )
        for command in commands
    ]
    failures = []
    for index, process in enumerate(running):
        _, stderr = process.communicate()
        if process.returncode != 0:
            failures.append(f"chunk {index}: {(stderr or '')[-1500:]}")
    if failures:
        raise CalibrateError("sweep failed: " + " | ".join(failures))

    for chunk_index, chunk in enumerate(chunks):
        chunk_out = out_dir / f"chunk-{chunk_index}"
        for local in range(len(chunk)):
            global_draw = chunk_index * chunk_size + local
            for path in chunk_out.glob(f"draw_{local}.*"):
                target = out_dir / path.name.replace(
                    f"draw_{local}.", f"draw_{global_draw}.", 1
                )
                path.rename(target)
    canonical.write_json(
        out_dir / "sweep-manifest.json",
        {
            "chunks": chunk_records,
            "draws": draws,
            "format": "sembla.australian-population-sweep/v1",
            "noise_mode": "independent",
            "processes": processes,
            "theta_file_sha256": parameter_digest,
        },
    )


def check_capacity(sweep_dir: pathlib.Path, draws: int) -> None:
    """PRD 0007 §5: a saturated draw is not calibration evidence."""
    for draw in range(draws):
        result = subprocess.run(
            [
                sys.executable,
                str(HERE / "chain.py"),
                "check-capacity",
                "--run",
                str(pathlib.Path(sweep_dir) / f"draw_{draw}.csv"),
                "--model",
                str(MODEL),
            ],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise CalibrateError(f"draw {draw} failed capacity: {result.stderr}")


# ---------------------------------------------------------------------------
# The lazy NPE entry points (executed under the calibration venv interpreter)
# ---------------------------------------------------------------------------


def _import_quarantined(module: str):
    sys.path.insert(0, str(NPE_DIR))
    try:
        return __import__(module)
    except ImportError as error:
        raise CalibrateError(
            f"cannot import the quarantined {module}: {error}; run this entry "
            f"point with {NPE_PYTHON}"
        ) from error


def train_posterior(
    pairs: pathlib.Path,
    observation: pathlib.Path,
    output: pathlib.Path,
    *,
    train_draws: int,
    sbc_draws: int,
    threads: int,
) -> dict:
    """Train via the quarantined trainer, injecting tolerances at runtime.

    The reference trainer hard-codes recovery tolerances for its two reference
    parameters and raises on any other name. Extending the mapping here — in
    memory, under the venv interpreter — leaves `calibration/npe` byte-unchanged
    and adds no Australian name to it. The tolerances are the draw standard
    deviations of each parameter; they are informational and are never used as
    a pass gate by this harness (contraction and SBC are the diagnostics).
    """
    npe_train = _import_quarantined("train")
    deviations: dict[str, float] = {}
    with pathlib.Path(pairs).open(encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    for name in theta.free_parameters():
        column = pairs_name(name)
        values = [float(row[column]) for row in rows]
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / len(values)
        deviations[column] = max(math.sqrt(variance), 1e-12)
    npe_train.MEAN_ABSOLUTE_TOLERANCES.update(deviations)
    namespace = npe_train.parser().parse_args(
        [
            "--pairs", str(pairs),
            "--observation", str(observation),
            "--output", str(output),
            "--train-draws", str(train_draws),
            "--sbc-draws", str(sbc_draws),
            "--threads", str(threads),
        ]
    )
    return npe_train.train(namespace)


def run_sbc(posterior: pathlib.Path, diagnostics: pathlib.Path, threads: int) -> dict:
    npe_sbc = _import_quarantined("sbc")
    namespace = npe_sbc.parser().parse_args(
        [
            "--model", str(posterior),
            "--diagnostics", str(diagnostics),
            "--threads", str(threads),
        ]
    )
    return npe_sbc.run_sbc_gate(namespace)


# ---------------------------------------------------------------------------
# Contraction
# ---------------------------------------------------------------------------


def contraction_report(
    pairs: pathlib.Path, posterior_samples: pathlib.Path
) -> dict[str, dict[str, float]]:
    """Prior/posterior contraction per free parameter.

    The prior here is the draw distribution the simulator was exercised over
    (log-normal draws centred on the gravity fit); a posterior whose spread
    equals that distribution was not informed by the data and is named as such.
    """
    names = theta.free_parameters()
    with pathlib.Path(pairs).open(encoding="utf-8", newline="") as handle:
        draw_rows = list(csv.DictReader(handle))
    with pathlib.Path(posterior_samples).open(encoding="utf-8", newline="") as handle:
        sample_rows = list(csv.DictReader(handle))

    def summarise(values: list[float]) -> tuple[float, float]:
        mean = sum(values) / len(values)
        variance = sum((value - mean) ** 2 for value in values) / len(values)
        return mean, math.sqrt(variance)

    report = {}
    for name in names:
        column = pairs_name(name)
        prior_mean, prior_sd = summarise(
            [float(row[column]) for row in draw_rows]
        )
        posterior_values = [float(row[column]) for row in sample_rows]
        posterior_mean, posterior_sd = summarise(posterior_values)
        ratio = posterior_sd / prior_sd if prior_sd else 1.0
        report[name] = {
            "contraction": 1.0 - ratio,
            "identified": ratio < 0.9,
            "posterior_mean": posterior_mean,
            "posterior_sd": posterior_sd,
            "prior_mean": prior_mean,
            "prior_sd": prior_sd,
            "sd_ratio": ratio,
        }
    return report


# ---------------------------------------------------------------------------
# Pilot
# ---------------------------------------------------------------------------


def run_pilot(args) -> dict:
    """One-year NPE pilot: machinery recovery plus real-data contraction."""
    run_year = args.run_year
    draws = args.draws
    held_out = 1
    total = draws + held_out
    out = pathlib.Path(args.out)
    if out.exists() and not args.force:
        raise CalibrateError(f"pilot output exists: {out} (pass --force to replace)")
    out.mkdir(parents=True, exist_ok=True)

    centre_path = GRAVITY / f"{run_year}.json"
    centre_bytes = centre_path.read_bytes()
    centre = json.loads(centre_bytes.decode("utf-8"))
    digest = hashlib.sha256(centre_bytes).hexdigest()
    names = theta.free_parameters()
    spreads = theta.prior_spreads()
    key = theta.draw_key(
        model_identity(), "hundredth", run_year, digest, total, "npe-pilot"
    )
    assignments = theta.build_draws(
        centre, draws=total, key=key, spreads=spreads, names=names
    )
    train_assignments, heldout_assignment = assignments[:draws], assignments[draws:]

    train_theta = out / "theta-train.json"
    held_theta = out / "theta-heldout.json"
    for path, subset in ((train_theta, train_assignments), (held_theta, heldout_assignment)):
        path.write_text(
            json.dumps(subset, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="utf-8",
            newline="",
        )

    seed_train = sweep_seed(run_year, digest, "npe-sweep-train")
    seed_held = sweep_seed(run_year, digest, "npe-sweep-observation")
    sweep_train = out / "sweep-train"
    sweep_held = out / "sweep-heldout"
    if not sweep_train.exists():
        run_sweep(
            train_theta,
            sweep_train,
            workers=args.workers,
            purpose="npe-sweep-train",
            run_year=run_year,
        )
    if not sweep_held.exists():
        run_sweep(
            held_theta,
            sweep_held,
            workers=1,
            purpose="npe-sweep-observation",
            run_year=run_year,
        )

    check_capacity(sweep_train, draws)
    check_capacity(sweep_held, held_out)

    identity = model_identity()
    train_pairs = out / "pairs-train.csv"
    if not train_pairs.exists():
        vectors = [x_from_draw(sweep_train, draw) for draw in range(draws)]
        write_pairs(
            train_pairs,
            theta_assignments=train_assignments,
            vectors=vectors,
            names=names,
            seed=seed_train,
            theta_file=train_theta,
            identity=identity,
        )
    held_pairs = out / "pairs-observation-sim.csv"
    if not held_pairs.exists():
        write_pairs(
            held_pairs,
            theta_assignments=heldout_assignment,
            vectors=[x_from_draw(sweep_held, 0)],
            names=names,
            seed=seed_held,
            theta_file=held_theta,
            identity=identity,
        )
    real_pairs = out / "pairs-observation-real.csv"
    if not real_pairs.exists():
        write_pairs(
            real_pairs,
            theta_assignments=[{name: centre[name] for name in names}],
            vectors=[x_real(run_year)],
            names=names,
            seed=seed_held,
            theta_file=centre_path,
            identity=identity,
        )

    sbc_draws = min(100, draws - 1)
    train_draws = draws - sbc_draws
    npe_sim = out / "npe-sim"
    npe_real = out / "npe-real"
    trainings = []
    if not (npe_sim / "diagnostics.json").exists() or not (
        npe_real / "diagnostics.json"
    ).exists():
        venv = NPE_PYTHON
        if not venv.exists():
            raise CalibrateError(f"NPE environment missing: {venv}")
        for observation, output in ((held_pairs, npe_sim), (real_pairs, npe_real)):
            command = [
                str(venv),
                str(HERE / "calibrate.py"),
                "train",
                "--pairs", str(train_pairs),
                "--observation", str(observation),
                "--output", str(output),
                "--train-draws", str(train_draws),
                "--sbc-draws", str(sbc_draws),
                "--threads", str(args.threads),
            ]
            result = subprocess.run(command, capture_output=True, text=True)
            trainings.append(result)
            if result.returncode != 0:
                raise CalibrateError(
                    f"NPE training failed for {output}: {result.stderr[-3000:]}"
                )
    sbc_results = {}
    for label, directory in (("sim", npe_sim), ("real", npe_real)):
        diagnostics_path = directory / "diagnostics.json"
        diagnostics = _load(diagnostics_path)
        if diagnostics["sbc"]["status"] == "pending":
            command = [
                str(NPE_PYTHON),
                str(HERE / "calibrate.py"),
                "sbc",
                "--model", str(directory / "posterior.pt"),
                "--diagnostics", str(diagnostics_path),
                "--threads", str(args.threads),
            ]
            result = subprocess.run(command, capture_output=True, text=True)
            if result.returncode != 0:
                raise CalibrateError(f"SBC failed for {label}: {result.stderr[-2000:]}")
            diagnostics = _load(diagnostics_path)
        sbc_results[label] = diagnostics["sbc"]

    report = {
        "contraction_real_observation": contraction_report(
            train_pairs, npe_real / "posterior-samples.csv"
        ),
        "contraction_simulated_observation": contraction_report(
            train_pairs, npe_sim / "posterior-samples.csv"
        ),
        "draws": draws,
        "format": PILOT_FORMAT,
        "held_out_recovery": _load(npe_sim / "diagnostics.json")["parameter_results"],
        "run_year": run_year,
        "sbc": sbc_results,
        "train_draws": train_draws,
        "sbc_draws": sbc_draws,
        "summary_dimension": len(SUMMARY_COLUMNS),
    }
    canonical.write_json(out / "pilot-diagnostics.json", report)
    return report


def _print_pilot_summary(report: dict) -> None:
    real = report["contraction_real_observation"]
    print(f"pilot run year {report['run_year']}: {report['draws']} draws, "
          f"x dim {report['summary_dimension']}")
    print(f"{'parameter':>17} {'prior_sd':>12} {'post_sd':>12} {'ratio':>7} {'verdict'}")
    for name in theta.free_parameters():
        row = real[name]
        verdict = "contracts" if row["identified"] else "NOT IDENTIFIED"
        print(
            f"{name:>17} {row['prior_sd']:>12.4g} {row['posterior_sd']:>12.4g} "
            f"{row['sd_ratio']:>7.3f} {verdict}"
        )
    for label in ("sim", "real"):
        sbc = report["sbc"][label]
        print(f"sbc[{label}]: pass={sbc.get('pass')} status={sbc.get('status')}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    observe = sub.add_parser("observe", help="print the real summary vector")
    observe.add_argument("--run-year", type=int, required=True)

    train_cmd = sub.add_parser("train", help="lazy NPE training (venv python)")
    train_cmd.add_argument("--pairs", type=pathlib.Path, required=True)
    train_cmd.add_argument("--observation", type=pathlib.Path, required=True)
    train_cmd.add_argument("--output", type=pathlib.Path, required=True)
    train_cmd.add_argument("--train-draws", type=int, required=True)
    train_cmd.add_argument("--sbc-draws", type=int, default=100)
    train_cmd.add_argument("--threads", type=int, default=4)

    sbc_cmd = sub.add_parser("sbc", help="lazy SBC gate (venv python)")
    sbc_cmd.add_argument("--model", type=pathlib.Path, required=True)
    sbc_cmd.add_argument("--diagnostics", type=pathlib.Path, required=True)
    sbc_cmd.add_argument("--threads", type=int, default=4)

    pilot = sub.add_parser("pilot", help="one-year NPE pilot")
    pilot.add_argument("--run-year", type=int, default=2010)
    pilot.add_argument("--draws", type=int, default=240)
    pilot.add_argument("--workers", type=int, default=8)
    pilot.add_argument("--threads", type=int, default=4)
    pilot.add_argument("--out", type=pathlib.Path, required=True)
    pilot.add_argument("--force", action="store_true")

    args = parser.parse_args(argv)
    if args.command == "observe":
        vector = x_real(args.run_year)
        print(json.dumps(dict(zip(SUMMARY_COLUMNS, vector)), indent=1)[:2000])
        return 0
    if args.command == "train":
        train_posterior(
            args.pairs,
            args.observation,
            args.output,
            train_draws=args.train_draws,
            sbc_draws=args.sbc_draws,
            threads=args.threads,
        )
        return 0
    if args.command == "sbc":
        run_sbc(args.model, args.diagnostics, args.threads)
        return 0
    if args.command == "pilot":
        report = run_pilot(args)
        _print_pilot_summary(report)
        return 0
    raise CalibrateError(f"unknown command {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())

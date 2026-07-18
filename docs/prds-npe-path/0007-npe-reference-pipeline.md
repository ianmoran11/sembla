# PRD 0007: NPE reference pipeline (external, `sbi`)

## Context

`DECISIONS.md` §G5: calibration is **amortized neural posterior estimation**,
run as an external Python workflow (the `sbi` stack) consuming only the
PRD-0006 export — never Sembla internals. This PRD builds the reference
pipeline and the milestone's acceptance example: recover known parameters of a
synthetic SIR from simulated data, reproducibly, and pass a
simulation-based-calibration (SBC) rank check (ROADMAP v0.4 exit criterion 1).
Quarantine mirrors v0.1's GPU-spike rule (`DECISIONS.md` §I5): outside the
Cargo workspace, never depended on by Rust code.

## Goal

`calibration/npe/` trains an amortized NPE on exported pairs, produces
posterior samples and a diagnostics report for a held-out observation at known
θ*, and passes documented statistical acceptance thresholds.

## Specification

- Layout: `calibration/npe/` with `README.md`, pinned dependencies
  (`requirements.txt` or `pyproject.toml` with exact versions: `sbi`, `torch`
  CPU build, `numpy`, `pandas`, `pytest`), `generate_data.sh` (invokes the
  `sembla` CLI to produce training pairs and the held-out observation),
  `train.py`, `sbc.py`, and `tests/`.
- Input contract: the pipeline reads only `pairs.csv` + `pairs.meta.json`.
  It **refuses** (clear error naming the reason): `noise_mode: "crn"`
  (`DECISIONS.md` §G5), a `pairs_sha256` mismatch, or unknown
  `schema_versions.pairs` major.
- Reference configuration (documented in the README, sized for a laptop CPU):
  the SIR example at a reduced population (≈10k persons), ≈50 ticks,
  2,000–5,000 training draws under `--noise independent`; a held-out
  observation generated at documented θ* with a distinct seed.
- Training: single-round (amortized) NPE via `sbi`'s current API with a
  normalizing-flow density estimator; fixed `torch`/`numpy` seeds and pinned
  thread counts. Bit-exact training is **not** claimed (document why);
  acceptance is statistical.
- Outputs: posterior samples (CSV) and `diagnostics.json` recording the
  input artifact hashes, seeds, per-parameter posterior mean/quantiles, the
  recovery and SBC results, and an overall `pass` boolean.
- Acceptance thresholds (stated in the README beside their rationale; changing
  them later requires a PRD note, not a silent edit):
  - **Recovery:** each true parameter lies inside its 95% marginal credible
    interval, and each posterior mean is within a documented absolute
    tolerance of θ*.
  - **SBC:** ≥100 rank statistics per parameter; per-parameter
    Kolmogorov–Smirnov test against uniform with p > 0.01.
- Honest reporting: if the environment cannot install the pinned dependencies,
  tests report *unanswered* with the reason — never a fabricated pass.

## Non-goals

Sequential NPE rounds. Embedding networks. GPU training. Posterior import
into Sembla. Any Rust change beyond what earlier PRDs shipped. Behavior
widgets (the trained flow is their future input, not this PRD).

## Acceptance criteria

1. `cargo build --workspace` and `cargo test --workspace` are untouched and
   green — no Cargo coupling to `calibration/` (asserted by grep test or
   workspace membership check).
2. From the repo root, the documented commands (`generate_data.sh` then
   `pytest`) run end-to-end and produce `diagnostics.json` with
   `pass: true` under the reference configuration.
3. Refusal tests: CRN-mode pairs, a tampered `pairs.csv` (hash mismatch), and
   an unsupported schema major each fail with the specified errors.
4. Reproducibility test: two pipeline runs from the same artifacts and seeds
   produce identical `diagnostics.json` verdicts and posterior quantiles
   within a documented tolerance (exact byte equality not required; say so).
5. `calibration/npe/README.md` documents: the full loop (sweep → export →
   train → SBC), every threshold with rationale, the determinism caveat, and
   the CRN refusal.

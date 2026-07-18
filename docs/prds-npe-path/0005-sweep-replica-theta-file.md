# PRD 0005: Sweep independent-noise mode and `--theta-file`

## Context

`DECISIONS.md` §G5 consequence 1: the sweep's CRN default — identical
simulation coordinates across draws — is ideal for policy contrasts and wrong
for NPE training data. Pairs generated under one shared noise realization
teach the estimator a deterministic θ→x map, and the learned posterior comes
out overconfident. Training draws need **independent noise per draw**, via a
replica index entering the seed derivation (`DESIGN.md` §5.3 machinery,
narrowly applied). Separately, an external-θ mode makes the runner
method-agnostic: any future sequential method supplies its proposal draws
through a file, with zero new IR surface.

## Goal

`sembla sweep` gains `--noise crn|independent` (default `crn`, byte-identical
to today) and `--theta-file <file>` (externally supplied θ draws), both
reproducible and stable under changes to the draw count.

## Specification

- `--noise crn` (default): existing behavior, regression-tested
  byte-identical on the SIR sweep fixture.
- `--noise independent`: draw `k` runs under a derived simulation seed
  `seed_k`, computed from the master seed via the PRD-0003 Philox API in the
  reserved namespace `rule_id = u32::MAX - 1`, `entity_id = 0`, `tick = k`
  (two 32-bit words → one `u64`; the implementer documents the exact
  composition in rustdoc and freezes it with a test vector). Consequences,
  tested: `seed_k` is identical whether the sweep has K=5 or K=5,000 draws,
  and can never collide with simulation (`rule_id < u32::MAX - 1`) or prior
  (`u32::MAX`) namespaces.
- θ draws are unchanged in both modes (master seed, `u32::MAX` namespace):
  draw k's θ is identical across noise modes — tested.
- `--theta-file <file.json>`: an ordered list of complete θ assignments (every
  prior-bearing parameter present; missing or unknown names are errors naming
  the parameter). Draw count = number of entries; mutually exclusive with
  `--draws`. Composes with either noise mode. `manifest.csv` marks the source
  as `file` and stdout prints the file's SHA-256.
- The PRD-0001 run-manifest gains append-only fields: `noise_mode`, per-draw
  derived seeds (in `executions`), and a `theta_source` all-or-nothing tuple
  `{kind: "prior" | "file", sha256, algorithm}`.
- CRN-across-θ (`sembla compare`) is untouched.

## Non-goals

Named experiment axes, grids, scenario sets, and the full canonical-coordinate
seed rule (`DESIGN.md` §5.3 binds when named axes arrive — v0.4-wide, not
here). Parallel draw execution. New prior families. Any change to `run`.

## Acceptance criteria

1. `cargo test --workspace` green; all prior tests still pass.
2. Regression: the existing SIR sweep fixture invocation (no new flags) is
   byte-identical to its pre-PRD output.
3. K-stability: with `--noise independent`, draw k=3's derived seed and
   per-draw results are identical for K=5 and K=50 (same master seed).
4. Independence: two draws pinned to identical θ produce **different**
   per-draw results under `independent` and identical ones under `crn`.
5. θ-stability: draw k's θ values are identical under both noise modes.
6. Round-trip: θ values exported from a prior-mode sweep's `manifest.csv`,
   fed back via `--theta-file` under `crn`, reproduce byte-identical per-draw
   results.
7. Error tests: theta-file with a missing parameter, an unknown parameter,
   and `--theta-file` combined with `--draws` each fail with the specified
   diagnostics.
8. Namespace test vector for the seed derivation is checked in and asserted.
9. `docs/examples/sir.md` prior-predictive section documents both modes and
   *why* CRN is wrong for training data (one paragraph, citing
   `DECISIONS.md` §G5).

# PRD 0007: The chained annual driver, 2010 to 2025

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N7
(monthly ticks, chained annual runs, per-year θ), §K4 (chained runs, not
checkpoints) and DESIGN §5.3 (seeds derive from a canonical semantic coordinate,
never a positional index) bind.

Everything needed for a fifteen-year run now exists in pieces: a 2010 state
artifact, a model, per-year parameter files, and targets. This PRD makes the
walk itself reproducible, and produces the **uncalibrated baseline chain** that
PRD 0008 improves on and PRD 0009 validates against.

There is no checkpoint subsystem and none is being built (standing-no #6). A
year is a run; the chain is a sequence of runs linked by hashed state artifacts.

## Goal

`scripts/run-australian-population.sh` executes 2010→2025 as fifteen chained
annual runs at a chosen scale, verifies every chain link by hash, scores each
year against its targets, and reproduces bitwise on re-execution.

## Specification

### 1. The driver — `scripts/run-australian-population.sh`

Arguments: scale (`full`, `tenth`, `hundredth`), start and end year, parameter
directory, targets directory, output directory, backend, and an optional
`--enable grouped-observations` passthrough (read the CLI first — grouped views
are behind that flag and this model uses them).

For each year *y*:

- input state is the 2010 artifact for the first year, otherwise year *y*'s
  exported state from year *y−1*;
- 12 ticks, `--params data/abs/params/<y>.json`,
  `--export-state <out>/<y+1>.state`, scalar and grouped CSV output, and a run
  manifest;
- the scorer runs against year *y*'s targets artifact.

Fail fast: a missing params file, a missing targets artifact, or a nonzero
scorer exit stops the chain rather than continuing with a gap.

### 2. Seeds derive from meaning, not position

Per DESIGN §5.3 the per-year seed must be a hash of that run's canonical
semantic coordinate — model identity, scale, year, parameter-file digest and
replica index — sorted and normalised. It must **not** be the loop index, a
counter, or `base_seed + y`. The consequence that matters: inserting a year,
re-running a subset, or extending the window leaves every other year's draws
untouched. Test this directly by re-running a middle year alone and asserting
byte-identical output.

### 3. Chain verification — `scripts/verify-population-chain.sh`

Reads the chain's manifests and asserts, for every consecutive pair, that run
*y+1*'s `initial_state` hash equals run *y*'s `exported_state` hash, that the
model identity and scale are constant across the chain, and that the parameter
digest recorded for each year matches the params file on disk. Emit a chain
report listing every link with its hashes.

Also assert the §K4 property rather than hiding it: a chained 12+12 run pair is
**not** bitwise equal to a continuous 24-tick run, because tick coordinates
restart. Test it, document it, and state why it is acceptable — each year is an
independent calibration window with its own θ.

### 4. The uncalibrated baseline chain

Run the full 2010→2025 chain at `hundredth` scale using PRD 0005's ABS-derived
parameters with no calibration, and commit the resulting evidence under
`docs/evidence/australian-population/baseline-<date>/`: per-year residual
reports, the chain report with all link hashes, the final-year population by
state, and the drift of simulated ERP against published ERP over the fifteen
years.

This baseline is the honest starting point. Expect drift to accumulate — that is
precisely what §N11 warns about and what PRD 0008 exists to reduce. Report it
plainly; a baseline that looks suspiciously good should be investigated before
it is celebrated.

### 5. Saturation and capacity checks

The runtime warns when deferred losers exceed 10% of fired transitions on a
contested resource. Every year's run must be checked for that warning and for a
vacancy margin reaching zero (`minimum_vacant_birth_slots`,
`minimum_vacant_overseas_slots`). Either condition invalidates the year as
calibration evidence (§K1) and must fail the chain loudly, not appear in a log
nobody reads.

### 6. Documentation — `docs/guides/australian-population-runs.md`

How to run a chain, the directory layout, the seed-derivation rule and why it is
not index-based, the chain-verification contract, the §K4 non-equivalence, the
saturation failure conditions, and how to read the residual reports. Link from
`docs/models/australian-population.md`.

## Allowed files

- `scripts/run-australian-population.sh`,
  `scripts/verify-population-chain.sh` (new)
- `data/abs/chain.py` (new — seed derivation and manifest reading, if a Python
  helper is cleaner than shell; standard library only)
- `crates/sembla-cli/tests/**` (chain link, seed-independence and §K4
  non-equivalence tests)
- `docs/guides/australian-population-runs.md` (new),
  `docs/models/australian-population.md` (link only)
- `docs/evidence/australian-population/**` (new)
- `.gitignore` (generated chain-output entries only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No checkpoint, restart or run-management subsystem.
- No calibration, sweep or posterior — PRD 0008.
- No model, IR, runtime or manifest-schema change.
- No new CLI flag; the driver composes existing commands only.
- No full-scale run — PRD 0010.

## Acceptance criteria

1. Full check battery passes; `git diff --check` passes.
2. The 2010→2025 chain runs end to end at `hundredth` scale and reproduces
   bitwise on re-execution.
3. Every chain link verifies by hash, and the chain report lists all fifteen
   links.
4. Re-running a single middle year in isolation produces byte-identical output,
   proving the seed derives from semantic coordinates rather than position.
5. The §K4 chained-versus-continuous non-equivalence is asserted by test and
   documented, not worked around.
6. A saturation warning or a zero vacancy margin fails the chain, proven by a
   deliberately saturating scenario.
7. The uncalibrated baseline evidence is committed with per-year residuals and
   the fifteen-year ERP drift reported honestly.

## Implementation evidence

- `scripts/run-australian-population.sh` delegates to the standard-library-only
  `data/abs/chain.py` driver. It refuses non-empty output directories, runs
  twelve ticks per annual window, exports the next boundary state, scores every
  year in evaluation mode and fails immediately on missing inputs, runtime
  failure, strict saturation or zero vacancy.
- Seeds use the domain `sembla.australian-population-seed/v1` over compact sorted
  coordinates containing model IR identity, scale, run year, exact parameter
  byte hash and replica index. The frozen 2010 replica-zero seed is
  `2594735361883248024`; year, parameter or replica changes produce a distinct
  seed, while declaration/list position is absent.
- `scripts/verify-population-chain.sh` recomputes all fifteen state-artifact and
  raw-byte links, model/plan identity, semantic seeds, exact resolved theta,
  scalar/summary/grouped hashes, target and score hashes and capacity evidence.
  Score reports are recomputed from run bytes rather than trusted. The verifier
  executes only its caller-selected or repository-default binary, never a path
  read from the report.
- Two independent complete hundredth-scale executions produced all 226 files
  byte-identically. Run year 2017 was then rerun in isolation from the same
  2017 boundary state and reproduced its fifteen annual output files exactly.
  The evidence builder independently verifies all three directories before
  writing either proof and rejects anything other than the complete 2010--2024
  hundredth-scale baseline.
- The canonical chain report has raw SHA-256
  `29fe3755a14f909c7437d01fb2e667d6525473533f4ffb917c348a4ab0075c24`.
  The terminal 2025 state has raw SHA-256
  `21f8a2013475ad2322548c314cd5a489368104eae25a64eb384ffe08db223b0f`
  and state-artifact digest
  `95241e9b4a0f961a8daf739d5cb4a98b17d0420db9f4d160e9c932240fcc9b63`.
- Minimum remaining pools were 4,606 birth and 7,425 overseas slots. The maximum
  deferred/fired ratio was 0.652174% in 2020, below the strict 10% threshold.
  A deliberately saturated real run was rejected, and exact-threshold and both
  zero-vacancy cases are covered by tests.
- The generic chained-run test continues to assert §K4: chained 12+12 execution
  is intentionally not a continuous 24-tick run.

### Baseline result

Evidence is committed under
`docs/evidence/australian-population/baseline-2026-08-06/`, including all
fifteen residual reports, chain and reproduction proofs, ERP drift, component
errors and final state totals. The eight-state national total ends 174,492 high
at 27,780,500 versus 27,606,008 (**+0.632%**), with maximum national relative
drift +0.673% at the 2024 boundary. This national agreement conceals the
uncalibrated spatial failure: terminal errors include NT +99.515%, ACT +40.291%,
Tasmania +37.338%, SA +7.030%, Queensland -6.579% and Victoria -3.678%; annual
O-D WAPE remains approximately 70--75%. These are baseline failures for PRD
0008 to improve, not calibration results.

A second evidence build from the complete chain outputs was byte-identical.
Raw run/grouped/state files are omitted from Git; their complete hash inventory
is retained in `reproduction.json`.

### Validation

- `bash scripts/check.sh` passed the full documentation, Rust, Lean
  proof-hygiene, negative-elaboration, export/parity, lock and dependency
  checks. The Australian suite passed 16 tests with the intentional full/tenth
  regeneration test ignored.
- `bash scripts/check-abs-data.sh` passed 140 tests, offline cache verification,
  deterministic extract/rate/target/parameter/report regeneration and
  byte-identical hundredth state regeneration.
- Cargo formatting and Clippy with `-D warnings`, shell syntax, Python compile,
  Markdown links and `git diff --check` passed.
- The baseline evidence package regenerated as 23 byte-identical committed
  files from the two complete chains and isolated 2017 chain.
- Final independent review: **PASS**, with no blocker, high or medium findings.
  No files were staged.

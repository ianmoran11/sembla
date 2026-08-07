# Running the Australian population chain

The Australian model advances through fifteen independent annual runs. Each run
contains twelve monthly ticks and exports a hashed `sembla.state/v1` artifact
for the next year. This is artifact chaining, not checkpoint/restart.

## Build and run

The committed scientific target and initial-state artifacts are currently at
`hundredth` scale:

```bash
cargo build --release --locked

scripts/run-australian-population.sh \
  --scale hundredth \
  --start-year 2010 \
  --end-year 2024 \
  --params-dir data/abs/params \
  --targets-dir data/abs/targets \
  --out /tmp/australian-population-baseline \
  --backend cpu \
  --enable grouped-observations

scripts/verify-population-chain.sh \
  --out /tmp/australian-population-baseline
```

The committed reproducibility evidence requires a second complete execution and
an isolated **interior** year, not the first or last year:

```bash
scripts/run-australian-population.sh \
  --scale hundredth --start-year 2010 --end-year 2024 \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-population-repeat --backend cpu \
  --enable grouped-observations

scripts/run-australian-population.sh \
  --scale hundredth --start-year 2017 --end-year 2017 \
  --initial-state /tmp/australian-population-baseline/2017.state \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-population-2017 --backend cpu \
  --enable grouped-observations

python3 data/abs/chain.py evidence \
  --chain /tmp/australian-population-baseline \
  --reproduction-chain /tmp/australian-population-repeat \
  --middle-chain /tmp/australian-population-2017 \
  --middle-run-year 2017 \
  --out docs/evidence/australian-population/baseline-2026-08-06
```

The evidence command independently verifies all three chains, recomputes the
full-chain and middle-year byte inventories, and refuses any chain that is not
the complete hundredth-scale 2010–2024 baseline.

`--end-year` is the inclusive **run year**, so `2024` produces the terminal
30 June 2025 state. The driver refuses a non-empty output directory and does
not resume partial work. To run a later annual window independently, provide
its already-verified boundary state explicitly:

```bash
scripts/run-australian-population.sh \
  --scale hundredth --start-year 2017 --end-year 2017 \
  --initial-state /tmp/australian-population-baseline/2017.state \
  --params-dir data/abs/params --targets-dir data/abs/targets \
  --out /tmp/australian-population-2017 \
  --backend cpu --enable grouped-observations
```

That is a new annual run, not restart of an interrupted process.

## Output layout

For each run year `y`, the output directory contains:

- `y.csv`, its run manifest, summaries and ten grouped CSVs;
- `y.score.json`, an evaluation-mode `sembla.target-score/v1` report containing
  fitted and held-out targets without conflating their roles; and
- `y+1.state`, the exported boundary state.

`chain-report.json` records all annual input/output state tuples, raw byte
hashes, model and plan identity, semantic seed, exact parameter and target
bytes, scalar/summary/grouped hashes, score hash and capacity diagnostics. It
contains fifteen links for a complete 2010–2025 walk.

Generated chain directories are intentionally not committed. The compact
baseline evidence retains the chain report, all residual reports, deterministic
reproduction proofs, ERP drift and final state totals.

## Semantic annual seeds

A run seed is the first big-endian 64 bits of:

```text
SHA-256(
  b"sembla.australian-population-seed/v1\0" +
  compact_sorted_json({
    format,
    model_identity,
    scale,
    run_year,
    params_raw_sha256,
    replica_index
  })
)
```

The complete digest is recorded alongside the integer seed. Loop position,
attempt number, timestamp, host path and output directory never enter this
coordinate. Inserting, removing or reordering years therefore cannot change an
unrelated year's random draws. A middle year rerun with the same boundary state
reproduces every annual output byte for byte.

Inspect one seed directly with:

```bash
python3 data/abs/chain.py seed \
  --model-identity 3d249dbc8c39249b234daa6d3af7a44bae77acf154a830b1b81180afbcd0f62e \
  --scale hundredth --run-year 2010 \
  --params data/abs/params/2010.json --replica-index 0
```

## Chain verification

The verifier recomputes the report from the chain bytes and rejects any change
to:

- a consecutive state-artifact link;
- raw initial or exported state bytes;
- model IR, plan-semantic identity, feature set, backend, tick count or seed;
- annual parameter bytes or the manifest's resolved theta;
- target bytes, results, summaries, grouped outputs or score reports; or
- capacity and saturation diagnostics.

It does not silently repair a gap. A missing year, target, parameter file or
nonzero scorer result fails the original run before the next year begins. The
verifier executes only the caller-selected `--sembla` binary or the repository's
`target/release/sembla`; a path recorded inside an untrusted report is never
executed.

## Chained versus continuous execution

Two chained twelve-tick windows are deliberately **not** bitwise equal to one
continuous 24-tick run. Tick coordinates restart at zero in the second annual
window, and every run year has its own semantic seed and parameter file. Annual
windows are independent calibration units linked by explicit state artifacts;
they are not hidden continuation state.

This requirement is asserted in
`crates/sembla-cli/tests/chained_runs.rs` and must not be “fixed” by inventing
checkpoint semantics.

## Saturation and vacancies

Every contested Australian transition uses the single `slot_resource` table.
For each tick the driver sums fired contested transitions and applies the
runtime's exact strict threshold:

```text
10 × deferred > fired
```

Exactly 10% is permitted; strictly more is saturation and fails the chain. The
driver also fails if either `vacant_birth_slots` or
`vacant_overseas_slots` reaches zero. It checks both captured runtime warnings
and the scalar evidence directly because these conditions invalidate a run for
calibration.

Use the guard independently with:

```bash
python3 data/abs/chain.py check-capacity \
  --run /tmp/australian-population-baseline/2010.csv \
  --model fixtures/australian-population/australian_population.hundredth.json
```

## Reading residuals

Each annual score preserves signed and absolute cell errors, MAE, RMSE and
maximum error by family, role, state and population-size bin. Fitted controls
remain reconstruction evidence; single-year stocks remain structural held-outs.
The baseline report also publishes state and national ERP drift and annual WAPE
for births, deaths, overseas flows and all 56 interstate O-D cells.

Published registration-year births/deaths, financial-year migration and 30 June
ERP do not form an exact accounting identity. Other Territories are excluded.
The chain reports these discrepancies and never forces the series to reconcile.

# PRD 0010: Full-scale run and benchmark evidence

## Context

Read `docs/prds-australian-population/README.md` first. `DECISIONS.md` §N8
(calibrate scaled, validate full), §M (performance methodology) and §K2 (the
`Expr::Tick` deferral and its measurement trigger) bind.

Everything so far runs at `hundredth` scale. §N8's second half is unmet until a
full-scale chain runs and the scaled conclusions are confirmed. This model also
changes the performance picture materially: 418 transitions after retaining
336 state-specific mortality cells, against `demographic_slots`' 12, so the
per-transition-per-row guard cost dominates
differently and §K2's ageing-write trigger needs re-measuring against a
realistic model rather than the aggregate one.

This folder's convention for hardware-dependent work applies: **local criteria**
must pass on a no-GPU, moderate-memory machine; **hardware criteria** are
scripted, documented and listed as pending — never fabricated.

## Goal

A full-scale (~35–40M slot) 2010 state builds and runs, memory and throughput
are measured across scales and backends, the §K2 trigger determination is
recorded against this model, and the scaled-calibration conclusions from PRD 0008
are confirmed or qualified at 1:1.

## Specification

### 1. Full-scale state and its cost

Build the `full` artifact with PRD 0003's builder and record: build wall time,
artifact size on disk, load time, and resident memory after load. Confirm the
present-slot cells still match published 2010 ERP exactly at this scale — the
exactness claim in PRD 0003 was proven at `full` scale by construction, and this
is the run-time confirmation of it.

The design estimate is ~64–80 B/slot double-buffered, giving ~5–6.5 GB steady
state. Report the **measured** figure against that estimate and say plainly
whether the estimate held.

### 2. Benchmark — `scripts/bench-australian-population.sh`

Mirror `scripts/bench-demographic.sh`'s structure and §M's methodology (read
both first; do not invent a second measurement convention). Measure across
`hundredth`, `tenth` and `full`:

- tick throughput and total run wall time for a 12-tick year;
- resident memory high-water mark;
- state artifact read and `--export-state` write cost;
- **cost share by transition group** — ageing, event clearing, the 56 moves, the
  336 state-specific mortality rules, entries and exits — since movement and
  disaggregated mortality are this model's distinguishing costs, and their
  shares decide whether finer geography is ever affordable.

### 3. The §K2 trigger determination

§K2 deferred `Expr::Tick`/derived age behind a measurement trigger: the ageing
write must be shown material before that construct is designed.
`demographic_slots` answered that question for a 12-transition model. Answer it
again here, where ageing is one transition among 418, and record the
determination either way. If the share is now immaterial, say so — that
strengthens the deferral rather than weakening it.

### 4. Backend differential

Run `sembla diff-backends` on this model at a tractable scale with
`--enable grouped-observations`, confirming CPU and CUDA agree including grouped
outputs. Record the determinism level, GPU model and driver in the evidence per
§5.2's manifest requirements. If CUDA is unavailable locally, script it and list
it as a pending hardware criterion with the exact command.

### 5. Full-scale confirmation of the scaled conclusions

Run at least the first and last calibration years at `full` scale with PRD
0008's fitted θ, and compare held-out error against the `hundredth`-scale
results. This is the direct test of §N8's assumption that per-capita hazards are
scale-invariant in practice.

Pay particular attention to NT and ACT, where `hundredth`-scale cells are
frequently 0 or 1 (PRD 0003 §1). If full scale materially changes conclusions
for the small jurisdictions, that qualifies §N8 and must be written back into
both PRD 0008's and PRD 0009's documentation.

A full fifteen-year chain at `full` scale is desirable but is a hardware
criterion, not a local one; script it and report it as pending if the machine
cannot carry it.

### 6. Evidence and documentation

Commit evidence under `docs/evidence/australian-population/scale-<date>/` with
the repository commit, binary hash, hardware description, and every measurement
in §M's format. Local and pending hardware criteria are listed separately and
unambiguously.

Extend `docs/models/australian-population.md` with a performance section: the
measured per-slot cost, the transition-group cost shares, the geography ceiling
this implies for any future folder, and the §K2 determination.

## Allowed files

- `scripts/bench-australian-population.sh` (new)
- `docs/evidence/australian-population/**` (new)
- `docs/models/australian-population.md` (performance section),
  `docs/guides/australian-population-runs.md` (link only)
- `docs/design/australian-population-model.md` (scale-arithmetic outcome only)
- `data/abs/**` (only if the builder needs a documented fix to reach full scale;
  any change relisted with its reason)
- implementation notes/artifacts created by the managed run

## Non-goals

- No `Expr::Tick` implementation regardless of the determination — this PRD
  measures, it does not build.
- No model, IR, runtime, CUDA kernel or CLI change.
- No recalibration; θ comes from PRD 0008 unchanged.
- No fabricated hardware results, and no GPU claim without a recorded device,
  driver and determinism level.
- No new dependency in any language.

## Acceptance criteria

1. Full check battery passes; `git diff --check` passes.
2. The `full` 2010 artifact builds, loads and runs; measured per-slot memory is
   reported against the ~64–80 B estimate with an explicit verdict.
3. Benchmarks cover all three scales with throughput, memory, artifact I/O and
   per-transition-group cost shares, following §M's methodology.
4. The §K2 trigger determination is recorded for this model, either way, with
   the measurement supporting it.
5. Backend differential passes including grouped outputs, or is scripted and
   listed as a pending hardware criterion with its exact command.
6. First and last calibration years run at `full` scale and are compared with
   the `hundredth`-scale held-out results; any small-jurisdiction divergence is
   written back into PRDs 0008 and 0009.
7. Evidence records commit, binary hash and hardware; local and pending criteria
   are separated and nothing hardware-dependent is claimed as passed.

# PRD 0005: Re-measure against the frozen case and publish the outcome

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L4 froze the benchmark case, the replicate count, and the
gate: CUDA must be **at least 3× faster than the same host's CPU** on the same
model, binary, commit, seed, and state artifact.

This PRD is the only one in the folder that requires rented hardware, and the
only one that can conclude the track — in either direction. A result below the
gate is a legitimate outcome that closes the track with a recorded reason; it is
not a signal to tune until the number passes.

## Goal

The frozen case is re-run on one host after PRD 0002, evidence is committed, and
every document carrying the old conclusion is corrected.

## Specification

### 1. Run the frozen case exactly

Per §L: 10,000,000 slots, 24 ticks, seed 9009, the no-grouped model, **three
replicates per backend, median reported**. Both arms in **one session, on one
host, at one commit** — the near-miss on 2026-07-25, where the two arms recorded
different commits, is precisely what this clause excludes.

Use `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`; see its
`RUNBOOK.md` preflight. Expected cost is roughly one hour of one GPU.

### 2. Commit evidence

A new directory under `docs/evidence/demographic-bench/` with
`bench-results.json`, `bench-results.md`, `SHA256SUMS`, GPU/CPU/RAM provenance,
the repository commit, and a `README.md` stating the median figures, the
replicate spread, and the §L4 verdict.

### 3. Record the verdict

Append the outcome to `DECISIONS.md` §L as a dated note: the measured ratio,
whether §L4 is met, and — if met — that CUDA is viable for models with contests
and `Ref` dereferences. If not met, record the ratio achieved and what remains
serial, and close the track.

### 4. Correct every document carrying the old conclusion

- `docs/demographic-benchmark.md` — supersede the 12.3× row with the new
  measurement; keep the old figure visible as the before value.
- `docs/forward-roadmap.md` — the Stage 1 scale-note amendment and the Preflight
  prerequisite both assume the defect stands. Update both to the measured
  outcome.
- `spikes/precision/infra-hyperstack/RUNBOOK.md` — if anything about the run
  procedure changed.

### 5. Report the ageing share

The frozen case reports the ageing cost share as a by-product. Two unreplicated
readings (11.6% M2 Pro, 12.2% EPYC) sit above the §K2 10% threshold. With three
replicates this becomes the first properly replicated reading — record it, and
note whether it strengthens or weakens the §K2 trigger. **Do not decide §K2
here**; that trigger has its own process.

## Allowed files

- `docs/evidence/demographic-bench/**` — new directory only
- `DECISIONS.md` (§L dated note only)
- `docs/demographic-benchmark.md`
- `docs/forward-roadmap.md`
- `spikes/precision/infra-hyperstack/RUNBOOK.md`
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No code changes. No tuning to reach the gate — if the measurement misses, that
is the finding. No 50M row; §L4 is decided at 10M. No grouped-observation work.

## Acceptance criteria

1. Evidence directory contains three replicates per backend with medians and
   spread, one host, one commit, verified `SHA256SUMS`.
2. The recorded commit is identical in both arms — asserted, not assumed.
3. `DECISIONS.md` §L carries the dated verdict with the measured ratio.
4. `docs/demographic-benchmark.md` and `docs/forward-roadmap.md` no longer
   assert the superseded conclusion anywhere; the old figure survives as a
   labelled before value.
5. The ageing share is reported with its spread and explicitly not decided.
6. `python3 scripts/check-markdown-links.py` passes; no `crates/**` diff.

## Revision note

Written before PRD 0002 lands. If 0002 changes the shape of the fix, the run
procedure here may need explicit revision — but §L4's gate and protocol are
frozen and must not be revised to fit a result.

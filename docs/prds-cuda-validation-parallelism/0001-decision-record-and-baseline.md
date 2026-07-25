# PRD 0001: Record the CUDA validation-parallelism decisions and freeze the baseline

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. This PRD is documentation-only.

Measurement on 2026-07-25 established that the CUDA backend is **12.3× slower
than the CPU on the same host** for the `demographic_slots` no-grouped model at
10M slots, and identified the cause: four generated kernels validate every row
on a single GPU thread, per claim and per fallible expression, every tick. The
cost is conditional on contests and `Ref` dereferences, which is why SIR is
unaffected and why every construct the forward roadmap plans next is affected.

Per project rule, the finding and the design choices become normative in
`DECISIONS.md` before implementation, and the affected planning documents are
corrected in the same commit that records them.

## Goal

`DECISIONS.md` gains section L covering this track's decisions;
`docs/demographic-benchmark.md` records the CUDA/CPU comparison as measured
evidence; the benchmark case and numeric gate are frozen so later PRDs cannot
move the target. No code changes.

## Specification

### 1. Add section L to `DECISIONS.md`

Append after §K, in the house style (decision, alternatives rejected, reason):

- **L1. The defect is validation, not execution.** The generated CUDA
  simulation kernels are parallel; four validation kernels
  (`sembla_validate_claims`, `sembla_validate_transition`,
  `sembla_validate_effects`, `sembla_validate_outputs`) execute a per-row loop
  on a single thread. Their cost is O(rows) serial per claim and per fallible
  expression per tick. *Alternatives rejected:* attributing the slowdown to
  host/device transfer, to `f64` arithmetic, or to the model's rule count —
  each contradicted by the measurement that SIR at 26M rows on the same GPU
  class runs at ~1,380 ticks/sec while generating none of these loops.
  *Reason:* the emitted source and the sustained 100% `utilization.gpu` reading
  (which reports kernel residency, not occupancy) jointly identify a serial
  kernel, not a stall.

- **L2. Validation remains a separate pass.** Per-row validation is
  parallelised in place; it is **not** fused into the execution kernels that
  already visit each row. *Alternatives rejected:* fusion, which is faster in
  principle. *Reason:* fusion entangles two independent concerns for a speedup
  not required to clear the §L4 gate, and the CPU oracle keeps them separate —
  divergence in structure makes differential reasoning harder for no gain now.

- **L3. Failure reporting is order-independent by construction.** Validation
  reports the **minimum failing candidate index**, computed by parallel
  reduction, not the first writer. *Alternatives rejected:* reporting any
  failing candidate, or making the reported index depend on launch
  configuration. *Reason:* diagnostics are part of the observable contract
  compared by the differential harness; a diagnostic that varies with block
  count breaks Level A determinism as surely as a differing state hash.

- **L4. The gate is "worth using", not a throughput target.** The track
  succeeds when CUDA at the frozen case is at least **3× faster than the same
  host's CPU** on the same model, binary, commit, seed, and state artifact.
  *Alternatives rejected:* a ticks/sec target, or parity with SIR throughput.
  *Reason:* the decision the roadmap needs is whether the GPU is worth using
  for this model class; an absolute target invites tuning beyond the question
  being asked.

- **L5. Grouped observations stay CPU-only.** Unchanged from §K6/§K9. This
  track admits the demographic model to the differential corpus in its
  no-grouped configuration only. *Alternatives rejected:* opportunistically
  adding CUDA grouped support here. *Reason:* it is a separate deferred
  construct with its own follow-up folder; bundling it would hide a semantic
  change inside a performance fix.

### 2. Freeze the benchmark case

Record in §L, as the case later PRDs must use unchanged:

```text
model:    fixtures/demographic/benchmark/demographic_slots.no-grouped.json
scale:    10,000,000 slots
ticks:    24
seed:     9009
areas:    4      present fraction: 0.8
streams:  birth:600,overseas:250,internal:150
command:  scripts/bench-demographic.sh --scales 10000000 --ticks 24 --seed 9009
```

Both arms run on one host in one session. Replicates: **three per backend**, and
the reported figure is the median; a single run is not evidence for a gate (the
ageing-share readings this year show why).

### 3. Record the measurement in `docs/demographic-benchmark.md`

Add a section reporting the CUDA/CPU comparison, the host, the toolchain, the
evidence directories, and the §L1 cause. State plainly that the current CUDA
path is not viable for models with contests or `Ref` dereferences, and that the
existing SIR throughput figures do not generalise to them.

### 4. Correct the forward roadmap's scale note

`docs/forward-roadmap.md` (once adopted) assumes CUDA is the answer for
national-scale sweeps and that grouped observations are the blocker. Record that
this is superseded: the blocker is §L1, and the CUDA path is unusable for the
driver model until this track lands or is closed.

## Allowed files

- `DECISIONS.md`
- `docs/demographic-benchmark.md`
- `docs/forward-roadmap.md` (if present in-repo at the time)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No changes to `crates/**`, no kernel edits, no benchmark runs, no new fixtures.
This PRD only records what is already measured and decides how the next PRDs
will be judged.

## Acceptance criteria

1. `DECISIONS.md` contains §L1–§L5 in house style, each with decision,
   alternatives rejected, and reason.
2. The frozen benchmark case appears in §L verbatim, including the replicate
   count and the median rule.
3. `docs/demographic-benchmark.md` reports the 12.3× measurement, names both
   evidence directories, and states the §L1 cause.
4. The forward roadmap's CUDA assumption is marked superseded with a pointer to
   §L1.
5. `python3 scripts/check-markdown-links.py` passes.
6. No file outside the allowed list is modified; `git diff --stat` shows
   documentation only.

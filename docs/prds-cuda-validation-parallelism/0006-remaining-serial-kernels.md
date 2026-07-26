# PRD 0006: Address the three kernels the profile actually identifies

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L1–L6 are normative, and **§L6 supersedes §L1**.

**This PRD was rewritten on 2026-07-26 after profiling.** Its first draft
targeted three kernels chosen by reading emitted source — the same method that
produced PRD 0002's incorrect scope. An `nsys` profile then showed that method
had again picked the wrong set. The measured distribution (500k rows, 2 ticks,
99.9% of GPU time):

| Kernel | Share | Instances | Character |
|---|---:|---:|---|
| `sembla_check_candidate_errors` | 37.7% | 20 | single-threaded; walks candidates (rules × rows) |
| `sembla_prepare_effects` | 33.7% | 2 | single-threaded; two per-row loops |
| `sembla_resolve_conflicts` | 28.5% | 2 | **already parallel** — slow for a different reason |

PRD 0002's four kernels now cost **0.0%**. They were serial, they are fixed, and
they never mattered.

## Goal

The three measured consumers are addressed on their own terms, and the §L4 gate
is re-measured. Two are serialisation; one is not, and must not be treated as if
it were.

## Specification

### 1. Profile before scoping, at the scale being fixed

Cost is superlinear in rows: 2.7 s/tick at 500k versus 235.3 s/tick at 10M — 87×
for 20× the rows. **The 500k distribution above may not hold at 10M.** Before
changing any kernel, capture an `nsys` kernel summary at a scale where the
superlinearity is visible (≥2M rows), and record it as evidence. If the ranking
differs from the table above, this specification is revised to match the profile,
not the other way round.

### 2. `sembla_check_candidate_errors` — the largest consumer

Single-threaded, and it walks the **candidate** array (rules × rows), which is
why detectors looking for `row < row_counts[...]` missed it — including mine.

Parallelise it with a grid-stride loop over candidates. It writes status, so it
must reuse PRD 0002's established reduction (`sembla_record_validation_failure` /
`sembla_commit_validation_status`, minimum failing index, no intra-launch early
exit that could suppress a lower index). Do not introduce a second mechanism.

Twenty launches in two ticks suggests it runs per rule per tick; confirm from the
profile whether reducing launch count is also available, but do not restructure
the tick loop in this PRD.

### 3. `sembla_prepare_effects`

Single-threaded with two per-row loops. Grid-stride both; same reduction protocol
as above.

### 4. `sembla_resolve_conflicts` — analyse, do not parallelise

It is **already parallel** and still costs 28.5%, ~483ms per instance at 500k.
Widening the launch configuration is not the fix and may not be a fix at all.

This PRD must determine *why* before proposing a change: atomic contention on the
contested resource, uncoalesced access across the claim arrays, an O(rows × rules)
argmin, or work that is simply irreducible. Use `ncu` (present on the CUDA image,
under `/usr/local/cuda*/bin`) for occupancy, memory throughput, and stall reasons.

**Record the finding and stop.** If it needs an algorithmic change, that is its
own PRD with its own decision record. Determinism here is load-bearing: §E3's
argmin with lexicographic tie-break is the conflict semantics, and CPU is ground
truth.

### 5. Guard against the mistake that produced two wrong scopes

Add a test asserting that no emitted kernel both carries the single-thread guard
and loops over a bulk array — **candidates as well as rows**, since the candidate
case is exactly what slipped through twice. Every allowed exception is named in
the test with a reason, so an exclusion becomes a visible decision rather than an
assertion in prose.

### 6. Re-measure

Re-run the frozen §L protocol unchanged and record the verdict, pass or fail.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**`
- `DECISIONS.md` (§L additions only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No semantic change. No fusion (§L2). No hoisting out of the per-tick path. No
grouped-observation support (§L5). No restructuring of the tick loop or launch
counts. No algorithmic rewrite of `resolve_conflicts` — §4 diagnoses, a later PRD
decides.

## Acceptance criteria

**Local:**

1. A profile at ≥2M rows is captured and committed as evidence **before** any
   kernel change, and the specification is reconciled against it.
2. `check_candidate_errors` and `prepare_effects` carry no single-thread guard
   and use grid-stride loops over their bulk arrays.
3. Both report failures through the PRD 0002 reduction; no second mechanism.
4. The §5 guard test exists, covers candidate-indexed loops as well as
   row-indexed ones, and enumerates every exception with a reason.
5. §4 produces a written finding for `resolve_conflicts` with `ncu` evidence.
6. Diagnostics unchanged: `device_status()` codes and messages identical; PRD
   0003's diagnostic-equality fixtures still pass.
7. `examples/**`, goldens, and tracked CUDA differential evidence byte-unchanged;
   `cargo test --locked` and `scripts/check-rust.sh` green.

**Hardware (pending per §J14.2):**

8. Frozen §L protocol re-run; §L4 verdict recorded as measured.

## Note on method

Two PRDs in this folder have now been scoped from emitted-source structure and
both picked the wrong kernels. The profile took fifteen minutes and about one
dollar. Criterion 1 is not bureaucracy — it is the correction for the specific
failure that produced this document twice.

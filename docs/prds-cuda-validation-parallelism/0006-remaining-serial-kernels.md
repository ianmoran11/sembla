# PRD 0006: Eliminate the remaining single-threaded per-row work

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L1–L6 are normative.

PRD 0002 parallelised four validation kernels and delivered better than an 11×
improvement (90m08s → under 10m for 24 ticks at 10M slots). The §L4 gate — CUDA
at least 3× faster than the same host's CPU — was still missed, because CUDA
remains roughly at CPU parity rather than ahead of it.

The cause is **not** architectural. PRD 0002's specification listed
`sembla_mark_effect_aggregates`, `sembla_prepare_effects`, and
`sembla_validate_claim_compatibility` as kernels to leave untouched, asserting
they "do O(1) or O(rules) work". That assertion was made from kernel names
without reading their bodies, and it was wrong:

| Kernel | Serial per-row loops | Writes status |
|---|---:|---|
| `sembla_mark_effect_aggregates` | 1 | no |
| `sembla_prepare_effects` | 2 | yes |
| `sembla_validate_claim_compatibility` | nested (`left_row` × `right_row`) | yes |

The same defect, in kernels excluded by an unverified claim.

## Goal

No single-threaded per-row work remains in the emitted CUDA for any corpus
model, and the codebase can no longer regress into it silently.

## Specification

### 1. Parallelise `sembla_mark_effect_aggregates`

Grid-stride its row loop. It writes no status, so no reduction is needed — this
is the simple case.

### 2. Parallelise `sembla_prepare_effects`

Grid-stride both row loops. It writes status, so it must reuse the reduction
protocol PRD 0002 established (`sembla_record_validation_failure` /
`sembla_commit_validation_status`, minimum failing candidate, no intra-launch
early exit that could suppress a lower index). Do not invent a second mechanism.

### 3. Decide `sembla_validate_claim_compatibility` — do not parallelise blindly

Its loops are **nested over rows**, i.e. O(rows²). At 10M rows that is ~10¹⁴
iterations, so it cannot currently be executing for the frozen model; the Rust
emits pair checks only for *statically incompatible* claim pairs, and the
demographic model evidently has none.

Parallelising a quadratic does not fix a quadratic. This PRD must therefore:

- **Establish empirically** whether the frozen model emits any pair loop, by
  asserting on the emitted CUDA source rather than by reasoning.
- If it emits none: record that, leave the kernel serial, and add a **named
  trigger** — the first corpus or benchmark model that emits a pair loop
  re-opens this question before its results are used for any gate.
- If it emits some: stop and record the finding. An O(rows²) check is a
  scalability defect in its own right and needs an algorithmic decision
  (indexing, sorting, or a different formulation), not a wider launch config.
  That decision belongs in its own PRD, not here.

Also record whether the CPU oracle performs the same quadratic check. If CPU
completes 10M in ~7 minutes while CUDA does not, either CPU avoids this work or
formulates it differently — and that asymmetry is worth knowing.

### 4. A mechanical guard against this class of mistake

Add a test asserting that, for every model in the differential corpus, the
emitted CUDA contains **no kernel that both carries the single-thread guard and
loops over rows**. Allowed exceptions must be named explicitly in the test with
a reason, so excluding a kernel becomes a visible decision rather than an
unverified assertion in prose.

This is the criterion that would have caught PRD 0002's scoping error
automatically, and it is the most durable part of this PRD.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**`
- `DECISIONS.md` (§L addition only, for the §3 outcome and any trigger)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No semantic change. No fusion into execution kernels (§L2). No hoisting out of
the per-tick path. No grouped-observation support (§L5). No algorithmic rewrite
of the quadratic check — that is a separate PRD if §3 shows it is reached.

## Acceptance criteria

**Local:**

1. `sembla_mark_effect_aggregates` and `sembla_prepare_effects` carry no
   single-thread guard and use grid-stride loops.
2. `sembla_prepare_effects` reports failures through the PRD 0002 reduction; no
   second reporting mechanism is introduced.
3. The corpus-wide guard test from §4 exists, passes, and enumerates every
   allowed exception with a stated reason.
4. A test records whether the frozen model emits a claim-compatibility pair
   loop, and asserts the §3 outcome either way.
5. `examples/**`, all goldens, and tracked CUDA differential evidence are
   byte-unchanged; `cargo test --locked` and `scripts/check-rust.sh` green.
6. Diagnostics unchanged: `device_status()` codes and messages identical, and
   PRD 0003's diagnostic-equality fixtures still pass.

**Hardware (pending per §J14.2):**

7. The frozen §L benchmark case re-runs under the unchanged §L4 protocol —
   three replicates per backend, one host, one commit, median reported.
8. The §L4 verdict is recorded as measured, pass or fail.

## Note on scope discipline

PRD 0002 failed its gate because a kernel exclusion was asserted rather than
checked. This PRD must not repeat that: every kernel left serial is named in a
test with a reason, not in prose. If §3 concludes the quadratic check is
unreached, that conclusion is an assertion in code with a trigger attached —
not a sentence in a document.

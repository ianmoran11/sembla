# PRD 0007: Parallelise the two remaining linear serial kernels

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L1–L6 are normative.

The 2026-07-26 profile (`docs/evidence/cuda-profile-20260726/`) shows two
single-threaded kernels that scale **linearly** in rows:

| Kernel | 500k | 2M | 5M | scaling |
|---|---:|---:|---:|---|
| `sembla_check_candidate_errors` | 37.7% | 22.7% | 11.7% | 4.0x for 4x rows |
| `sembla_prepare_effects` | 33.7% | 20.6% | 10.3% | 4.1x for 4x rows |

Their share **falls** with scale because `resolve_conflicts` is quadratic and
swamps them. They are real serial work worth removing, but they are secondary:
PRD 0006 addresses the quadratic first, because that determines whether CUDA is
viable at all.

`check_candidate_errors` walks the **candidate** array (rules x rows), not rows.
Detectors looking for `row < row_counts[...]` miss it — mine did, twice.

## Goal

Neither kernel performs single-threaded work over a bulk array, and the codebase
cannot silently regress into that shape again.

## Specification

### 1. `sembla_check_candidate_errors`

Grid-stride over candidates. It writes status, so reuse PRD 0002's reduction
(`sembla_record_validation_failure` / `sembla_commit_validation_status`, minimum
failing index, no intra-launch early exit that could suppress a lower index). Do
not introduce a second mechanism.

### 2. `sembla_prepare_effects`

Grid-stride both per-row loops; same reduction protocol.

### 3. Guard test covering every bulk-array shape

Assert that no emitted kernel both carries the single-thread guard and loops over
a bulk array — **candidates and rows alike**, since the candidate case is what
slipped through twice. Every allowed exception is named in the test with a
reason, so an exclusion is a visible decision rather than prose.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**`
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No semantic change. No work on `resolve_conflicts` (PRD 0006). No fusion (§L2),
no hoisting, no grouped-observation support (§L5).

## Acceptance criteria

**Local:**

1. Neither kernel carries the single-thread guard; both use grid-stride loops
   over their bulk arrays.
2. Both report failures through the PRD 0002 reduction.
3. The §3 guard test exists, covers candidate-indexed and row-indexed loops, and
   enumerates exceptions with reasons.
4. Diagnostics unchanged: `device_status()` codes and messages identical, and
   PRD 0003's diagnostic-equality fixtures still pass.
5. `examples/**`, goldens, and tracked CUDA differential evidence byte-unchanged;
   `cargo test --locked` and `scripts/check-rust.sh` green.

**Hardware (pending per §J14.2):**

6. A profile at >=2M rows shows both kernels reduced to a negligible share.

## Sequencing note

Run after PRD 0006. If 0006 clears the §L4 gate on its own, this PRD is
optimisation rather than necessity — worth doing, but its priority should be
re-assessed against the roadmap rather than assumed.

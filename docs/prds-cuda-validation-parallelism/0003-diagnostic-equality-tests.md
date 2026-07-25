# PRD 0003: Prove CPU and CUDA report identical validation diagnostics

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L3 makes failure reporting part of the observable
contract: the reported candidate index must be the minimum failing one, not
whichever thread wrote first.

PRD 0002 makes the reporting *capable* of being deterministic. This PRD makes it
*demonstrated*. Without it, a regression here is silent: a model that fails
validation still fails, just with a different index, and no existing test looks.

## Goal

A negative-model corpus proves that CPU and CUDA agree on validation
diagnostics, and that CUDA's answer does not depend on launch geometry.

## Specification

### 1. A negative corpus for validation failures

Add small fixtures — reduced scale, deterministic state, no RNG dependence —
each triggering exactly one validation failure class, with **multiple failing
rows at known indices** so that "first" and "minimum" are distinguishable and
"any" is detectably wrong:

- an out-of-range `Ref` dereference in a claim resource (`validate_claims`);
- a guard/rate expression that overflows or divides by zero
  (`validate_transition`);
- an effect expression that overflows `Int` (`validate_effects`);
- an output/summary expression that fails (`validate_outputs`).

Each fixture must fail at **at least three** distinct row indices, and the
lowest failing index must not be row 0 — otherwise every implementation agrees
by accident.

### 2. Equality assertions

For each fixture: run CPU, run CUDA, assert both reject, and assert the emitted
`status[0]` code and `status[1]` candidate index are **equal**. CPU is ground
truth per DESIGN.md §8.

### 3. Launch-geometry invariance

For each fixture, run CUDA under at least three launch configurations and assert
an identical reported index. If PRD 0002 factored the reduction into
host-testable logic, mirror this as a local unit test too; the GPU run then
confirms rather than establishes it.

### 4. Wire into the existing harness

Extend the CUDA differential corpus and its runbook
(`crates/sembla-cuda/scripts/run-differential-corpus.sh`) so these cases run
with the rest. Follow §J14.2: local criteria pass GPU-less, hardware criteria
are listed as pending.

## Allowed files

- `crates/sembla-cuda/tests/**`
- `crates/sembla-cli/tests/**` (only if the CPU arm needs a harness hook)
- `fixtures/**` — **new negative fixtures only**; no existing fixture may change
- `crates/sembla-cuda/scripts/run-differential-corpus.sh`
- `docs/cuda-differential-harness.md`
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No production-code changes; if a defect surfaces, this PRD records it and PRD
0002 is revised rather than patched here. No new validation classes. No
performance work. No grouped-observation cases (§L5).

## Acceptance criteria

**Local:**

1. At least four negative fixtures exist, each with ≥3 failing rows and a lowest
   failing index ≠ 0.
2. CPU runs assert the expected code and index for every fixture.
3. Every existing fixture, golden, and example is byte-unchanged.
4. `cargo test --locked` and `scripts/check-rust.sh` green; corpus listing
   includes the new cases; GPU-less runs skip gracefully with a named reason.
5. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2):**

6. For each fixture, CUDA reports the same `status[0]` and `status[1]` as CPU.
7. For each fixture, three launch geometries report identical indices.

## Note for the reviewer

If criterion 6 fails, the correct outcome is **not** to relax the assertion. It
means either the reduction in 0002 is wrong, or the CPU and CUDA validation
orders genuinely differ — and the second would be a semantic finding worth its
own decision record, not a test to be adjusted.

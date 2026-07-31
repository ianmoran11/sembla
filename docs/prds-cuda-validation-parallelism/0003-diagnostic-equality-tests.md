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

- a checked claim resource or ordering-key expression that overflows `Int`
  (status code 10 in the claim-validation path);
- a guard/rate expression that overflows (`validate_transition`);
- an effect expression that overflows `Int` (`validate_effects`);
- an output/summary expression that fails (`validate_outputs`).

The original out-of-range stored-`Ref` case is not a reachable execution
state: `StateStore` rejects it at construction, and `validate_claims` does not
implement a stored-reference range diagnostic. Adding that validation class is
outside this test PRD. Claim expressions are also checked eagerly with the
transition expressions; this corpus freezes the observable code/identity rather
than claiming a unique kernel owns code 10.

Each fixture must contain **at least three** distinct bad source rows, and the
lowest bad row must not be row 0 — otherwise every implementation agrees by
accident. For candidate-bearing transition, claim, and effect diagnostics,
`status[1]` is the minimum failing candidate. For outputs, the frozen contract
uses the target output-field identity in `status[1]`; the CPU test asserts the
earliest bad source row separately.

### 2. Equality assertions

For each fixture: run CPU and assert its semantic failure class and earliest
source row. CPU `TickError` does not expose CUDA's numeric status array, so each
case also freezes an explicit normalized CUDA `(status[0], status[1])` expected
value derived from that CPU semantic result and the unchanged `device_status()`
mapping. Run CUDA, assert rejection, and compare the raw emitted status words to
that expected value. CPU remains ground truth per DESIGN.md §8; the
normalization must not be represented as raw words emitted by CPU.

Candidate-bearing cases compare the minimum candidate. The output case compares
code 9 and its frozen target-field identity while separately retaining the CPU
earliest-row assertion.

### 3. Launch-geometry invariance

For each fixture, run CUDA under at least three explicit launch configurations
and assert identical committed status words. PRD 0002 is narrowly reopened to
permit a private test-only launch override and raw-status observation inside a
CUDA backend unit test; the normal launch choice and public API are unchanged.
Mirror the fixture cases through the existing host-testable reduction too; the
GPU run then confirms rather than establishes invariance.

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
- `docs/performance/cuda-differential-harness.md`
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)
- `docs/prds-cuda-validation-parallelism/0003-diagnostic-equality-tests.md`
  (this approved contract correction only)
- `docs/prds-cuda-validation-parallelism/0002-parallel-validation-kernels.md`
  and `crates/sembla-cuda/src/backend.rs` (narrow private test seam only)

## Non-goals

No production semantic, diagnostic, or public-API changes. The only production
file change is the PRD-0002-authorized private launch override used by a
`cfg(test)` hardware unit test; default launches and messages remain unchanged.
No new validation classes, performance work, or grouped-observation cases
(§L5).

## Acceptance criteria

**Local:**

1. At least four negative fixtures exist, each with ≥3 bad source rows and a
   lowest bad row ≠ 0.
2. CPU runs assert the expected failure class and earliest source row for every
   fixture, and each case freezes its normalized expected CUDA code/identity.
3. Every existing fixture, golden, and example is byte-unchanged.
4. `cargo test --locked` and `scripts/check-rust.sh` green; corpus listing
   includes the new cases; GPU-less runs skip gracefully with a named reason.
5. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2):**

6. For each fixture, CUDA raw `status[0]` and `status[1]` equal the normalized
   CPU-grounded expected diagnostic.
7. For each fixture, three explicit launch geometries report identical committed
   status words.

## Note for the reviewer

If criterion 6 fails, the correct outcome is **not** to relax the assertion. It
means either the reduction in 0002 is wrong, or the CPU and CUDA validation
orders genuinely differ — and the second would be a semantic finding worth its
own decision record, not a test to be adjusted.

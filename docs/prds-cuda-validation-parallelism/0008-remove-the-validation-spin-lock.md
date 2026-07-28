# PRD 0008: Remove the validation spin lock

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind, including the §J14.2 local/hardware split.

**This PRD corrects a defect in PRD 0002 of this folder**, which this folder
approved. 0002 was approved on local criteria with hardware criteria listed
pending, exactly as §J14.2 requires. Its first execution on a GPU was
2026-07-28, and it deadlocked on the first case.

`sembla_record_validation_failure` (`crates/sembla-cuda/src/codegen.rs:2795`)
opens a critical section with a spin lock:

```cuda
while (atomicCAS(status + 4, 0ULL, 1ULL) != 0ULL) { }
```

On an H100 (sm_90) the differential corpus passes `claim_key_overflow` at launch
geometry `1x1` and **hangs indefinitely** at `1x32`. Observed at 100% GPU
utilisation for 2h31m before the run was killed. The same suite completed in
**23.04s** on 2026-07-19, before this code existed.

`1x1` is a single thread, so the lock is never contended. `1x32` is one full
warp, so 32 threads contend and the kernel never terminates.

The comment above the function reads:

> Requires independent thread scheduling (sm_70+), which every supported device
> has.

ITS **is** present on sm_90 and the kernel hangs anyway. The comment states a
necessary condition as if it were sufficient. Evidence, including the
23-second reproduction: `docs/evidence/cuda-validation-deadlock-20260728/`, and
`DECISIONS.md` §L12.

## What the lock is protecting

The scratch slots hold the lexicographically smallest
`(scan, order_identity, branch)` triple seen so far, plus payload paired with it:

| slot | contents |
|---|---|
| `status[4]` | the lock |
| `status[5]` | scan |
| `status[6]` | order identity |
| `status[7]` | code |
| `status[8]` | branch |
| `status[9]` | reported identity |
| `status[10..=11]` | details |

The lock exists to keep the payload **paired** with the winning key. That
requirement is real and must not be weakened: a diagnostic that reports the code
from one failure and the identity from another is worse than no diagnostic.

## Goal

Deterministic first-failure selection with **no critical section in generated
device code**, and a byte-identical diagnostic.

## Specification

### 1. No lock, and this is the criterion

The generated CUDA must contain no spin lock, no `atomicCAS` retry loop over a
mutex word, and no critical section. A test asserting this over the generated
source prevents regression — `codegen.rs` already has tests that inspect
generated text.

`sembla_atomic_min_i64` and `sembla_atomic_max_i64` contain `atomicCAS` loops,
but those are ordinary lock-free CAS reductions that terminate: each iteration
either succeeds or the observed value moved monotonically closer. **They are not
in scope** and must not be changed.

### 2. Prefer the pattern this codebase already proves

Conflict resolution solves the same problem — lexicographic argmin over a
composite key, carrying payload — without a lock, in four passes of pure
`atomicMin`: `sembla_reduce_claim_keys`, `_rules`, `_entities`, `_instances`
(`codegen.rs:2249-2252`). Each pass reduces one component, then the next pass
considers only instances matching the winning prefix.

That construction is already load-bearing for §E3 and is verified by the
differential corpus. **Prefer mirroring it.** The validation path differs in
that failures are recorded from inside other kernels rather than in a dedicated
reduction, so the shape will not transfer unchanged; say how it was adapted.

An alternative is to pack the ordering key into a single 64-bit word reduced by
`atomicMin`, with payload recovered in a second phase by the thread whose key
matches the winner. If the three components cannot be packed without losing
ordering, say so and use the multi-pass form.

**If neither can reproduce the current selection exactly, stop and report it
rather than changing the diagnostic.**

### 3. The diagnostic is byte-identical

For every corpus case, at every geometry, the committed `status[0..=3]` and all
reported fields must equal what the CPU oracle produces and what the current
code produces at `1x1` — the geometry that does not deadlock and therefore has
a trustworthy result today.

PRD 0003 of this folder established diagnostic equality tests. They must pass
unchanged.

### 4. Add a geometry that contends harder

`GEOMETRIES` is `[(1, 1), (1, 32), (3, 4)]`
(`crates/sembla-cuda/tests/support/diagnostic_cases.rs:6`). `1x32` is one warp
and `3x4` is twelve threads over three blocks; neither exercises many full warps
across many blocks.

Add at least one geometry with multiple full warps in multiple blocks — `(4, 128)`
or similar. **This is not optional.** The defect being fixed was invisible at
`1x1` and fatal at `1x32`; the next one of its kind may be invisible until the
occupancy is realistic.

Update the `assert_eq!` on `GEOMETRIES` and the corpus script's `--list` output
to match.

### 5. Bound the test

The lib test must not be able to hang a session again. Whatever mechanism suits
— an internal deadline, a bounded iteration count with a failure, a documented
`timeout` in the corpus script — a deadlock must surface as a **failure**, not
as an indefinite wait. The collector now wraps the corpus in `timeout` and exits
7, but that is the outer backstop; the test should not rely on it.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**`, `crates/sembla-cuda/src/**` tests
- `crates/sembla-cuda/scripts/run-differential-corpus.sh`
- `docs/evidence/**` (new evidence only)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)
- `DECISIONS.md` — §L12 only, to record the verdict

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

No change to what is validated, to the diagnostic codes, or to §E3 conflict
resolution. No change to `sembla_atomic_min_i64`/`_max_i64`. No performance work
— validation is not on the measured critical path and this PRD must not be
justified or judged by wall time. No CPU, IR, or Lean changes. No new
dependencies.

## Acceptance criteria

**Local (required for approval):**

1. Generated CUDA contains no spin lock or critical section, asserted by a test
   over the generated source.
2. The selection rule is stated in the implementation notes, with the argument
   for why it produces the same `(scan, order_identity, branch)` winner and the
   same paired payload as the current code.
3. `GEOMETRIES` includes at least one multi-warp, multi-block entry, and the
   corpus script's `--list` output matches.
4. A deadlock surfaces as a test failure rather than an indefinite wait.
5. `cargo test --locked` and `scripts/check-rust.sh` green; CUDA-feature
   `cargo check`/`clippy` green without claiming GPU execution.
6. Every golden byte-identical, including the manifest and `final_state_sha256`.
7. `python3 scripts/check-markdown-links.py` passes.

**Hardware (§J14.2 — and this time the command exists, per §M3):**

8. `BENCH_CORPUS=1 bash run-demographic-benchmark.sh` completes with
   `differential-corpus/exit-code.txt` = 0.
9. The negative corpus passes at **every** geometry including the new one, with
   the per-case status lines captured.
10. Total corpus duration recorded and compared against the 23.04s of
    2026-07-19. A large regression is itself a finding.

## Note on expectations

This is a correctness fix with no performance component. Do not report a wall
time as its result.

The reproduction is 23 seconds, so the fix should be verifiable quickly. If it
is not — if the lock-free selection turns out to be genuinely hard to make
bit-identical — that is worth knowing early, and §2's instruction to stop rather
than change the diagnostic applies.

## Why this folder, and what it says about the folder

0002 was correct by every criterion available to it at the time, and the runner
did nothing wrong: hosted CI has no GPU, §J14.2 permitted approval on local
criteria, and the hardware criteria were properly listed as pending. What was
missing was any *mechanism* that would later execute them, which is why the
defect survived three GPU sessions before anything ran it.

`DECISIONS.md` §M3 now requires a deferred hardware criterion to name a runnable
command. This PRD's criterion 8 is that command.

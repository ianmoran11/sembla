# PRD 0004: The always-fires hazards, revisited with numbers

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind.

PRD 0002 added the guarded racing clock and it worked: `birth_activate` computes
`ln` for 2.47% of candidates against a 2.469% fire probability,
`overseas_arrive` 1.98%, `internal_arrive` 1.77%.

But it left most of the work in place, by a **deliberate and correctly recorded**
choice. §3 of that PRD asked whether to special-case hazards whose threshold is
exactly zero; the implementer declined:

> Do not special-case `threshold == 0`: open uniforms are admitted, and the
> canonical `ln` race time remains available for any contested model.

That is a defensible call — one code path, no special case, and the exact race
time stays available should a contested model ever need it. It was made before
the distribution was known.

**The distribution is now known**, and it changes the balance:

| transition | enabled candidates | `ln` computed | |
|---|---:|---:|---:|
| **age_monthly** | **18,547,052** | **18,547,052** | **100%** |
| clear_event | 279,009 | 279,009 | 100% |
| birth_activate | 3,721,059 | 91,823 | 2.5% |
| overseas_arrive | 972,512 | 19,280 | 2.0% |
| internal_arrive | 596,491 | 10,560 | 1.8% |
| — | — | — | — |
| **total** | **79,408,910** | **19,117,749** | **24.1%** |

**`age_monthly` alone is 97% of the remaining `ln` calls.** The guarded filter
removed three quarters of the work; nearly all of what survives is one
transition, retained by choice rather than by necessity.

## Why these can be answered without a draw at all

`age_monthly` and `clear_event` declare `hazard = 1e300`, so
`threshold = exp(-λ·dt)` is exactly `0.0`, and `uniform_f64` excludes zero.
`u > 0` is therefore true for every candidate: **they always fire, provably,
with no draw.**

Both declare `contests = 0`, so their `race_time` is never consumed by §E3's
argmin — it is used only for the `< dt` test whose answer is already known.

And skipping the draw perturbs nothing else. `DECISIONS.md` §E1 makes each draw
a pure function of `(seed, tick, rule_id, entity_id, draw_idx)` with no stream
state, so not drawing for one candidate changes no other candidate's value.
**Coordinate purity is what makes this safe**, and it is the reason the same
optimisation would be unthinkable with a stateful generator.

## Goal

Transitions that provably always fire and whose race time is never consumed skip
both the draw and the `ln`. Results are unchanged, bit for bit.

## Specification

### 1. Both conditions must hold, and both must be checked

The fast path applies only where:

- `threshold == 0.0` exactly, computed from the transition's constant hazard; and
- the transition's `race_time` is **not consumed** — no contests, and no other
  consumer of the sampled time.

Checking only the first would be wrong: a contested transition needs its exact
race time for §E3's argmin even when it always fires. Derive the second
condition from the IR rather than from the benchmark model's shape.

### 2. Preserve 0002's reasoning, do not replace it

This is an additional path alongside the guarded filter, not a replacement. The
canonical `ln` race time must remain reachable for every transition that
consumes it — that was the implementer's stated reason for declining, and it
stays satisfied.

### 3. Preserve every diagnostic

The `entity_id` conversion and its `EntityIdOverflow` check must still run for
every enabled candidate, drawn or not. Error types, messages and ordering are
unchanged.

### 4. Prove it rather than test it

A test showing the firing set is unchanged is necessary and not sufficient — it
would pass for a model where the fast path never triggers.

State the argument explicitly in the implementation notes: why
`threshold == 0.0` implies every open uniform fires, why the excluded-zero
property of `uniform_f64` is load-bearing, and why §E1's coordinate purity means
a skipped draw is invisible. Then test that the argument's preconditions are
detected correctly, including a **contested** transition with a degenerate
hazard, which must *not* take the fast path.

### 5. Measure under the revised protocol

Wall time primary, single-worker figures separate, CPU efficiency reported.
**Report the per-transition `ln` counts again** in the form 0002's evidence used,
so the table above can be compared directly.

## Allowed files

- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/src/rng.rs` — only if genuinely required
- `crates/sembla-runtime/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

**No RNG change.** Not the algorithm, not the round count, not the uniform
construction — that is the deferred decision in the README and it regenerates
every golden. This PRD regenerates none.

No change to §E3 conflict resolution. No change to the guarded filter from 0002.
No tiling work — that is 0003. No IR, Lean, CUDA, or CLI changes.

## Acceptance criteria

1. Transitions meeting both §1 conditions skip the draw and the `ln`; a test
   demonstrates a degenerate **contested** transition does not.
2. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
3. §4's argument is in the implementation notes, not merely tested.
4. Diagnostics unchanged, including the entity-id overflow path.
5. Per-transition `ln` counts reported in 0002's format.
6. `cargo test --locked` and `scripts/check-rust.sh` green.
7. Before/after under the revised protocol.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

This removes ~97% of the surviving `ln` calls and the Philox draws behind them.
But 0002 reduced user time 10.7% and moved wall time by 0.19%, because the
critical path is the untiled serial remainder. **If 0003 has not landed, expect
the same shape here: user time down, wall time flat.**

That is not a reason to do it later. It is a reason to read its result on user
time and CPU efficiency, and to treat a flat wall time as confirmation of 0003's
premise rather than as failure. If 0003 *has* landed and wall time still does not
move, that is a finding worth more than the change.

A note on the sequencing, since this reopens a decision 0002 recorded: nothing
was done wrong there. The choice was correct on the information available, and
the information that changes it — that one transition holds 97% of the residue —
is precisely what 0002's own evidence produced. Reopening a recorded decision
because its own measurement moved the balance is the process working.

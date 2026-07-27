# PRD 0001: Stop rebuilding the host state every tick

## Context

Read `docs/prds-cuda-host-path/README.md` first; its constraints bind, including
the §J14.2 local/hardware split.

`CudaBackend::download_state_store` (`crates/sembla-cuda/src/backend.rs:1354`)
runs once per tick:

```rust
let state = self.stream.memcpy_dtov(&self.state)?;
let inputs = self.stream.memcpy_dtov(&self.inputs)?;
let input_counts = self.stream.memcpy_dtov(&self.input_counts)?;
let initial = unpack_state(&self.model, &self.layout, &state);
let mut store = StateStore::new(&self.model, initial)?;
store.replace_backend_inputs(unpack_inputs(&self.model, &self.layout, &inputs, &input_counts))?;
```

After the three copies, everything that follows is host allocation:

1. `unpack_state` builds a fresh `Vec<TableInit>`, allocating a new `Vec` for
   every column;
2. `StateStore::new` validates it and allocates every column *again* into
   `current` (`state.rs:223`);
3. and then clones the whole of `current` into `next` (`state.rs:278`).

So a five-million-row state is allocated twice and copied three times, per tick,
to produce a structure whose *shape* is identical to the one discarded at the
end of the previous tick. Only the values differ.

Measured at 5M rows over 2 ticks: `state_transfer` 232.6 ms against
`state_reconstruct` **766.8 ms**, 45.8% of CUDA wall time.

## Goal

The host `StateStore` is allocated once and refreshed in place from the device.
Nothing about what is computed, accepted, or reported changes.

## Specification

### 1. Add an in-place refresh to `StateStore`

Add a method that overwrites an existing `StateStore`'s committed columns from
backend-supplied table data, reusing the existing allocations rather than
replacing them.

Requirements:

- **Shape must be checked, not assumed.** If the incoming row counts or column
  set do not match what the store was built with, that is an error, not a
  silent resize. The CUDA layout is fixed at construction so a mismatch means
  something upstream is wrong.
- Column buffers are overwritten in place. `Vec::clear` + `extend`, or
  `copy_from_slice` where lengths match, so capacity is retained and no large
  allocation is returned to the allocator between ticks. The 4.04× measured in
  `alloc_spike` is precisely this effect.
- The method is `pub(crate)` or otherwise not part of the general surface unless
  a caller outside the backend needs it.

### 2. Validation must be identical, not merely present

`StateStore::new` calls `validate_table_initializers` — row counts, enum variant
ranges, `Ref` target bounds. **The refresh path must run the same checks and
produce the same errors, with the same messages, in the same order.**

This is the criterion the PRD turns on. A refresh that skips validation would be
faster and would still pass every golden, because the goldens contain valid
states. It would fail silently, later, on the first malformed device readback —
in the oracle's own state, where nothing else can catch it.

Prefer sharing one validation routine between `new` and the refresh over
duplicating it. If the shared routine must change shape to be callable from
both, that refactor is in scope.

### 3. Handle the `next` buffer deliberately

`StateStore::new` sets `next: current.clone()`, and `prepare_next` later does
`next.clone_from(&current)`. Reusing the store across ticks means the `next`
buffer is also reused, which is part of the win — but the write-prepared state
machine (`write_prepared`, `commit`, `discard_writes`) must remain coherent.

State in the implementation notes what happens to a refresh arriving while a
write buffer is prepared. Rejecting it is a reasonable answer; silently
discarding staged writes is not.

### 4. Hold one store across ticks in the backend

`CudaBackend` keeps a single `StateStore` and refreshes it per tick.
`download_state_store` becomes a refresh of that store rather than a
constructor.

If `unpack_state`'s intermediate `Vec<TableInit>` can be bypassed — reading
directly from the downloaded byte buffer into the existing column buffers — that
removes the first of the two allocations as well. Do it if it does not
compromise §2's validation; if it does, keep the intermediate and say so.

`download_hash` performs its own copies and is not part of this PRD.

### 5. Measure with the phase instrumentation

Re-run the `cuda-l4-20260726` case — 5M rows, 2 ticks — with `--timing-json`,
and report the full phase table before and after so it is directly comparable to
the README's. `state_reconstruct` is the headline.

Report `state_transfer` too: it should be unchanged, and if it moves, something
other than allocation changed and the result needs explaining.

## Allowed files

- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-runtime/src/state.rs`
- `crates/sembla-cli/src/main.rs` — **added 2026-07-27 by operator authorisation**,
  see below
- `crates/sembla-runtime/tests/**`, `crates/sembla-cuda/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-cuda-host-path/README.md` (status notes only)

### Why the CLI is in scope (authorised 2026-07-27)

The original list omitted the CLI, and that made this PRD unachievable as
written. `CudaTickObservation.state` is an **owned** `StateStore`, and
`main.rs` moves it out at each tick. Retaining one store across ticks therefore
requires changing that contract, and the contract crosses into the CLI. Confined
to `backend.rs` and `state.rs`, the only options were to clone the retained
store every tick — which recreates the allocations this PRD exists to remove —
or to break the CUDA build.

The runner identified the conflict and asked rather than guessing, and declined
to ship a cloning path dressed as reuse. That was correct.

**The exception is limited to the ownership change**: the backend lends the
refreshed state during a tick and yields it once at the end. Specifically it does
not extend to redesigning the CUDA run loop, to the CPU path, or to the
`--timing-json` phases beyond what the signature forces. `state_transfer` and
`state_reconstruct` must still be measured where they are today, or the
before/after comparison this PRD exists to produce becomes meaningless.

Note the change is `#[cfg(feature = "cuda")]`, so it cannot be compiled on the
development machine. Its first build is on the GPU host, per §J14.2.

## Non-goals

**No device-side observation.** That is a §K6/§L5 semantic decision with its own
folder, and this PRD must not pre-empt it — see the README.

No change to what is transferred from the device or when. No change to
`download_hash`. No change to the CPU backend's tick loop. No evaluator changes
— those are `prds-evaluator-throughput`. No kernel changes: they are 0.56% of
wall time. No IR or Lean changes. **CLI changes are limited to the authorised
ownership change above** — nothing else in `main.rs` is in scope. No new
dependencies.

## Acceptance criteria

**Local (required for approval):**

1. `StateStore` exposes an in-place refresh that reuses column allocations, with
   a test proving buffer capacity is retained across refreshes.
2. **Validation parity**: a test asserts the refresh path rejects the same
   malformed inputs as `StateStore::new`, with identical error messages —
   covering at minimum a row-count mismatch, an out-of-range enum variant, and
   an out-of-bounds `Ref`.
3. A test covers the §3 interaction between refresh and a prepared write buffer,
   and the chosen behaviour is recorded in the implementation notes.
4. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`. `git diff --stat` shows none of them.
5. `cargo test --locked` and `scripts/check-rust.sh` green.
6. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2, listed in the implementation notes):**

7. `cargo build --release --features cuda` compiles on the GPU host.
8. The `cuda-l4-20260726` case re-run with `--timing-json`, full phase table
   reported before and after.
9. CPU/CUDA differential equality holds on the corpus including the demographic
   no-grouped model.
10. `state_transfer` unchanged; any movement explained.

## Note on expectations

`state_reconstruct` is 45.8% of CUDA wall time and this PRD targets the
allocation inside it, not the transfer. Removing it entirely would take the 5M
2-tick case from 1,674 ms to roughly 900 ms; extrapolated to the frozen
10M/24-tick case that is 31.8 s toward the high teens. **Both are projections
from a 5M/2-tick measurement and neither is verified at gate scale.**

Some residue will remain: the values still have to be written into the reused
buffers, which is a memcpy that no amount of reuse removes. A result around half
the current `state_reconstruct` would be a good outcome; the full 45.8% would
be a surprise.

Do not report a §L4 ratio as this PRD's result. The gate has passed and §L8
records that it should no longer steer work.

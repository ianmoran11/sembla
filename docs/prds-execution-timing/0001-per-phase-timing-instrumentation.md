# PRD 0001: Per-phase timing instrumentation for both run paths

## Context

Read `docs/prds-execution-timing/README.md` first; its constraints bind,
including the inertness requirement and the §J14.2 local/hardware split.

The CUDA tick loop (`crates/sembla-cli/src/main.rs`, from ~2656) is:

```rust
let observation = backend.run_tick_observed()?;
state = observation.state;
if let Some(hashes) = hashes.as_mut() { hashes.push(state.state_hash()); }
let views = executor::observe_views(model, &state, params)?;
```

and `CudaBackend::run_tick_observed` (`crates/sembla-cuda/src/backend.rs:518`)
calls `execute_tick()`, then reads back `wins` and `deferred`, then at line 573
calls `download_state_store()` — which copies the whole device state to the
host and rebuilds a `StateStore` through `unpack_state` and `StateStore::new`.

Four very different costs are collapsed into one number today: kernel
execution, device-to-host transfer, host state reconstruction, and host
observation. They have four different fixes.

## Goal

Each tick's wall time is attributed to named phases, in both backends, with the
instrumentation provably inert when disabled.

## Specification

### 1. A default-off flag that writes a file

Add a CLI option — `--timing-json <path>` — that writes one JSON document at
the end of the run. Absent, nothing is timed, nothing is written, and no code
path changes.

A file rather than stdout or stderr: the Hyperstack collector retrieves
artifacts by path, and stdout already carries hashes that tests match on.

### 2. Phases to attribute — CUDA path

Per tick, and summed:

| Phase | Boundary |
|---|---|
| `kernels` | `execute_tick()`, including the synchronisation of §4 |
| `readback_control` | the `wins` and `deferred` device-to-host copies |
| `state_transfer` | the device-to-host copies inside `download_state_store` |
| `state_reconstruct` | `unpack_state` + `StateStore::new` + `replace_backend_inputs` |
| `state_hash` | the per-tick hash, when `HashMode::EveryTick` |
| `observe_views` | `executor::observe_views` |
| `report` | tick-report and CSV assembly |
| `other` | tick wall time minus the above; must never be negative |

`state_transfer` and `state_reconstruct` **must be separate**. One is a PCIe
cost and the other is host allocation; conflating them is exactly the ambiguity
this PRD exists to remove.

### 3. Phases to attribute — CPU path

The comparable subset, so the two backends can be read side by side:
`execute_tick`, `state_hash`, `observe_views`, `report`, `other`.

### 4. The asynchrony subtlety

CUDA kernel launches are asynchronous: `execute_tick()` can return before the
GPU has finished, and the wait is then absorbed by whichever copy synchronises
first. Timed naively, `kernels` would read near-zero and `readback_control`
would absorb GPU time — reproducing in a new form exactly the misattribution
this PRD is meant to fix.

**When instrumentation is enabled, synchronise the stream at the end of
`execute_tick()`** so GPU wait time lands in the `kernels` bucket.

This is a real perturbation: it removes any launch/copy overlap and can change
total wall time. It must be:

- applied **only** when instrumentation is enabled, so uninstrumented runs are
  untouched;
- recorded in the JSON as a flag (e.g. `"kernel_sync_inserted": true`);
- described in the evidence, so nobody compares an instrumented total against
  an uninstrumented one without knowing.

Where the existing code already synchronises, say so in the implementation
notes rather than adding a second synchronisation.

### 5. Inertness — the criterion this PRD turns on

With the flag absent, the run must be **byte-identical** to the same build
without the instrumentation: every output CSV, every summary, every manifest
field including `final_state_sha256`, and stdout.

Add a test that runs the fixed case with and without `--timing-json` and
asserts the outputs and manifest match. Timing must not change results even
when enabled — the only permitted difference is the extra file and, on CUDA,
the §4 synchronisation.

Prefer a design where the disabled path holds no timer state at all, rather
than one that collects timings and discards them.

### 6. Not a `FeatureSet` entry

Do not thread this through `FeatureSet` or the `--enable` mechanism. Those
carry semantic flags under `DECISIONS.md` §E8, which exists because a flag that
changes results while staying invisible to the manifest would falsify the §2
contract. Timing cannot change results, so it is not that kind of flag, and
registering it as one would imply a meaning it does not have — against §E8's
own "no inert syntax" half.

Record this reasoning in the implementation notes. §5's on-versus-off test is
what makes the argument checkable rather than asserted. No `DECISIONS.md` entry
is required; if the reviewer disagrees, that disagreement is worth an entry.

### 7. Output schema

One JSON document, stable enough for the collector and for diffing across
sessions:

- session: backend, scale, ticks, seed, repository commit, binary SHA-256
- `kernel_sync_inserted`, and the timer resolution used
- per-tick rows: tick index and each phase in milliseconds
- totals per phase, plus total wall time
- a self-check that per-tick phases sum to the tick wall time within tolerance,
  with `other` absorbing the remainder and asserted non-negative

Use `std::time::Instant`. No new dependencies.

### 8. Overhead

Per-tick granularity only. **No timer inside a per-row loop.** At 24 ticks the
overhead is a few dozen `Instant::now()` calls and is immaterial; state the
measured overhead on the CPU path in the implementation notes.

## Allowed files

- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-runtime/src/executor.rs` — only if a phase boundary genuinely
  requires it; prefer timing at the call site
- `crates/sembla-cli/tests/**`, `crates/sembla-runtime/tests/**` (tests only)
- `docs/prds-execution-timing/README.md` (status notes only)

## Non-goals

No performance change of any kind — this PRD measures, it does not optimise. No
device-side observation, no change to `download_state_store`'s behaviour beyond
timing it, no change to what is transferred or when. No `nsys` integration; that
stays a session-level tool. No IR, Lean, evaluator, or semantic change. No new
dependencies. No manifest field.

## Acceptance criteria

**Local (required for approval):**

1. `--timing-json` exists, is off by default, and writes the §7 schema on the
   CPU path.
2. **Inertness**: a test proves outputs, summaries, manifest and stdout are
   byte-identical with the flag absent, and that results are unchanged with it
   present.
3. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, and the tracked CUDA differential
   evidence. `git diff --stat` shows none of them.
4. `cargo test --locked` and `scripts/check-rust.sh` green.
5. §6's reasoning and the measured CPU-path overhead are in the implementation
   notes.
6. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2, listed in the implementation notes):**

7. `cargo build --release --features cuda` compiles on the GPU host.
8. The CUDA path emits all §2 phases, with `state_transfer` and
   `state_reconstruct` separated.
9. Phase totals reconcile with wall time; `other` is non-negative.
10. Collected at the `cuda-l4-20260726` case (5M rows, 2 ticks) so the new
    attribution is directly comparable to the ~10,200 ms unaccounted block.

## Note on expectations

This PRD produces no speedup and should not be judged on one. Its output is a
number that decides whether the next piece of work is device-side observation,
a cheaper state readback, or nothing at all.

The most likely surprise is that `state_reconstruct` dominates — rebuilding a
full `StateStore` per tick is O(state) host allocation, and it exists only so
host-side `observe_views` has something to read. If that is what the numbers
say, the fix is architectural rather than an optimisation, and worth knowing
before any more GPU time is bought.

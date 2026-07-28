# Sweep throughput PRDs

Make batch runs cheap. Run from the Sembla repository with:

```text
/piprd run docs/prds-sweep-throughput
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding.

## Why this folder exists

The driver workflow is **uncertainty and sensitivity analysis** — many draws
over the same model and population, differing only in parameters and seed. After
`prds-device-observation` landed (§L11), a single 1M-slot run over 24 ticks
costs roughly 3.1 s on an H100, of which about **2.2 s is startup**: JIT-compiling
the CUDA kernels, loading the state, uploading it.

`sweep` pays that per draw. `crates/sembla-cli/src/main.rs:1809` loops over
draws and calls `execute_backend_output_with_features`, which constructs a fresh
backend each time — so `CudaBackend::new` recompiles the entire model through
NVRTC on **every draw**. The same loop also does:

```rust
let initial = initial_tables.clone();   // main.rs:1828
```

a full host copy of the initial state per draw — about 458 MiB at 10M slots.

Neither is inherent to what a sweep computes. Both are per-draw costs for work
whose result is identical across draws.

## The size of it

A hundred 1M draws, 24 ticks:

| | total |
|---|---:|
| today | ~310 s |
| after the device-side `wins`/`deferred` reduction | ~230 s, of which ~220 s is startup |
| after this folder | **~11 s** |

Roughly **28×**, which independently reproduces the figure the earlier
draw-independence analysis reached from a different direction.

**This folder is worth more to the driver workflow than any remaining
simulation optimisation**, and it needs no GPU session to develop — the change
is in the CLI and the backend lifecycle, not in kernels.

The two compound rather than compete: the reduction is what makes the per-draw
simulation cost small enough for startup to dominate.

## Binding constraints

- **Results must not change.** Every golden, every CSV and hash golden, the
  frozen demographic state fixture, every run and sweep manifest including
  `final_state_sha256`, and the tracked CUDA differential evidence must be
  **byte-identical**. A sweep's outputs must not depend on how many draws
  preceded it in the same process.
- **Draw independence is the property being relied on and must be proved, not
  assumed.** A retained backend introduces exactly one hazard: state leaking
  from draw *n* into draw *n+1*. Every PRD here must show that draw *k* run
  alone is byte-identical to draw *k* run after *k−1* others.
- **§E2 determinism levels are unchanged.** Reuse must not alter which draws are
  taken, only what is rebuilt between them.
- **The CRN/independent noise distinction is semantic.** `NoiseMode::Crn` reuses
  one seed; `Independent` derives a per-draw replica seed. A backend that takes
  its seed at construction cannot simply be reused across `Independent` draws —
  re-seeding must be explicit and correct, not incidental.
- **Backend identity checking must not be weakened.** The sweep asserts the
  backend device identity is stable across draws (`main.rs:1843`). Retaining one
  backend makes that trivially true, which removes a real check. State what
  replaces it.
- **No new dependencies.**

## What is already built and should be reused

`prds-cuda-host-path/0001` added an in-place `StateStore` refresh that reuses
column allocations and runs the same validation as the constructor. It exists to
avoid exactly the reallocation the draw loop does per draw. Prefer reusing it
over writing a second mechanism.

## Local versus hardware acceptance (inherited from §J14.2)

Most of this folder is CPU-side and fully testable locally: the draw loop, the
state reset, the manifest contents, byte-identical outputs. The CUDA lifecycle
change cannot be compiled on the development Mac.

Per §M3, a PRD here that defers a hardware criterion must name the command that
will run it. `BENCH_CORPUS=1 BENCH_PROFILE=1 bash run-demographic-benchmark.sh`
is the collector; a sweep-specific measurement needs its own stage, and adding
that stage is **in scope for the PRD that needs it**.

## Measurement protocol

Wall time for a whole sweep is the headline — that is what the workflow waits
on. Report per-draw cost as total ÷ draws, and separately report the cost of
draw 0 against the median draw, since the whole point is that draw 0 pays a
setup the others should not.

`--timing-json` per-phase instrumentation exists for a single run. If it does
not aggregate usefully over a sweep, say so rather than reporting a per-tick
table that hides the setup being removed.

## Before running a PRD from here

```sh
python3 scripts/check-prd-allowlist.py docs/prds-sweep-throughput/0001-*.md
```

It lists every repo path the PRD names that its allowed-file list does not
cover. Most will be read-only context; the point is that the list is short
enough to eyeball. `0001` stalled a managed run for five attempts on exactly
this defect — it required a sweep stage in the collector while omitting the
collector from its own allowed files — and this check reproduces that finding in
under a second.

## PRDs

- `0001-reuse-the-backend-across-draws` — implemented locally 2026-07-28:
  construct one retained CPU/CUDA backend, reset and reseed it per draw, and
  capture identity once. Local correctness and 1M CPU evidence are recorded in
  [`../evidence/sweep-backend-reuse-20260728.md`](../evidence/sweep-backend-reuse-20260728.md);
  10M and GPU collector results remain hardware-pending.

Later PRDs are **deliberately unwritten** and re-scoped from what 0001 measures,
per §M1. The obvious candidate is running several draws concurrently on one
device, since a 1M draw badly underutilises an H100 — but that is worth
measuring before it is scoped, and it depends on 0001 landing first.

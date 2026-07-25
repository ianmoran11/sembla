# PRD 0002: Parallelise the per-row validation kernels

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L1–L5 (added by PRD 0001) are normative.

Four generated kernels validate every row on one GPU thread. Two things make
them serial and both must change:

1. `crates/sembla-cuda/src/codegen.rs` opens each body with
   `if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;` and
   then loops `for (unsigned long long row = 0; row < row_counts[...]; ++row)`.
2. `crates/sembla-cuda/src/backend.rs` launches them with the `one` config
   (`grid_dim: (1,1,1), block_dim: (1,1,1)`, around line 522), which the
   genuinely single-threaded status kernels also use.

## Goal

The four per-row validation kernels execute across the device and report the
**minimum** failing candidate index, independent of launch configuration and
scheduling order. Meaning, timing of checks, and accepted inputs are unchanged.

## Specification

### 1. Grid-stride the four row loops (`codegen.rs`)

For `sembla_validate_claims`, `sembla_validate_transition`,
`sembla_validate_effects`, and `sembla_validate_outputs`:

- Remove the `blockIdx.x != 0 || threadIdx.x != 0` guard.
- Replace each row loop with a grid-stride loop:
  `for (unsigned long long row = blockIdx.x * blockDim.x + threadIdx.x;
   row < row_counts[...]; row += (unsigned long long)gridDim.x * blockDim.x)`.
- Leave every other kernel untouched, including the genuinely single-threaded
  status kernels (`sembla_reset_status`, `sembla_check_candidate_errors`,
  `sembla_record_aggregate_errors`, `sembla_check_output_errors`,
  `sembla_mark_effect_aggregates`, `sembla_prepare_effects`,
  `sembla_prepare_outputs`, `sembla_validate_claim_compatibility`). Those do
  O(1) or O(rules) work; parallelising them is out of scope.

### 2. Deterministic minimum-index reporting

Replace first-writer-wins (`status[0] = code; status[1] = candidate; return;`)
with a reduction that yields the same answer for any launch geometry:

- `status[1]` accumulates via `atomicMin` over failing candidate indices.
- `status[0]` is set to the check's code such that a reader never observes a
  code without its index, or an index from a different check.
- **The early-exit on `status[0] != 0` must not skip work within its own
  launch.** A thread bailing out because a *sibling* thread already failed can
  suppress a lower index and make the reported candidate depend on scheduling.
  Cross-launch short-circuiting (a prior check already failed) remains
  permitted, because that ordering is fixed by the host.

The chosen mechanism is the implementer's, but it must satisfy: for a model
with failures at candidate indices `{a, b, c}`, the reported index is
`min(a,b,c)` for every launch configuration and every run.

### 3. Launch the four kernels across the device (`backend.rs`)

Use `LaunchConfig::for_num_elems(rows)` — as the already-parallel kernels do —
rather than the `one` config. The `one` config remains for the status kernels.
Where a validation launch covers a table, `rows` is that table's row count.

### 4. Preserve the error surface

`device_status()` mapping from `status[0]` codes to messages is unchanged, and
so are the codes themselves. Users see identical diagnostics.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**` (new local tests only)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No change to which expressions are validated, when validation runs, what it
accepts, or the codes it reports. No fusion into execution kernels (§L2). No
hoisting out of the per-tick path. No grouped-observation support (§L5). No
touching the already-parallel kernels.

## Acceptance criteria

**Local (must pass without a GPU):**

1. `cargo build --locked -p sembla-cuda --features cuda` succeeds; the whole
   workspace builds and `scripts/check-rust.sh` passes.
2. A test asserts on the **emitted CUDA source** that none of the four kernel
   bodies contains `blockIdx.x != 0`, and that each contains a stride increment
   using both `gridDim.x` and `blockDim.x`.
3. A test asserts the emitted source contains no bare
   `status[1] = candidate` assignment inside those four kernels — the index
   reaches `status[1]` only through the minimum reduction.
4. A test asserts the remaining single-threaded kernels **still** carry the
   `blockIdx.x != 0` guard, so the change is confined to the four.
5. `examples/**`, all CSV/hash goldens, and the tracked CUDA differential
   evidence are byte-unchanged (`git diff --stat` shows none of them).
6. Legacy CPU goldens unchanged; `cargo test --locked` green.

**Hardware (recorded as pending per §J14.2, executed later):**

7. The frozen §L benchmark case runs on CUDA and completes without error.
8. A model with deliberately invalid rows reports the **same** `status[0]` and
   `status[1]` under at least three different launch geometries.

Criterion 8 may be expressed locally as a host-side unit test over the
reduction logic if the implementation factors it out; the GPU run then confirms
rather than establishes it.

## Narrow test-seam revision for PRD 0003

PRD 0003 demonstrated that `CudaBackend` otherwise exposes neither the raw
committed status words nor a way to select distinct validation launch
geometries. For criterion 8 only, this PRD is reopened to permit a private
validation-launch override in `backend.rs`, set directly by a `cfg(test)` unit
hardware test. The same private test may download the existing status buffer
after an expected execution error.

This seam adds no public API, changes no status code or message, and has no
production setter. With no test override, all four validation kernels continue
to use `LaunchConfig::for_num_elems(rows)` exactly as specified above. It does
not authorize a new validation class or a change to output-field identity.

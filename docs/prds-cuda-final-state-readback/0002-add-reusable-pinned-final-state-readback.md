# PRD 0002: Add reusable pinned final-state readback

## Context

Read `docs/prds-cuda-final-state-readback/README.md` first; its contracts bind.
PRD 0001 supplies a green `materialized` control, a `packed-pageable` treatment,
one canonical packed hash implementation, and attributable timing.

Cudarc 0.17.6 exposes page-locked memory as `PinnedHostSlice<T>`, allocated by
`CudaContext::alloc_pinned()`. `CudaStream::memcpy_dtoh()` enqueues D2H work and
the pinned slice synchronizes before host access. This is sufficient to test
whether retained pinned destinations reduce host blocking and allow one lane's
copy to overlap other lanes' kernels. It does not require another CUDA stream or
a device snapshot.

## Goal

Add a third hidden final-state mode, `packed-pinned`, which copies packed state,
inputs and input counts into one retained page-locked buffer set per CUDA sweep
lane, then copies them into retained cacheable staging buffers and computes the
unchanged canonical digest on the CPU.

The treatment is default-off, bounded, reusable across draws, and observable in
timing/capacity evidence. It is not promoted by this PRD.

## Specification

### 1. Retained per-backend pinned buffers

Add an internal buffer owner containing correctly typed/sized pinned
destinations and cacheable staging buffers for packed state bytes, input bytes
and input counts. It belongs to the retained `CudaBackend`, so supported
concurrent sweeps naturally have one owner per lane. Allocate lazily on first
`packed-pinned` use or during explicit treatment preflight; do not page-lock or
reserve the staging memory for the default or `packed-pageable` paths.

Cudarc 0.17.6 allocates `PinnedHostSlice` with
`CU_MEMHOSTALLOC_WRITECOMBINED`; direct CPU SHA over that memory could be much
slower. Therefore C synchronizes the D2H, copies each component into its retained
ordinary cacheable buffer, and hashes the cacheable slices. Time this staging
copy separately. A miss closes this staged cudarc route only, not every possible
pinned-memory implementation.

Size from the actual device slices/layout, not demographic constants. Checked
arithmetic is mandatory. A zero-length component skips pinned allocation, copy
and staging allocation, reports zero bytes/allocations, and supplies an empty
slice to canonical framing. Retain and reuse allocations across resets/draws
when sizes are unchanged. Drop only after pending transfers are synchronized.

Use cudarc's safe pinned abstraction and copy operation. Any unavoidable
`unsafe` call to allocate uninitialized pinned memory must be encapsulated in a
small constructor whose initialization/read-before-write invariant is stated
and tested structurally. Do not introduce a raw CUDA FFI wrapper when cudarc
already provides the operation.

### 2. Correct transfer and hashing order

Enqueue state, input and input-count D2H copies on the lane's existing stream:
the default stream for supported workers 1 and that lane's existing non-blocking
stream for supported workers greater than 1. Do not create or substitute a
stream for this treatment. Do not access a pinned destination until its recorded
work is complete; then copy to cacheable staging and compute the digest through
the same canonical `hash_state` framing used by `packed-pageable`.

A backend reset or next draw may not overwrite device state until the previous
final-state transfer has captured the bytes. A buffer may not be reused while
CPU hashing reads it. Express this through ownership/borrowing and explicit
synchronization; do not rely on timing or worker scheduling.

This PRD may enqueue all three copies before waiting, but it must not add a
second copy stream, double buffering, device-to-device snapshot, background hash
thread, or coordinator pipeline. Those require fresh evidence after this simple
arm is measured.

### 3. Selector and failure behavior

Extend 0001's single hidden selector with `packed-pinned`. Unset remains
`materialized`. Pinned allocation/transfer failure returns a clear error before
publishing the affected draw. There is no pageable fallback, silent worker
reduction or mode change.

The selector is diagnostic-only and CUDA-sweep-only. Ordinary run, CPU sweep,
validation, and historical fused modes must not accidentally allocate treatment
memory. If fused mode cannot satisfy the lane ownership contract, reject the
combination clearly. Cacheable staging allocation uses a fallible reserve path;
its injected failure is reported just like pinned allocation failure, with no
abort, pageable fallback or partial publication.

### 4. Admission and accounting

Expose exact checked pinned- and staging-byte estimates for a backend. The
supported `--draw-workers N` capacity preflight must account for `N × per-lane`
bytes for both categories when `packed-pinned` is selected, in addition to
existing host/device requirements. Report requested/effective bytes and two
named counts: `buffer_set_count <= retained_lane_count` and
`underlying_pinned_allocation_count <= 3 * retained_lane_count`, reduced for
zero-length components.

The first allocation cost is separate from per-draw transfer/hash timing. Counts
must prove one buffer set per retained lane, not one per draw. Do not claim a
universal OS page-lock limit. On allocation failure, report requested bytes and
lane count and exit cleanly.

### 5. Timing semantics

Extend 0001's versioned diagnostic with:

- buffer-set count, underlying pinned-allocation count, pinned bytes, cacheable
  staging bytes and one-time allocation time;
- D2H enqueue/API interval;
- wait-to-pinned-host-readable interval;
- pinned-to-cacheable staging-copy time;
- CPU hash time; and
- total final-state seam time, explicitly excluding one-time allocation.

A measured interval must have one definition across modes. If pageable and
pinned APIs expose different synchronization boundaries, retain separate honest
fields rather than forcing a misleading comparison. Require draw wall time to
reconcile with allocation plus final-state totals within a documented timer
resolution/tolerance. Report exact component bytes for state, inputs and counts,
not only totals. Nsight remains the hardware authority for copy union and kernel
overlap.

## Allowed files

- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/src/lib.rs` — only to re-export a hidden named diagnostic
  result/buffer-accounting type if the CLI needs it
- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cuda/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `docs/prds-cuda-final-state-readback/README.md` (status notes only)

The pinned implementation must not modify generated CUDA source or add a
dependency. If a required gate fails outside this list, stop and report it on
the first attempt. Do not edit `.piprd/**` or hardware collector files; those
belong to 0003.

## Non-goals

- No paid GPU execution or performance claim.
- No promotion/default change or supported public flag.
- No second stream, double buffer, device snapshot, background CPU hash, or
  reset/hash pipeline.
- No digest, serialization, manifest, RNG, kernel or scientific-semantic change.
- No global pinned pool and no cross-backend/lane memory sharing.

## Acceptance criteria

1. `packed-pinned` uses one lazily allocated pinned-plus-cacheable buffer set per
   retained CUDA backend/lane, sized from actual slices with checked arithmetic;
   default and pageable modes allocate zero treatment buffers. Zero-length
   components allocate/copy zero bytes and still hash with canonical framing.
2. Tests prove `buffer_set_count <= retained_lane_count` and
   `underlying_pinned_allocation_count <= 3 * retained_lane_count` across
   multiple draws/resets, reduced for zero-length components, and that pinned
   and staging byte estimators exactly match the three destinations. A fake
   allocation failure produces a clear error with no fallback or publication.
3. Ownership/synchronization prevents host access before D2H completion, device
   reset before capture, and buffer reuse during staging/hash. Drop synchronizes
   pending work. Workers 1 retain the existing default stream; workers greater
   than 1 retain their existing lane stream, with no treatment-created stream.
   These invariants are covered by unit/structural tests and explanatory comments
   at the unsafe boundary.
4. Pinned and pageable packed modes call the same canonical hash framing.
   Available CUDA tests prove digest equality; local tests prove the shared
   framing and mode plumbing without requiring hardware.
5. Supported concurrent mode includes per-lane pinned and cacheable staging
   bytes in capacity preflight and diagnostics. Overflow, zero-length state,
   inputs and counts where representable, excessive worker requests and injected
   allocation failure are tested.
6. The diagnostic separates one-time allocation, enqueue/API, wait,
   pinned-to-cacheable copy, CPU hash and total seam time, reports both allocation
   counts and component bytes, reconciles timers within a documented tolerance,
   and emits lane records in ascending `k` without changing scientific outputs.
7. No new dependency, CUDA kernel/source change, copy stream, snapshot or
   default-path allocation is introduced.
8. Run and pass `cargo fmt --check`, runnable relevant CLI/CUDA tests,
   `cargo test --locked -p sembla-cuda --features cuda --no-run`,
   `cargo test --locked -p sembla-cli --features cuda --no-run`,
   `python3 scripts/check-prd-allowlist.py` on this PRD, Markdown-link checks,
   and the repository's standard local check command. Hardware-gated tests may
   skip execution locally, but CUDA-feature compilation may not be skipped.
   Hardware performance and Nsight evidence remain deferred to 0003.

# PRD 0001: Retire scalar device SHA and measure packed host hashing

> Historical experiment contract: its unset=`materialized` requirement was
> necessary to preserve a real A control during measurement. It is superseded
> for production CUDA sweeps by the B promotion in `DECISIONS.md` §L14.

## Context

Read `docs/prds-cuda-final-state-readback/README.md` first; its contracts bind.

The opt-in device SHA at commit `7478d5b5340febfd881c2136def0d8354e4ed46e`
serially hashes the production 480 MB state in one CUDA thread. Its H100 arm
made steady but unusably slow progress and timed out at 900 seconds. It also
leaves `cargo test --locked -p sembla-cuda` red because the generated hash
kernel's `literals` pointer is not classified by the fused-kernel ABI test.

The ordinary CUDA sweep obtains its final digest through
`ensure_observed_state().state_hash()`, which downloads packed bytes,
reconstructs/refreshes the host `StateStore`, and then hashes. The existing
`CudaBackend::observed_hash()`/`download_hash()` route computes the same
canonical digest directly from downloaded packed state, input and input-count
bytes. This PRD turns that existing route into a hidden, attributable treatment;
it does not promote it.

## Goal

Restore a green no-device-SHA baseline and provide two default-off diagnostic
modes for the sweep final-state seam:

- `materialized`: current full host-state refresh followed by `StateStore` hash;
- `packed-pageable`: existing pageable packed download followed by the same
  canonical host SHA-256 framing.

Both modes expose enough diagnostic timing to distinguish download,
reconstruction and CPU hashing without changing scientific output.

## Specification

### 1. Remove the scalar device-SHA experiment completely

Remove the generated SHA kernel, code-generation insertion, launch handle,
device digest and hash-plan buffers, backend method, CLI enable/verify variables,
and tests that exist only for device execution of SHA-256. Do not merely fix the
pointer-classification test or leave dormant generated code.

Retain or add a regression test proving generated CUDA source contains no
`sembla_final_state_sha256` kernel and that ordinary/fused pointer inventories
are green. Record the timeout as a negative result in the folder README and
`docs/performance/model.md`; do not claim canonical GPU SHA is universally
impossible.

### 2. Add one hidden final-state mode selector

Add a single internal selector with values `materialized` and
`packed-pageable`. Unset means `materialized`. Invalid values fail clearly
before scientific output. The selector is accepted only for CUDA sweep
execution and is not written into the scientific manifest. Prefer one
well-named environment seam suitable for the hardware collector over multiple
booleans; document it in code and tests, not as a supported CLI option.

Every CUDA sweep lane, including sequential and supported `--draw-workers`,
must use the selected seam consistently. Lockstep/fused historical spike modes
must either use it correctly or reject the selector clearly; no implicit mixed
mode.

### 3. Reuse the canonical packed hash implementation

`packed-pageable` must call/refactor the existing `observed_hash()` and
`download_hash()` implementation. At the final-state treatment seam it must
force the unconditional packed download even when `host_state_current` is true;
otherwise a zero-tick or host-observation-fallback model could silently execute
A while reporting B. There is one canonical `hash_state` framing implementation.
Do not create another serializer, hash function or field-order table in the CLI.
"No second verification download" means no extra post-treatment parity readback,
not reuse of an earlier materialized observation.

Add tests over materially different layouts proving the packed path equals
`StateStore::state_hash()`, including state plus model inputs and input counts.
A negative fixture that changes one byte must change the digest. Existing
per-tick/differential uses of `download_hash()` remain unchanged.

### 4. Attribute final-state time

Add diagnostic-only per-draw timing fields, with a versioned schema, for at
least:

- selected mode;
- packed D2H host-API time and completion-wait time, or an explicitly
  documented equivalent available from cudarc;
- host-state reconstruction/refresh time (zero/not-applicable for packed mode);
- CPU SHA-256 time;
- total final-state seam time; and
- downloaded bytes by component and total.

The fields must be carried to the existing sweep concurrency timing sidecar or a
new diagnostic sidecar without entering scientific files. Concurrent lanes
record independent timings and the coordinator emits them in ascending `k`.
Timing instrumentation is always structurally present but must not force an
extra download, hash, synchronization or reconstruction when diagnostics are
off.

For the pageable destination, name the available field
`pageable_dtoh_host_api_ms`; use `completion_wait_ms: null` when cudarc exposes
no separate wait boundary and document why. Do not synthesize a zero wait or
label the pageable call asynchronous overlap.

### 5. Keep the control unchanged

With the selector unset, the materialized path must execute the same calls as
the pre-PRD ordinary path and produce byte-identical output. Do not opportunistically
switch production to `observed_hash()`. The hardware experiment needs a real A
control.

## Allowed files

- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/src/lib.rs` — only to re-export a hidden named diagnostic
  result type if the CLI needs it
- `crates/sembla-cuda/src/codegen.rs` — removal of the failed hash kernel only
- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cuda/tests/**`, `crates/sembla-cli/tests/**` (tests only)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` — only to
  remove the retired device-hash treatment references; the replacement
  collector belongs to 0003
- `spikes/precision/infra-hyperstack/RUNBOOK.md` — retire obsolete device-hash
  instructions only
- `docs/performance/model.md`
- `docs/prds-cuda-final-state-readback/README.md` (status notes only)
- `docs/evidence/**` (new analysis/checksum text only; do not rewrite raw runs)

If a required gate fails outside this list, stop and report it on the first
attempt. Do not edit `.piprd/**` managed state or commit unrelated untracked
evidence.

## Non-goals

- No pinned memory; that is 0002.
- No paid GPU execution or hardware performance claim.
- No new CUDA SHA, tree hash, digest contract, dependency or kernel work.
- No supported user-facing flag and no default-path optimization.
- No changes to sweep scheduling, worker admission, publication or RNG.

## Acceptance criteria

1. All scalar device-SHA code, generated source, buffers, selectors and
   experiment-only tests are removed; a regression test proves the kernel is
   absent.
2. `cargo test --locked -p sembla-cuda` passes, including the previously failing
   fused pointer-classification test.
3. Unset mode executes `materialized`; `packed-pageable` is CUDA-sweep-only.
   Tests cover CPU sweep, ordinary CPU/CUDA run, fused mode, invalid UTF-8/value,
   and conflicting retired variables; every rejection occurs before output
   directory creation.
4. Packed hashing reuses one canonical framing function and is test-equal to
   `StateStore::state_hash()` across at least two materially different state
   layouts including inputs/counts; a one-byte negative control differs.
5. Sequential and concurrent sweep plumbing returns final digests and timing in
   ascending `k` without a second verification download. Existing complete
   output-tree/golden tests remain byte-identical with the selector unset.
6. The versioned diagnostic records the selected mode, total bytes, final-state
   total, CPU hash, reconstruction, and the honestly available D2H host timing.
   Packed mode reports reconstruction as zero/not-applicable.
7. The performance model records the scalar-kernel timeout, its limited scope,
   and the replacement A/B/C plan; stale text no longer names the 2026-07-28
   phase split as the current post-control-count CUDA constraint.
8. Run and pass `cargo fmt --check`, runnable relevant CLI/CUDA tests,
   `cargo test --locked -p sembla-cuda --features cuda --no-run`,
   `cargo test --locked -p sembla-cli --features cuda --no-run`,
   `python3 scripts/check-prd-allowlist.py` on this PRD, Markdown-link checks,
   and the repository's standard local check command. Hardware-gated tests may
   skip execution locally, but CUDA-feature compilation may not be skipped.
   Hardware evidence is explicitly deferred to 0003's operator-run collector.

# CUDA final-state readback PRDs

Replace the failed scalar device-SHA experiment with a bounded measurement of
three exact final-state paths. Run from the Sembla repository with:

```text
/piprd run docs/prds-cuda-final-state-readback
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the contracts below are binding.

## Why this folder exists

The supported concurrent CUDA sweep downloads one packed final state per draw
and computes `final_state_sha256` on the host. On the 10M-slot demographic
shape that download is about 480 MB. The completed H100 diagnostic at
`docs/evidence/demographic-bench/hyperstack-l4-20260730T072627Z/` measured four
large pageable copies at 978.416 ms, of which 906.710 ms was outside kernels.
At 20 draws/four workers that projects to 4.535 s of exposed D2H time, roughly a
19–21% whole-sweep ceiling against the accepted and latest baselines.

Commit `7478d5b5340febfd881c2136def0d8354e4ed46e` tested a device-side canonical
SHA-256. That implementation launches one CUDA thread over the complete byte
stream. The H100 arm timed out after 900 seconds with only 15 of 20 worker-one
draws published; completed draws advanced at roughly 56–57 seconds each. The
branch also fails the generated-kernel pointer-classification test. This is a
negative scalar-kernel result, not evidence against every possible GPU hash,
but it closes that implementation and this folder does not replace it with a
different digest or tree hash.

The sweep publishes only the digest, not a final-state artifact. The CUDA
backend already has an exact packed-host route: `observed_hash()` calls
`download_hash()`, which hashes downloaded packed state/input bytes without
reconstructing a `StateStore`. Cudarc 0.17.6 also exposes
`CudaContext::alloc_pinned()` and `CudaStream::memcpy_dtoh()` for retained
page-locked destinations. Those are the two treatments this folder measures.

## Ordered plan

1. `0001` removes the scalar device-SHA arm, restores a green baseline, and
   adds a default-off packed-host-hash treatment with attributable timing.
2. `0002` adds a default-off reusable pinned packed-host-hash treatment, one
   bounded buffer set per retained CUDA lane.
3. `0003` adds the focused A/B/C collector, preflight, analyzer, and runbook.
   It prepares hardware evidence but does not provision paid infrastructure.

The runner executes these serially. Do not promote a treatment, add async copy
streams, or alter the default path inside this folder. A later PRD may promote a
winner only after the hardware gate below passes.

## Binding contracts

- **Canonical identity is frozen.** `final_state_sha256`, complete sweep output
  trees, manifests, grouped sidecars, exported pairs, and all existing goldens
  remain byte-identical. Hash framing, field order, byte order, and SHA-256 are
  unchanged.
- **A/B/C differ only at the final-state seam.** A is the current materialized
  host path; B hashes pageable packed downloads; C downloads into reusable
  pinned buffers, copies into retained cacheable staging buffers, and hashes
  those bytes. Kernels, tick scheduling, RNG, observations, draw admission,
  seeds, and ascending-`k` publication are identical.
- **Default behavior remains A.** Experimental selectors are hidden,
  default-off, rejected on non-CUDA paths, and fail clearly on invalid or
  conflicting values. They do not enter scientific manifests.
- **No silent fallback.** If pinned allocation or transfer cannot be used, C
  fails before publishing that draw; it never falls back to pageable memory.
- **One retained buffer set per lane.** Buffers are sized from the actual packed
  state/input/count layouts, retained across that lane's draws, synchronized
  before CPU hashing or reuse, and released with the backend. One set contains
  up to three underlying pinned allocations plus three cacheable staging
  buffers; zero-length components allocate neither. No global pool and no
  cross-lane borrowing.
- **Bounded admission includes pinned and staging memory.** Requested pinned
  and cacheable staging bytes are explicit in diagnostics and capacity checks.
  Four 10M lanes may page-lock roughly 1.9 GB plus retain comparable cacheable
  staging capacity; neither cost can be hidden or allocated per draw.
- **No new digest, tree hash, final-state artifact, dependency, CUDA kernel, or
  generated kernel ABI.** No double-buffered device snapshots, extra copy
  stream, CUDA Graph, or overlap pipeline is in scope.
- **Measurement is adjacent and attributable.** Never compare the failed
  session's control to a later treatment as a speedup. A/B/C use one binary,
  one VM/session, adjacent arms, the same model, draw set, noise mode and worker
  count. Whole-sweep wall time is the headline; phase timing explains it.

## Local versus paid-hardware acceptance

All numbered PRDs must pass locally without a CUDA device: selectors, hash
framing, timing schema, lifetime/admission logic, collector validation, shell
syntax, tests, formatting and repository checks. GPU-specific tests may be
compiled or structurally checked where the development host cannot execute
them.

No numbered PRD may create a Hyperstack VM, firewall rule, or other paid
resource. After `/piprd run` finishes and its commits are reviewed, the operator
must separately approve the paid H100 plan under
`spikes/precision/infra-hyperstack/RUNBOOK.md`. The focused collector must then
tear down the VM and SSH rule even on failure and run orphan reconciliation.

## Hardware decision gate

The collector first runs one 10M draw for A, B and C. Hash/output parity and all
diagnostic fields must pass before any multi-draw arm. It then runs the bounded
four-draw workers-1/workers-4 matrix with adjacent repeated arms; it does not
launch the old 20-draw matrix automatically.

Eligibility uses the **median of the three adjacent within-repetition wall
ratios**, calculated independently for workers 1 and workers 4. Workers 4 at
10M is the binding performance aggregate. B compares only with adjacent A; C
compares only with adjacent B. Individual repetitions are reported but do not
alone trigger a veto.

A treatment is only eligible for a promotion PRD when:

- every compared complete output tree is byte-identical and a deliberate
  negative control is rejected;
- its workers-4 binding aggregate improves by at least 5% (median B/A ratio <=
  0.95 for B; median C/B ratio <= 0.95 for C);
- its workers-1 aggregate does not regress by more than 2% (ratio <= 1.02), and
  neither treatment-specific aggregate is malformed or incomplete;
- the phase evidence agrees with the claimed mechanism; and
- C reports bounded pinned/staging bytes and passes at least one frozen mechanism
  ratio: `C_host_blocking_ms / B_host_blocking_ms <= 0.90`, where
  `B_host_blocking_ms = pageable_dtoh_host_api_ms` and `C_host_blocking_ms =
  pinned_dtoh_enqueue_api_ms + wait_to_pinned_host_readable_ms`; or
  `C_exposed_final_state_dtoh_ms / B_exposed_final_state_dtoh_ms <= 0.90` from
  Nsight. Staging-copy time is excluded from these D2H mechanism ratios but
  remains included in final-seam and whole-sweep time.

These values and comparison directions are frozen before hardware execution
and must appear verbatim as machine-readable threshold objects. If neither
treatment passes, close this path and next decompose the measured 3.345 s sweep
setup aggregate. Do not rescue a miss by changing the threshold after observing
it.

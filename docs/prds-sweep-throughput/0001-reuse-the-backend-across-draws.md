# PRD 0001: Build the backend once per sweep, not once per draw

## Context

Read `docs/prds-sweep-throughput/README.md` first; its constraints bind,
including the draw-independence proof obligation.

`sweep_file_result` loops over draws (`crates/sembla-cli/src/main.rs:1809`) and
for each one calls `execute_backend_output_with_features` (`main.rs:2611`),
which constructs a fresh backend. On CUDA that means `CudaBackend::new`
recompiles the whole model through NVRTC **every draw**. The same loop also
does a full host copy of the initial state per draw:

```rust
let initial = initial_tables.clone();   // main.rs:1828
```

about 458 MiB at 10M slots.

Neither result differs across draws. The model is the same, so the compiled
kernels are the same; the initial state is the same, so the copy is the same
bytes every time.

Measured context: after `prds-device-observation` (§L11), a 1M-slot 24-tick run
costs roughly 3.1 s of which about **2.2 s is startup**. A hundred draws pay
that hundred times.

## Goal

A sweep compiles the model once and holds one backend across all draws, with
per-draw state reset. Every output byte-identical.

## Specification

### 1. One backend per sweep

Construct the backend once, before the draw loop, and reuse it. The natural
seam is `execute_backend_output_with_features`, whose current contract is
"construct, run, return" — this PRD changes that contract, and the change
crosses `main.rs` and the backend crates.

Both backends benefit; CPU has no JIT but still rebuilds per draw. **Do both, or
say why not.**

### 2. Per-draw reset must be complete, and this is the criterion

A retained backend introduces exactly one hazard: state leaking from draw *n*
into draw *n+1*. Everything a draw mutates must be restored:

- committed state and inputs,
- parameters, which change per draw,
- the seed, which changes per draw under `NoiseMode::Independent`,
- any scratch that carries a value across ticks — validation status, `wins`,
  `deferred`, grouped histograms and band extrema, double-write scratch.

**Enumerate the per-draw mutable surface in the implementation notes** and state,
for each item, how it is reset. An omission here is silent and produces wrong
science rather than a crash, which is why it is the acceptance criterion rather
than a note.

Reset should not reallocate. `prds-cuda-host-path/0001` built an in-place
`StateStore` refresh with full constructor validation for exactly this reason;
prefer it over a second mechanism. On device, a device-to-device copy from a
retained pristine buffer is preferable to re-uploading from the host.

### 3. Seeding must be explicit

`CudaBackend::new` takes the seed at construction. Under `NoiseMode::Crn` every
draw shares one seed, so reuse is trivially correct. Under `Independent`, each
draw derives its own via `derive_sweep_replica_seed`.

A retained backend therefore needs the seed to be settable per draw, and the
draws it produces must be **identical to those a freshly-constructed backend
would produce for that seed**. Prove it with a test comparing both paths, not
by inspection.

### 4. Preserve what the identity check was doing

The loop asserts backend device identity is stable across draws
(`main.rs:1843`). With one retained backend that becomes trivially true, so the
check stops doing work.

It was guarding against a sweep silently spanning two devices. State what
replaces it — capturing identity once at construction and recording it in the
manifest is a reasonable answer; silently dropping the check is not.

### 5. Draw independence must be demonstrated

A test must show that **draw *k* run alone is byte-identical to draw *k* run
after *k−1* preceding draws**, for at least one model with contests and one with
grouped observations, under both noise modes.

This is the property the whole PRD rests on. It is cheap to test and impossible
to argue convincingly without a test.

### 6. Measure the sweep, not the tick

Report whole-sweep wall time before and after, at a scale where startup
dominates — 1M slots, 24 ticks, at least 20 draws — and separately report draw 0
against the median draw. The gap between them is the setup this PRD removes, and
it should appear only in draw 0 afterwards.

Also report a large-state case (10M) so the removed `initial_tables.clone()`
shows up distinctly from the removed JIT.

## Allowed files

- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-runtime/src/state.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-cli/tests/**`, `crates/sembla-runtime/tests/**`,
  `crates/sembla-cuda/tests/**` (tests only)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` — **added
  2026-07-28 by operator authorisation**, see below
- `spikes/precision/infra-hyperstack/RUNBOOK.md` — same authorisation; a new
  collector stage that is not documented is a stage nobody will run, which is
  the §M3 failure this PRD is partly a response to
- `docs/evidence/**` (new evidence only)
- `docs/prds-sweep-throughput/README.md` (status notes only)

### Why the collector is in scope (authorised 2026-07-28)

**The original list omitted it and that made this PRD unachievable as written.**
Criterion 11 requires whole-sweep CUDA timings, §5 of this section says building
the collector stage is in scope, and §M3 forbids deferring a criterion nothing
can execute — yet the only file that could carry that stage,
`spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`, was not
listed. The PRD therefore demanded work it forbade, and the runner was right to
stop rather than guess. Five attempts were spent on it.

**The exception is limited to adding a sweep stage** — a new opt-in block
guarded by its own environment variable, in the shape of the existing
`BENCH_PROFILE` and `BENCH_CORPUS` stages, plus its documentation. It does
**not** extend to changing the frozen §L4 gate protocol, the existing profile or
corpus stages, teardown, or anything in the collector's Terraform handling. The
frozen collection must remain byte-for-byte the same protocol it is today, and
`BENCH_PROFILE`/`BENCH_CORPUS` behaviour must be unchanged.

### Deliberately excluded

`crates/sembla-cli/src/manifest.rs` is **not** in scope. Criterion 6 requires
every manifest byte-identical, so the serialised schema must not move.
`BackendIdentity` capture may relocate within `main.rs`; the manifest field it
populates stays as it is.

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2. If the list still makes the PRD unachievable, say so and
stop rather than working around it. **This has now happened five times**, and
each time the runner stopping was correct and the operator's list was wrong.
Report it immediately rather than at attempt five: the operator can amend in
minutes, and four of those attempts bought nothing.

## Non-goals

**No concurrency across draws.** Running several draws at once is a separate
question that needs its own measurement, and mixing it in would make this PRD's
result unattributable. Sequential draws only.

No change to which draws are taken, to parameter sampling, to §E2 levels, or to
CRN semantics. No kernel changes. No change to single-run `run` behaviour beyond
what the ownership change forces. No evaluator optimisation — that is
`prds-evaluator-throughput`. No new dependencies.

## Acceptance criteria

**Local (required for approval):**

1. One backend is constructed per sweep, asserted by a test rather than by
   inspection.
2. The per-draw mutable surface is enumerated in the implementation notes with a
   reset stated for each item.
3. **Draw independence**: draw *k* alone is byte-identical to draw *k* after
   *k−1* others, covering contests, grouped observations, and both noise modes.
4. Re-seeding produces draws identical to a freshly-constructed backend for the
   same seed.
5. The backend-identity guarantee is preserved or explicitly replaced, and the
   replacement is recorded.
6. **Every golden byte-identical**, including every run and sweep manifest and
   `final_state_sha256`. `git diff --stat` shows none of them.
7. `cargo test --locked` and `scripts/check-rust.sh` green; CUDA-feature
   `cargo check`/`clippy` green without claiming GPU execution.
8. Whole-sweep before/after on CPU at 1M × 24 ticks × ≥20 draws, with draw 0
   against the median draw.
9. `python3 scripts/check-markdown-links.py` passes.

**Hardware (§J14.2, and per §M3 the command must exist):**

10. `cargo build --release --features cuda` on a GPU host.
11. Whole-sweep CUDA before/after at 1M and 10M, draw 0 against median draw.
12. CPU/CUDA sweep outputs byte-identical, compared **within the new collector
    stage** — run the same sweep on both backends and compare the whole output
    directory, including the `.grouped.<view>.csv` sidecars and the summaries,
    not only the top-level results CSV.

**Local criteria 1–9 are sufficient for approval.** Criteria 10–12 are §J14.2
hardware criteria and are listed as pending, executed in a later GPU session.
Do not present a local result as GPU evidence, and do not block approval on
lacking a GPU.

**What §M3 requires here, precisely.** The *stage* that will run criteria 10–12
must be built and committed by this PRD — that is why the collector is in the
allowed files. **Running it needs a paid GPU host and is not part of this PRD.**
So the deliverable is a working, documented, opt-in collector stage plus the
criteria marked pending; it is not a set of GPU numbers.

Criterion 12's file-set comparison is written the way it is because of §M4: the
first version of the grouped parity check compared two files that contain no
grouped data and would have passed unconditionally. Compare the whole directory
and require the file *sets* to match, then demonstrate the check failing on a
perturbed copy before trusting it.

## Note on expectations

The projection is that a hundred 1M draws fall from ~310 s to ~11 s once this
and the `wins`/`deferred` reduction have both landed. This PRD alone, without
that reduction, should take them from ~310 s to roughly 100 s — the per-draw
simulation cost remains, only the setup goes.

**Both figures are projections from a 5M/2-tick profile and neither is
measured.** §L11 records that linear extrapolation from that profile
underpredicted the 10M case by 11%, and increasingly so with scale. Treat them
as the reason to do the work, not as its success criterion. The success
criterion is whole-sweep wall time measured before and after.

If draw 0 and the median draw do not separate cleanly afterwards, something is
still being rebuilt per draw and §2's enumeration is incomplete.

# CUDA validation-parallelism PRDs

Ordered PRD set that makes the CUDA backend usable for models with contests and
`Ref` dereferences. Run from the Sembla repository with:

```text
/piprd run docs/prds-cuda-validation-parallelism
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this README,
this README wins.

## The measured problem

On 2026-07-25, on one Hyperstack host (NVIDIA H100 PCIe, AMD EPYC 9554, CUDA
12.8, driver 570.195.03), the `demographic_slots` no-grouped model at 10,000,000
slots for 24 ticks measured:

| Backend | Wall time | s/tick |
|---|---:|---:|
| CUDA (H100 PCIe) | 1h 30m 08s | 225.3 |
| CPU (EPYC 9554, same host) | 7m 19s | 18.3 |

**The GPU is 12.3× slower than the CPU beside it**, on the same binary, commit,
seed, and state artifact, differing only in `--backend`. Evidence:
`docs/evidence/demographic-bench/hyperstack-cuda-10m-20260725/` and
`docs/evidence/demographic-bench/hyperstack-cpu-10m-20260725/`.

**Evidence status correction (2026-07-25).** The checked-in CUDA arm records
commit `f81fef9d90ad`; the CPU arm records `8857cb839220`. No `crates/**`, Cargo
manifest/lockfile, or fixture files changed between those commits, and the
benchmark-script delta affects only the CUDA zero-tick export step. The stored
pair therefore supports the diagnostic 12.3× baseline but does not substantiate
the same-commit, same-artifact sentence above and is not §L4 gate evidence. The
frozen §L4 protocol remains unchanged.

For contrast, ADR 0001 measured ~1,380 ticks/sec for SIR at 26M rows on the same
GPU class. SIR declares no contests and dereferences no `Ref`, so it generates
none of the code below.

## The cause

`crates/sembla-cuda/src/codegen.rs` emits four kernels whose bodies open with

```c
if (blockIdx.x != 0 || threadIdx.x != 0 || status[0] != 0ULL) return;
```

and which then contain `for (unsigned long long row = 0; row < row_counts[...];
++row)`:

- `sembla_validate_claims`
- `sembla_validate_transition`
- `sembla_validate_effects`
- `sembla_validate_outputs`

One GPU thread therefore walks every row, once per claim and per fallible
expression, every tick. A single GPU thread has no instruction-level parallelism
and no latency hiding, so it is far slower at this than one CPU core. The
simulation kernels themselves (`resolve_conflicts`, `apply_effects`,
`build_aggregate_partials`, `finish_aggregates`, `build_output_partials`,
`finish_outputs`) are already properly parallel. **The defect is in validation,
not in execution.**

The loops are emitted conditionally, per claim and per expression that can fail,
which is why the cost appears exactly for the model class the forward roadmap is
built on: slot claiming, generation-safe references, household links, and
identity-preserving migration all dereference `Ref` or contest a resource.

## Profile before scoping (binding)

**No PRD in this folder may scope a performance change without a kernel profile
first.** Two PRDs were scoped from emitted-source structure — 0002 and the first
draft of 0006 — and both selected the wrong kernels. An `nsys` profile settled it
in fifteen minutes for about one dollar.

Structural reasoning about which kernel dominates is a hypothesis. `nsys stats
--report cuda_gpu_kern_sum` is a finding. `ncu` is available on the CUDA image
under `/usr/local/cuda*/bin` for occupancy and stall analysis when a kernel is
already parallel but slow.

Profile at the scale you intend to fix: cost here is superlinear in rows (2.7
s/tick at 500k against 235.3 s/tick at 10M), so a small-scale ranking may not
hold at the target scale.

## Authority and scope

- `DESIGN.md` (§4.2 kernel fragment, §5.2 precision, §8 two execution paths),
  `DECISIONS.md` (§E2 determinism levels, §E3 conflict resolution, §J14 GPU
  evidence discipline, §K6 grouped observations), and
  [ADR 0001](../decisions/0001-gpu-precision.md) all bind.
- **Validation stays a separate pass.** Fusing per-row validation into the
  execution kernels is rejected for this track: it entangles two concerns that
  are currently independent and easy to reason about, for a speedup that is not
  needed to clear the bar below. Recorded as a decision, not left implicit.
- **No new model semantics.** This folder changes how existing checks execute,
  never what they mean, when they run, or what they accept.

## Frozen and untouchable

`examples/**`, every CSV/hash golden, the CUDA differential evidence, plan and
source schemas, version strings, the `SEMBLA_POP` and `sembla.state/v1` formats,
Philox layout, and the conflict argmin semantics. A diff to any of them is a
failed PRD.

## Determinism and diagnostics are the hard constraint

The current code reports the **first failing candidate in row order**
(`status[1] = candidate`). Diagnostics are part of the observable contract, not
an implementation detail: the CPU oracle is ground truth and the differential
harness compares contract diagnostics, so a parallel implementation that reports
*a* failing candidate instead of *the same* one is a regression even when it is
"more correct". Parallel reduction to the minimum failing index is required, and
must be demonstrated by test rather than asserted.

Determinism Level A (bitwise, same binary and GPU model) is unchanged. Nothing
here may make results depend on block count, launch configuration, or scheduling
order.

## Deferred, with triggers

- **CUDA support for grouped observations** (§K9, follow-up folder). This track
  must not become that one; the demographic model enters the differential corpus
  in its no-grouped configuration only.
- **Hoisting validation out of the per-tick path.** Tempting, and the largest
  possible win, but it requires proving the checked property cannot change
  within a tick — and Track R deliberately makes `Ref` mutable. Trigger: an
  accepted proof obligation, not a performance argument.
- **`Expr::Tick` / derived age.** Separate §K2 trigger. Note that the ageing
  cost share measured 11.6% (M2 Pro) and 12.2% (EPYC) at 10M, both above the
  10% threshold, on unreplicated single runs. That is this track's business only
  insofar as its benchmark should report the share.

## PRDs

- `0001-decision-record-and-baseline` — record the finding and the three design
  decisions in `DECISIONS.md`; freeze the benchmark case and the numeric gate.
  Documentation only.
- `0002-parallel-validation-kernels` — grid-stride the four row loops with
  deterministic minimum-index failure reporting.
- `0003-diagnostic-equality-tests` — negative models must report identical
  status codes and candidate indices on CPU and CUDA.
- `0004-differential-corpus-membership` — existing corpus byte-unchanged; the
  demographic no-grouped model joins it, closing the coverage gap that let this
  regression pass every differential test.
- `0005-measure-and-publish` — re-run the frozen case, commit evidence, correct
  `docs/demographic-benchmark.md` and the forward roadmap's scale note.
- `0006-segmented-argmin-conflict-resolution` — **the decisive one.** Profiling
  at 500k/2M/5M showed `resolve_conflicts` is quadratic (exponent 1.96) and
  77.9% of GPU time at 5M. Replaces the nested scan with the segmented argmin
  the CPU oracle already uses. First PRD here that can change *results*, so
  byte-identical goldens are the guard.
- `0007-linear-serial-kernels` — the two remaining single-threaded kernels,
  which scale linearly and are secondary. Runs after 0006.
- `0008-remove-the-validation-spin-lock` — **corrects a defect in 0002.** Its
  generated spin lock deadlocks on hardware at launch geometry `1x32`, one full
  warp contending, while passing at `1x1`. Found 2026-07-28 on the first
  hardware execution of code approved on local criteria three sessions earlier.
  See `DECISIONS.md` §L12 and
  [`docs/evidence/cuda-validation-deadlock-20260728/`](../evidence/cuda-validation-deadlock-20260728/).

## A note this folder should carry

0002 was correct by every criterion available when it was approved. Hosted CI
has no GPU, §J14.2 permitted approval on local criteria, and its hardware
criteria were properly listed as pending. What was missing was any *mechanism*
that would later execute them — `run-differential-corpus.sh` existed but no
collector invoked it — so the defect survived three GPU sessions before anything
ran it, and then failed on the first case of the first contending geometry.

`DECISIONS.md` §M3 now requires a deferred hardware criterion to name a runnable
command, and `BENCH_CORPUS=1` is that command for this folder.

## Acceptance notes

Per §J14.2, PRDs split acceptance into **local criteria** (must pass without a
GPU: compilation, codegen text assertions, corpus listing, graceful skips,
legacy goldens unchanged) and **hardware criteria** (executed on a rented GPU
and recorded as evidence). Codegen changes are testable locally by asserting on
the emitted CUDA source; only the timing gate needs hardware.

## Status notes

### 2026-07-25 — PRD 0002 implementation (attempt 2)

Implemented the parallel validation protocol: the four target kernels
grid-stride their row loops and report failures through a scratch-and-commit
reduction (`status[4..=8]` scratch, `sembla_record_validation_failure` under a
short lock with `atomicMin` on the candidate, single-thread
`sembla_commit_validation_status` publishing candidate before code after every
target launch). Diagnostics reproduce the serial CPU validator's first failure
as the minimum of (emission-order scan, candidate, per-row branch); committed
`status[0..=3]` layout, codes, and `device_status()` messages are unchanged.

Three PRD clarifications were resolved during implementation and are recorded
here per the review/advisor process:

1. **Ordered output fold (narrow exception to "replace each row loop").**
   `sembla_validate_outputs` grid-strides the independent per-row filter and
   value checks, but the Int-typed ordered checked-addition prefix fold stays
   on worker 0. Checked addition is order-sensitive, so strided partial sums
   could both miss real overflow and report false overflow; a parallel
   prefix-summary algorithm is deferred as substantially larger than this PRD.
2. **Effect liveness prepass.** `sembla_validate_effects` validates a
   transition's full column only when that transition has a winner. A new
   parallel `sembla_mark_effect_active` kernel reduces `wins` into a per-rule
   flag after conflict resolution (cleared per tick by
   `sembla_init_validation_scratch`), replacing the serial `any_winner` rescan;
   workers all observe the same stable flag.
3. **`sir.generated.cu` fixture regenerated.** Intentional codegen changes
   necessarily change the exact-source fixture; it was regenerated with the
   canonical `examples/generate_sir_golden.rs` and the diff reviewed line by
   line (only the intended emission changes appear). CSV/hash goldens,
   `examples/**`, and `docs/evidence/**` remain byte-unchanged.

Unrelated pre-existing repair: `tests/gpu_philox.rs` referenced a renamed
`PhiloxCoordinate` field (`rule_id` → `rule_word`) and did not compile under
`--features cuda`; fixed in place so the criterion-6 suite builds.

Follow-up (attempt 3): the three emission-ordering assertions in
`tests/gpu_semantics.rs` (`semantic_gpu_fixtures_validate_without_a_device`)
were re-literalized from `if (aggregate_facts[0]` to `aggregate_facts[0] != 0U`
to match the worker-guarded scalar-check emission; the ordering claims they
protect are unchanged and the full `--features cuda` suite is green.

Hardware criteria 7–8 remain **pending** per §J14.2; criterion 8 is covered
locally by the host-side reduction tests in
`crates/sembla-cuda/tests/codegen_validation_parallelism.rs`.

### 2026-07-25 — PRD 0003 implementation revision

Added a four-case validation-diagnostic corpus with deterministic eight-row
state and bad rows `[2, 5, 7]`. Plain CPU tests assert the semantic failure and
earliest row; normalized expected CUDA diagnostics are `(10,2)` for claim-key
overflow, `(3,2)` for transition overflow, `(5,2)` for effect overflow, and
`(9,1)` for the existing output-field identity contract.

The original PRD wording was corrected after implementation analysis found that
validated state cannot contain an out-of-range stored Ref, `validate_claims`
does not define that class, CPU `TickError` does not emit CUDA status words, and
output code 9 identifies a target field rather than a source row. No new
validation class or diagnostic meaning was introduced.

PRD 0002 was narrowly reopened for a private test-only backend seam. A named
ignored hardware unit test downloads raw status after expected rejection and
runs every case under `1x1`, `1x32`, and `3x4`; production continues to use
`LaunchConfig::for_num_elems`. The differential runner lists the corpus locally,
skips GPU-less hardware criteria with a named reason, and remains strict when
`SEMBLA_REQUIRE_CUDA=1`. Hardware criteria 6–7 remain **pending** per §J14.2.

### 2026-07-25 — PRD 0004 implementation

Admitted `fixtures/demographic/benchmark/demographic_slots.no-grouped.json`
directly to the CUDA differential runner at the reduced corpus contract of
numeric population 1,000, seed 7, and 20 ticks. The dedicated invocation records exact
state-hash, results-CSV, and summaries-CSV equality in
`demographic-corpus.log`; `--all-examples` and `--all-plan-fixtures` retain
their existing meanings.

Local tests freeze the listed demographic contract and verify that its model
has no grouped views while exercising both contests and `Ref` attributes. A
GPU-less CLI regression test executes the all-plan selector, requires rejection,
and asserts the exact diagnostic naming `--enable grouped-observations`; a
separate ignored hardware test runs every CUDA-supported plan fixture.
Existing examples, plan fixtures, goldens, and tracked CUDA evidence are
byte-unchanged. Hardware criterion 5 remains **pending** per §J14.2.

### 2026-07-26 — PRD 0005 executed with a recorded deviation

The frozen §L protocol requires three replicates per backend. It was stopped
after replicate 1 (CUDA `5647.9s`, CPU `434.6s`, 13.0x slower) because the
magnitude made further replicates uninformative and each CUDA replicate costs
~90 minutes of rented GPU. **PRD 0005's acceptance criteria are therefore not
fully met**, and `DECISIONS.md` §L6 records the result as a measured verdict
rather than §L4 gate evidence.

Its §4 documentation obligations are complete: `docs/demographic-benchmark.md`
and `docs/forward-roadmap.md` both carry the corrected conclusion, and §5's
ageing share is reported as unresolved rather than decided.

The §L4 gate itself is re-run by PRD 0006 under the unchanged protocol, at which
point three replicates are affordable because the run should no longer take 90
minutes each. That is the intended point to satisfy 0005's numeric criteria, not
a re-run of 0005.

**Closed 2026-07-27.** Three replicates per backend were collected under the
unchanged protocol and the §L4 gate is **met at 4.207×** — CUDA median
`31.82s`, CPU median `133.86s`, all seven collector assertions passing
(`docs/evidence/demographic-bench/hyperstack-l4-20260726T140326Z/`,
`DECISIONS.md` §L8). 0005's numeric criteria are satisfied by that run.

Note what did *not* cause it: no GPU code changed after PRD 0007. The gate
passed because `docs/prds-host-evaluator-performance` removed host-side work
that the CUDA path was paying on top of its device work. This folder's own
conclusion — that further CUDA kernel optimisation cannot pay, at 0.56% of wall
time — still stands.

### 2026-07-26 — PRD 0006 local implementation

Replaced the per-candidate all-row scan with a materialized flat
`(candidate, claim)` list and direct-address resource segments. Four ordered
atomic-min passes select the lexicographic `(ordering key, rule word, entity ID,
stable instance)` minimum; every pass restricts to the prefix fixed by earlier
passes. Candidate finalization reads only its statically bounded own claims.
The floating-point key encoding is tested pairwise against Rust
`f64::total_cmp`, including signed zero, subnormals, infinities, and positive and
negative NaNs.

The frozen demographic model has five contested transitions over the slot
table, so 10M slots materialize 50M claim instances. Their four SoA fields use
1.2 GB before the separate per-resource winner arrays, within the 80 GB target
H100 but substantially above a one-claim estimate. Local source guards prohibit
cross-table row scans, a fixture distinguishes lexicographic from component-wise
minima, and an ignored CUDA test compares the CPU winner under `1x1`, `1x32`,
and `3x4` conflict launch geometries. Existing CSV/hash/example goldens remain
byte-identical; only the canonical generated CUDA source fixture changes.
Hardware criteria 5–7 remain **pending** per §J14.2.

### 2026-07-26 — PRD 0007 local implementation

`sembla_check_candidate_errors` now grid-strides both candidate-error columns
and records guard/hazard failures through the existing ordered validation
reduction before the host commits status. Effect-owner initialization is a
separate grid-stride kernel, and `sembla_prepare_effects` is launched once per
rule with a grid-stride winner-row loop. Stream-ordered rule launches preserve
transition/effect write order without racing owner cells.

The PRD 0002 reduction now carries the complete selected diagnostic payload so
double-write status fields remain paired with the minimum semantic failure key;
status codes and `device_status()` messages are unchanged. A generated-source
guard detects candidate-, row-, owner-, and metadata-indexed bulk loops under a
single-worker guard and names every retained exception with its reason. The
canonical generated CUDA source fixture changes, while examples, CSV/hash
goldens, and tracked differential evidence remain byte-identical. Hardware
criterion 6 remains **pending** per §J14.2.

### 2026-07-28 — PRD 0008 local correctness implementation

Removed the validation mutex and replaced it with four stream-ordered passes:
`atomicMin` of scan, identity within the winning scan, branch within the winning
prefix, then exact-key payload recovery. The kernel boundary between passes is
the global synchronization point; the final single-thread kernel publishes the
paired payload and resets scratch. `sembla_prepare_effects` writes owners only
on the first pass and replays genuine double-write facts without serializing the
PRD 0007 kernel.

The negative corpus now includes multi-block, multi-warp geometry `4x128`, and
the runner's `--list` output is tested directly against the shared geometry
constant. The ignored GPU lib test executes in a killable child with a
120-second internal deadline. The checked-in generated SIR fixture and every
existing CSV/hash, manifest, state, and differential evidence file remain
byte-identical. Local evidence and the exact equivalence argument are in
[`cuda-validation-lock-free-20260728`](../evidence/cuda-validation-lock-free-20260728/).
Hardware criteria 8–10 remain **pending** under §J14.2 and are runnable through
`BENCH_CORPUS=1 bash run-demographic-benchmark.sh`.

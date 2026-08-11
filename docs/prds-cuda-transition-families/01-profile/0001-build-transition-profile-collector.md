# PRD 0001: Build the CUDA transition-profile collector

max_review_cycles: 3

## Context

Read the parent track [README](../README.md) first. Its measurement protocol,
comparison contract, semantic invariants, and existing-work exclusions bind.

The committed Australian H100 run records aggregate throughput but no Nsight or
per-phase evidence. This PRD builds measurement infrastructure only. It neither
claims a bottleneck nor implements a candidate optimization.

`DECISIONS.md` §M1 requires a direct same-result spike before any production
performance PRD. This collector will first identify which independent spikes are
worth measuring; it does not authorize them itself.

## Goal

Add a deterministic, locally testable collector and structural accounting report
for the exact Australian workload plus comparison shapes, ready for PRD 0002 to
run on H100 without changing ordinary simulation behavior or scientific output.

## Specification

### 1. Add one reproducible collector

Add `scripts/run-cuda-transition-family-benchmark.py` with a `baseline`
subcommand. It must:

- fail on a dirty tree unless an explicit evidence-only override records the
  complete diff hash;
- build one locked release CUDA binary and record its SHA-256 when not in dry-run
  mode;
- require an explicit output directory;
- record commands, environment, exit status, stdout, stderr, elapsed wall, and
  checksums for every arm;
- expose a local dry-run/fixture mode that tests command construction, parsers,
  formulas, and failure handling without claiming GPU execution;
- treat a missing profiler/GPU, failed differential, incomplete artifact, failed
  checksum, or malformed profile as failure; and
- never modify tracked model/state/parameter fixtures or scientific evidence.

Add standard-library tests in
`scripts/tests/test_run_cuda_transition_family_benchmark.py`. Add no Python or
Rust dependency.

The existing Hyperstack collector may gain one opt-in stage selected by
`BENCH_CUDA_TRANSITION_FAMILIES=1` and
`CUDA_TRANSITION_FAMILY_STAGE=baseline`. Default remote behavior and all existing
stages remain unchanged.

### 2. Freeze exact model shapes

Encode these shapes as versioned collector inputs, never production eligibility
rules:

1. **Many-transition Australian:**
   `fixtures/australian-population/australian_population.hundredth.plan.json`,
   `fixtures/state/australian_population_2010_hundredth.state`,
   `data/abs/params/calibrated/2010.json`, seed `2506966152521542632`, 12 ticks,
   grouped observations enabled.
2. **Moderate-transition contested:**
   `fixtures/demographic/demographic_slots.plan.json` at 1,000,000 slots, reusing
   the exact deterministic four-area synthetic-state recipe in
   `scripts/bench-demographic.sh`.
3. **Few-transition:** `fixtures/plans/sir.plan.json` at 1,000,000 rows with a
   fixed numeric-population initialization, seed, and tick count recorded by the
   collector.

Use one worker for kernel attribution. The hardware evidence PRD may measure
additional explicit workers, but this PRD chooses no worker default.

### 3. Derive structural accounting generically

Derive accounting from validated model/layout data, not paths or names. At
minimum report per box/table/family-shaped transition group:

- rows, transitions, effect-bearing and contested transitions;
- logical candidates and claim instances;
- candidate/claim/control/owner bytes per lane;
- generated transition-function and expected phase-launch counts; and
- generated-source/PTX sizes when available.

The checked-in Australian shape must emerge as 418 transitions, 147,328,280
logical candidates, and 140,984,000 claim instances without a special case.

If accounting requires runtime/CLI output, keep it behind the collector's hidden
evidence mode. Do not add public help, manifest, IR, or plan fields.

### 4. Parse profiles without inventing wall time

The collector must preserve raw `.nsys-rep` output and parse versioned Nsight
Systems CUDA API/kernel summary CSVs. It must distinguish:

- transition validation and ordered reduction;
- generated transition execution;
- candidate-error checking and claim construction/resolution;
- effect activity/preparation/application;
- fired/deferred diagnostics;
- device observation;
- state/final-state transfer; and
- host setup/report/publication.

Do not sum overlapping streams and call the result wall time. Do not infer
occupancy/bandwidth from `utilization.gpu`. Reject unknown/missing required CSV
columns rather than silently producing zeroes.

Define `simulation_window_ms` as the interval after model/state loading, backend
construction, NVRTC/module setup, and warm-up through the final required observed
tick/control report. State export and artifact publication are separate fields.
Define `removable_transition_window_ms` as the union of target kernel intervals
clipped to that simulation window.

Record `queried_free_bytes_at_preflight` from device zero immediately before any
benchmark lane is constructed, with total VRAM, timestamp, existing context
allocations, and estimator component bytes.

### 5. Implement versioned evidence and gate schemas

Implement serializers/parsers and fixture tests for:

- raw run provenance;
- structural accounting;
- timing/profile summaries; and
- `profile-gate.json` with `verdict`, `authorized_spikes`, formulas, inputs, and
  evidence hashes.

PRD 0002 owns real gate creation. Dry-run fixtures must cover `proceed`, `stop`,
and `inconclusive`, and independently authorized `dense_family_dispatch` and
`stable_candidate_compaction` spikes.

## Allowed files

- `fixtures/australian-population/australian_population.hundredth.plan.json`
  (read-only benchmark input; any diff is a failure)
- `fixtures/state/australian_population_2010_hundredth.state` (read-only input;
  any diff is a failure)
- `data/abs/params/calibrated/2010.json` (read-only input; any diff is a failure)
- `fixtures/demographic/demographic_slots.plan.json` (read-only input; any diff
  is a failure)
- `fixtures/plans/sir.plan.json` (read-only input; any diff is a failure)
- `scripts/bench-demographic.sh` (read-only recipe; any diff is a failure)
- `scripts/run-cuda-transition-family-benchmark.py` (new)
- `scripts/tests/test_run_cuda_transition_family_benchmark.py` (new)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` (one opt-in
  stage only)
- `crates/sembla-cli/src/main.rs` (hidden accounting/timing emission only)
- `crates/sembla-cli/tests/**` (accounting/timing tests only)
- `crates/sembla-cuda/src/backend.rs` (hidden accounting/timing emission only)
- `crates/sembla-cuda/tests/**` (accounting tests only)
- `docs/prds-cuda-transition-families/README.md` (status note only)
- implementation notes/artifacts created by the managed run

If profiling can be supported without runtime/CLI changes, leave those files
unchanged. If a required change falls outside this list, stop and amend the PRD
before resuming.

## Non-goals

- No H100 run, measured profile, gate verdict, or speedup claim in this PRD.
- No transition-family classifier, direct candidate spike, generated family
  kernel, candidate/claim compaction, or production selector.
- No validation/control/effect/observation fusion, CUDA Graph, persistent kernel,
  worker policy, or capacity-policy change.
- No public CLI, manifest, IR, plan, dependency, model, calibration, or
  scientific evidence change.

## Acceptance criteria

1. The collector dry-run tests cover command construction, dirty-tree policy,
   missing GPU/profiler, subprocess failure, missing artifacts, checksums,
   unknown Nsight columns, overlap/union arithmetic, preflight sampling,
   structural accounting, and all gate-schema verdict/authorized-spike variants.
2. Exact frozen shape inputs and state-generation commands are emitted by dry run
   and are independent of the caller's locale/path layout.
3. Generic accounting reproduces the Australian transition/candidate/claim
   totals without matching a model/path/name and handles zero transitions,
   multiple boxes/tables, and overflow.
4. Ordinary CLI/runtime behavior, public help, manifests, generated CUDA source,
   scientific outputs, and tracked fixtures are unchanged.
5. The Hyperstack hook is opt-in; unset behavior is byte/command identical to its
   prior behavior.
6. Schema/parser fixtures can be recomputed deterministically and deliberately
   malformed/perturbed fixtures are rejected.
7. Required checks pass:

   ```text
   python3 scripts/tests/test_run_cuda_transition_family_benchmark.py
   cargo test --locked
   cargo test --locked -p sembla-cuda
   cargo check --locked --features cuda
   cargo clippy --locked --features cuda --all-targets -- -D warnings
   bash scripts/check-rust.sh
   python3 scripts/check-markdown-links.py
   python3 scripts/check-prd-allowlist.py docs/prds-cuda-transition-families/01-profile/0001-build-transition-profile-collector.md
   git diff --check
   ```

8. Implementation notes state explicitly that CUDA compilation/execution and all
   performance findings remain unverified until PRD 0002.

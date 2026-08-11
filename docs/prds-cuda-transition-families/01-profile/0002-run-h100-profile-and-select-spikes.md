# PRD 0002: Run the H100 profile and select direct spikes

max_review_cycles: 3

## Context

Read the parent track [README](../README.md) first. PRD 0001 must have completed
with the collector, parsers, structural accounting, and gate schemas green.

This PRD runs that collector on H100 and decides which small direct same-result
spikes, if any, are justified. It does not implement a candidate optimization or
scope a production optimizer PRD.

Hardware evidence is mandatory. A no-GPU run must not be approved as "hardware
pending".

## Goal

Profile the exact Australian many-transition workload plus moderate- and
few-transition comparison shapes, publish reproducible raw/derived evidence, and
issue independent authorization for a dense-family-dispatch spike and/or a
stable-candidate-compaction spike.

## Specification

### 1. Freeze and verify the measurement environment

Use one clean commit and one locked release CUDA binary for all arms. Record:

- repository status/commit and binary SHA-256;
- Rust, CUDA, driver, Nsight, OS, CPU/topology, RAM, GPU, and total VRAM;
- model/state/parameter and generated-source/PTX hashes;
- exact commands, environment, worker/stream mode, seed, ticks, warm-up, arm
  order, exit status, stdout/stderr, and checksums; and
- device-zero queried free bytes immediately before lane construction, including
  timestamp and existing context allocations.

Run the exact three frozen shapes from PRD 0001. Use one worker for kernel
attribution. Whole-run timing may include workers 1, 2, and 4 where conservative
preflight admits both compared arms; this PRD chooses no worker default.

### 2. Capture raw profiles and truthful phase attribution

Preserve `.nsys-rep`, CUDA API/kernel summary CSVs, timing JSON, static
accounting, output comparisons, and tool logs. At minimum report:

- `simulation_window_ms` under PRD 0001's exact boundaries;
- `removable_transition_window_ms` as interval union clipped to that window;
- setup/NVRTC/module, state transfer, observation, control/report, export, and
  publication separately;
- launch/function/source/PTX counts and sizes;
- candidate/claim/control/owner bytes per lane and worker arm;
- kernel union/overlap and positive inter-kernel gaps; and
- peak VRAM/RSS versus the conservative estimator.

Do not use summed multi-stream kernel duration as wall time. Do not extrapolate a
profiler-scale percentage into a claimed whole-run speedup.

### 3. Verify exact baseline behavior

The Australian CPU/CUDA differential must pass including grouped observations.
CPU-versus-CUDA sweep output uses the parent README's comparator: normalize only
`backend_identity`; all other file sets/bytes remain exact. A deliberate one-byte
perturbation must fail.

The baseline collector must reproduce the committed 7.730 CUDA/3.583 CPU gate
within explicitly reported run-to-run and environment differences; it need not
match those historical rates exactly.

### 4. Authorize spikes independently

Write `docs/evidence/cuda-transition-families/profile-gate.json` with:

- `schema` and complete input/evidence hashes;
- `verdict`: `proceed`, `stop`, or `inconclusive`;
- `authorized_spikes`: an ordered subset of
  `dense_family_dispatch` and `stable_candidate_compaction`;
- raw numerator/denominator fields and formulas for each decision; and
- rejected explanations and measurement limitations.

Authorize `dense_family_dispatch` only when:

```text
removable_transition_window_ms / simulation_window_ms >= 1/3
```

and the profile/static plan shows repeated compatible transition skeletons on
the Australian shape plus at least one structurally different comparison/fixture
shape suitable for a direct arm. This authorizes only the manual dense-dispatch
spike protocol, not production code.

Authorize `stable_candidate_compaction` independently when either:

```text
candidate_claim_bytes / queried_free_bytes_at_preflight >= 1/4
```

at an intended explicit worker arm, or conservative preflight proves those
arenas prevent the next required Australian scale/worker arm. This authorizes
only a manual stable-compaction spike; dense dispatch need not pass first.

`verdict` is `proceed` when at least one spike is authorized, `stop` when complete
evidence authorizes neither, and `inconclusive` when evidence is incomplete or
unstable. Do not average model shapes or invent a universal threshold from one.

### 5. Publish evidence and the measured decision

Commit raw/derived evidence under
`docs/evidence/cuda-transition-families/australian-baseline/`, including a
script-recomputed summary and SHA-256 manifest. Do not modify the calibrated
scientific evidence directory.

Append a `DECISIONS.md` entry in decision/alternatives/reason form. It must state
which manual spike protocol is authorized, or close the track when neither is.
Update `docs/performance/model.md` only where this exact profile contradicts a
current generalized performance statement.

A `proceed` verdict permits the operator to execute the authorized non-PRD spike
protocol(s). It does not permit `/piprd run` on production optimizer drafts.

## Allowed files

- `scripts/run-cuda-transition-family-benchmark.py` (hardware/parser fixes only)
- `scripts/tests/test_run_cuda_transition_family_benchmark.py` (evidence/parser
  fixtures only)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` (baseline
  stage fixes only)
- `docs/evidence/cuda-transition-families/**` (new hardware evidence)
- `DECISIONS.md` (measured profile/spike-authorization decision only)
- `docs/performance/model.md` (profile-backed correction only)
- `docs/prds-cuda-transition-families/README.md` (status note only)
- implementation notes/artifacts created by the managed run

No runtime, CUDA codegen/backend, model, state, parameter, IR, plan, manifest, or
scientific-evidence file is allowed. If a required change falls outside this
list, stop and amend the PRD before resuming.

## Non-goals

- No direct candidate spike or production optimizer implementation.
- No family classifier/kernel, compaction queue, selector, fallback, capacity
  policy, public CLI, manifest/IR/plan change, or model rewrite.
- No worker default, observation fusion, CUDA Graph, persistent kernel, fused
  grid-y revival, multi-GPU work, or recalibration.
- No speedup promise from profile shares.

## Acceptance criteria

### Local evidence-processing criteria

1. PRD 0001's collector/tests/checks are green at the run baseline.
2. Raw evidence parses without ignored required fields; all summaries and gate
   fields reproduce from raw records and checksums.
3. Comparator self-test passes and the deliberate perturbation is rejected.
4. Threshold-boundary fixtures prove independent authorization of neither,
   either, or both spikes and all three overall verdicts.
5. No ordinary runtime/codegen/model/scientific bytes change.
6. Required checks pass after evidence publication:

   ```text
   python3 scripts/tests/test_run_cuda_transition_family_benchmark.py
   cargo test --locked
   bash scripts/check-rust.sh
   python3 scripts/check-markdown-links.py
   python3 scripts/check-prd-allowlist.py docs/prds-cuda-transition-families/01-profile/0002-run-h100-profile-and-select-spikes.md
   git diff --check
   ```

### Mandatory hardware criteria

7. Build with `cargo build --release --locked -p sembla-cli --features cuda` and
   execute:

   ```text
   BENCH_CUDA_TRANSITION_FAMILIES=1 CUDA_TRANSITION_FAMILY_STAGE=baseline bash spikes/precision/infra-hyperstack/run-demographic-benchmark.sh
   ```

8. All three shapes complete from the same binary; Australian CPU/CUDA exactness
   and grouped outputs pass, and the comparator negative control fails as
   expected.
9. Raw Nsight reports, API/kernel summaries, timing records, structural/capacity
   accounting, hardware/toolchain provenance, exact commands, and checksums are
   committed.
10. At least three timed whole-run repetitions follow one warm-up; evidence
    reports medians/spread and keeps kernel interval union separate from wall.
11. `profile-gate.json` is independently reproducible, uses the exact formulas,
    and contains only justified `authorized_spikes`.
12. `DECISIONS.md` records the exact verdict and explicitly says production PRDs
    remain unauthorized pending direct same-result spike evidence.
13. This PRD is not approved while criteria 7–12 are pending.

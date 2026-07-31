# PRD 0001: Prepare the CUDA current-path rebaseline

Read `docs/prds-cuda-current-rebaseline/README.md` first; its contracts bind.

## Goal

Add a strict, default-off collector and analyzer that can execute the promoted
packed-pageable CUDA sweep path on a later approved H100, report absolute
workers-4 wall/phase/profile evidence, and always tear down paid resources. This
PRD performs local implementation and validation only.

## Specification

### 1. Freeze one six-execution protocol

Add `scripts/run-cuda-current-rebaseline.py`. Its manifest is fixed to these
commands, in this order, using one release CUDA binary, one synthesized 10M
state, seed 9009, 24 ticks, grouped observations and independent noise:

1. explicit `materialized`, workers 1, one draw, correctness preflight;
2. unset-selector production default, workers 1, one draw, correctness
   preflight;
3. unset-selector production default, workers 4, four draws, timed repetition
   1;
4. the same, timed repetition 2;
5. the same, timed repetition 3;
6. the same, post-timing Nsight Systems profile.

This is exactly six executions and 18 draws. Only command 1 sets
`SEMBLA_SWEEP_CUDA_FINAL_STATE_MODE`; commands 2–6 must actively remove it and
both retired device-SHA variables from the child environment. Reject inherited
selector/retired variables at collector startup rather than accidentally
measuring an override.

Use bounded 1,200-second per-command timeouts, atomic status/manifest writes,
per-arm stdout/stderr/exit/timing/resource records, 0.2-second RSS/VRAM samples,
and repository/dirty-status/binary/model/state identities. A timeout preserves
partial evidence and status 124.

### 2. Gate timed work behind correctness

Before command 3, require commands 1–2 to succeed, validate their timing
schemas, compare complete scientific output file sets and bytes, and separately
compare every `final_state_sha256`. The unset path must report
`mode=packed-pageable`, numeric pageable D2H and CPU SHA fields, null
reconstruction/pinned fields, true reconciliation fields, and zero pinned plus
cacheable-staging counts/bytes.

Copy one compared scientific file, mutate one byte, and require the comparator
to reject it. Record the negative control. Any parity, digest, schema,
diagnostic or negative-control failure stops before the timed repetitions.

All unset-selector timed/profile outputs must remain mutually byte-identical for
identical inputs where the protocol expects identical independent draw seeds.
Do not perform an extra verification download.

### 3. Attribute the current path without inventing a gate

Add `scripts/analyze-cuda-current-rebaseline.py`. It fails closed on missing or
extra arms, wrong order/count/draws/workers/noise/commit/hashes/argv, incomplete
outputs, parity failure, an accepted negative control, malformed timing,
unexpected materialized/pinned fields on current arms, nonzero pinned/staging
accounting, missing resource samples, or incomplete Nsight exports.

For the three unprofiled repetitions report raw values plus median, minimum,
maximum and range for:

- whole-sweep wall, setup, execution-window and publication time;
- per-draw wall time;
- final-state seam total, pageable D2H host API, CPU SHA-256, attributed and
  unattributed time;
- downloaded component/total bytes; and
- peak RSS and VRAM.

There is no pass/fail speed threshold. Label prior H100 B numbers historical and
non-binding. Analyzer success means evidence is complete and interpretable, not
that another optimisation is approved.

### 4. Add one separate Nsight profile

Command 6 uses `nsys profile --trace=cuda --sample=none --cpuctxsw=none
--stats=false` and is excluded from timing aggregation. Create its output parent
before launch. Export CUDA API, GPU trace/memcpy and kernel summaries.

Reuse or narrowly share the proven Nsight CSV compatibility logic, including
exact `Ctx` preference over `GreenCtx` and acceptance of rounded `0.000 MB` tiny
rows. Report final-state large-copy count/bytes/summed duration, copy-union
duration, copy/kernel overlap, exposed D2H outside kernels and relevant CUDA API
time. Never infer device overlap from host timers.

### 5. Integrate one strict paid-session flag

Add `BENCH_CUDA_CURRENT_REBASELINE=1` to
`spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`. It is mutually
exclusive with every other benchmark stage, baseline selector and focused flag;
accept only 0/1. Reject `KEEP_VM=1` and selector/retired variables before artifact
creation. Require an H100, `nsys`, a clean pinned commit and admin-only NVIDIA
performance counters.

The wrapper builds/synthesizes once, invokes only the six-command collector,
runs the analyzer, retrieves and verifies checksummed evidence, records benchmark
and teardown status separately, restores counter state, and uses the existing
`scripts/cuda-final-state-teardown.sh` path on EXIT/TERM/INT. Final status is the
nonzero benchmark status first, otherwise teardown status, and can never be zero
when state inspection or final reconciliation fails.

Do not change Terraform resources, defaults, CUDA/runtime behavior, the frozen
27-arm collector/analyzer, or the existing teardown helper unless a test exposes
a necessary generic-stage integration defect.

### 6. Document later operation, not approval

Update the Hyperstack runbook with the exact local-preparation boundary, later
saved-plan approval, CLI invocation, six-command inventory, evidence layout,
interpretation limits, secure-counter restoration, mandatory teardown, orphan
reconciliation and watchdog disarm rule. State explicitly that `/piprd run` and
this PRD do not approve or create paid resources.

## Allowed files

- `scripts/run-cuda-current-rebaseline.py` (new)
- `scripts/analyze-cuda-current-rebaseline.py` (new)
- `scripts/tests/test_run_cuda_current_rebaseline.py` (new)
- `scripts/tests/test_analyze_cuda_current_rebaseline.py` (new)
- `scripts/tests/test_cuda_readback_collector_flags.py`
- `scripts/cuda-final-state-teardown.sh` (only for a tested generic-stage
  integration defect; otherwise unchanged)
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`
- `spikes/precision/infra-hyperstack/RUNBOOK.md`
- `docs/prds-cuda-current-rebaseline/README.md` (status only)
- this PRD (status only)

If the existing teardown helper or its tests require a change, stop and report
the concrete defect rather than expanding scope silently.

## Non-goals

No VM provisioning, hardware execution, CUDA/runtime/source change, digest or
serialization change, C/pinned work, copy stream, double buffering, GPU SHA,
CPU evaluator optimisation, NCU collection, 20-draw matrix, adaptive rerun,
production regression threshold or new optimization decision.

## Acceptance criteria

1. The generated manifest has exactly six ordered records and 18 draws with
   classes control preflight, current preflight, timed ×3 and profile; only the
   last is profiled.
2. Commands 2–6 actively remove the current selector and retired variables;
   command 1 alone sets `materialized`. Tests prove inherited overrides are
   rejected before execution.
3. Synthetic tests prove preflight byte/hash parity, current-path timing/resource
   invariants and negative-control rejection before any timed command. Timeout,
   malformed schema, wrong identity/argv, missing arm/export/sample, nonzero
   pinned accounting and accepted negative control fail closed and preserve
   partial status.
4. The analyzer emits JSON and Markdown with raw/median/min/max/range absolute
   metrics, setup variability, final-state D2H/SHA attribution, resource peaks
   and Nsight evidence. It contains no performance eligibility verdict or
   binding old/new ratio.
5. Wrapper tests prove the new flag is strict and mutually exclusive, invokes no
   old matrix, rejects `KEEP_VM=1` and inherited selectors before artifacts, and
   preserves benchmark-versus-teardown status across success, failure, timeout,
   TERM and INT. Existing final-state collector/analyzer/teardown tests remain
   green.
6. The runbook documents the exact later approval and six-command invocation,
   mandatory teardown and contextual-only historical comparison. No local test
   provisions paid infrastructure.
7. Run and pass `python3 -m py_compile` for the new scripts, focused and existing
   script unit tests, `bash -n` on wrapper/teardown, Markdown-link checks,
   `python3 scripts/check-prd-allowlist.py` on this PRD, `git diff --check`, and
   the repository standard local check.

## Implementation status

Locally complete. The collector freezes six executions/18 draws, the analyzer
reports absolute current-path statistics without an eligibility gate, and the
wrapper reuses mandatory focused teardown. Synthetic and existing final-state
protocol tests pass. Paid H100 execution is intentionally pending a separate
operator-approved plan.

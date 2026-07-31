# PRD 0003: Prepare the focused H100 A/B/C decision run

> Completed experiment contract: this PRD deliberately did not authorize
> promotion. The later H100 result and operator promotion of B are recorded in
> `DECISIONS.md` §L14.

## Context

Read `docs/prds-cuda-final-state-readback/README.md` first; its contracts bind.
PRDs 0001 and 0002 provide three hidden modes on one binary:

- A `materialized` — current pageable download, host-state refresh and hash;
- B `packed-pageable` — pageable packed download and direct canonical host hash;
- C `packed-pinned` — retained pinned packed download, timed copy into retained
  cacheable staging, and direct canonical host hash.

The previous device-hash collector ran the full 20-draw worker matrix before a
production-size one-draw performance gate. Its worker-one treatment consumed the
900-second timeout and left only partial evidence. This PRD makes that ordering
impossible and prepares a short, attributable run. It does not itself create or
use paid hardware during `/piprd run`.

## Goal

Provide an opt-in collector, analyzer and runbook procedure that:

1. proves A/B/C correctness on one production-size draw before a matrix;
2. runs only a bounded four-draw workers-1/workers-4 decision matrix after the
   preflight passes;
3. captures timing, Nsight, memory and teardown evidence; and
4. emits a machine-readable go/no-go decision using the predeclared thresholds
   in the folder README.

## Specification

### 1. One explicit collector stage

Add one opt-in stage, named consistently in the script and runbook, for the
final-state A/B/C diagnostic. Validate it as a strict `0`/`1` switch and make it
mutually exclusive with unrelated benchmark/profile/corpus/device-hash stages.
The retired scalar device-hash selector must not remain reachable.

The remote build occurs once. Every A/B/C arm uses the same checkout, binary,
model, seeds, ticks and environment except for the hidden final-state mode and
the worker count under test. Capture commit, dirty state, CUDA/driver/GPU
identity, command lines and environment selectors.

### 2. Mandatory preflight before any matrix

Run exactly one 10M-slot, 24-tick draw for A, B and C in adjacent order. Before
continuing:

- compare complete output trees/file sets and every byte;
- separately compare `final_state_sha256`;
- require all versioned diagnostic fields and sane non-negative phase values;
- require C to report non-zero bounded pinned bytes and one retained buffer set;
- run a deliberate comparator negative control and require rejection; and
- apply a generous per-arm timeout so a recurrence cannot consume the session.

Any failure stops before multi-draw work while preserving logs, exit codes,
partial outputs and checksums. Performance is reported at preflight but is not a
promotion verdict from one sample.

### 3. Bounded decision matrix

Only after preflight passes, run exactly 18 independent-noise performance
commands: A/B/C at 10M slots, 24 ticks, four draws, workers 1 and 4, with three
adjacent paired repetitions. The frozen workers-1 order is `A-B-C`, `C-B-A`,
`A-B-C`; the frozen workers-4 order is `C-B-A`, `A-B-C`, `C-B-A`. Only these two
orders are valid, so A/B and B/C are adjacent in every repetition. The collector
asserts this property before the remote run.

Then run exactly one three-command CRN correctness set: A/B/C, 10M slots, 24
ticks, four draws, workers 4, one repetition. Both noise modes must remain
byte-identical across modes. Finally run exactly three profiling commands, one
per mode at 10M/four workers/four draws, after the timed matrix; these are
excluded from performance aggregation. Together with the three one-draw
preflight commands, the stage has exactly 27 remote benchmark executions.

Do not launch the historical 20-draw matrix, workers 2, other concurrency
designs, or unrelated corpus/profile suites in this stage. A later operator may
approve more evidence only after reviewing this result.

### 4. Focused profiling and resource evidence

Collect Nsight Systems in the three explicit post-matrix profiling commands,
covering CUDA API, GPU memcpy and kernels. Do not wrap or contaminate a timed
performance repetition. The analyzer must report:

- whole-sweep wall and paired ratios;
- final-state total, D2H host API/enqueue, completion wait, reconstruction and
  CPU hash phases;
- large-copy count/bytes/duration, copy-union duration, exposed D2H outside
  kernels, and copy/kernel overlap where Nsight permits;
- setup/execution/publication times;
- peak VRAM, RSS, pinned bytes and cacheable staging bytes by worker count; and
- buffer-set count and underlying pinned-allocation count per retained lane.

Do not infer overlap from a host timer when Nsight can measure it. Do not add
tiny control copies to the 480 MB final-state category.

### 5. Machine-readable gate

Add or extend an analysis script that consumes the evidence directory and emits
both Markdown and JSON. It validates completeness before calculating a verdict.
The JSON records each predeclared threshold and pass/fail evidence.

Use the median of the three adjacent within-repetition wall ratios,
independently for each worker count. Workers 4 at 10M is binding. B compares
only with A; C compares only with B. B passes performance when median B/A <=
0.95 at workers 4 and B/A <= 1.02 at workers 1. C passes performance when median
C/B <= 0.95 at workers 4 and C/B <= 1.02 at workers 1. Individual repetitions
are reported but do not alone veto a treatment.

C additionally requires one frozen mechanism ratio to pass:

- host: `C_host_blocking_ms / B_host_blocking_ms <= 0.90`, where
  `B_host_blocking_ms = pageable_dtoh_host_api_ms` and `C_host_blocking_ms =
  pinned_dtoh_enqueue_api_ms + wait_to_pinned_host_readable_ms`; or
- Nsight: `C_exposed_final_state_dtoh_ms /
  B_exposed_final_state_dtoh_ms <= 0.90`.

This is an OR gate fixed before execution. Staging-copy time is excluded from
these D2H mechanism ratios but remains included in final-seam and whole-sweep
time. Neither treatment is eligible if output parity fails, a negative control
is accepted, diagnostics are incomplete, or its resource accounting is
unbounded.

Encode all thresholds/directions as JSON objects and test exact boundary values.
Report absolute times and raw repetitions alongside ratios. A miss remains a
no-go; the analyzer cannot lower thresholds.

### 6. Runbook, approval and teardown

Document exact CLI commands, expected duration/cost envelope, preflight and
matrix boundaries, evidence location, recovery, and teardown. Follow the
existing Hyperstack rules:

- explicit operator approval is required before `terraform apply` or VM
  creation;
- prefer CLI/Tailscale transport over VNC;
- install no persistent broad SSH rule;
- teardown VM and SSH rule in success, failure, timeout and interrupt paths;
- run `reconcile-orphans.sh` and record zero remaining VMs/orphans; and
- restore GPU performance-counter access to the documented secure state.

The new diagnostic stage must reject `KEEP_VM=1` before artifact creation. Give
this stage an idempotent EXIT/TERM/INT teardown path that attempts
`terraform destroy`, invokes `reconcile-orphans.sh --delete --yes` if normal
destroy is incomplete, then runs report-only reconciliation and records a
zero-resource result. Test success, command failure, timeout, TERM and INT with
stubbed Terraform/reconciler commands. Record separate `benchmark_status` and
`teardown_status`. The final process status is the benchmark status when it is
nonzero, otherwise the teardown status; teardown or reconciliation failure can
never yield overall success.

The automated `/piprd run` acceptance stops at local collector validation. It
must not interpret this PRD as paid-plan approval.

## Allowed files

- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`
- `spikes/precision/infra-hyperstack/RUNBOOK.md`
- `spikes/precision/infra-hyperstack/reconcile-orphans.sh` — only if a tested
  teardown gap specific to this collector is found
- `scripts/**` — new focused analyzer/comparator support only
- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/tests/**` — only if 0001's
  diagnostic sidecar needs minimal collector-facing serialization support
- `docs/prds-cuda-final-state-readback/README.md` (status notes only)
- `docs/evidence/**` (new hardware evidence only, after separate approval)
- `docs/performance-model.md`, `DECISIONS.md` — only after real hardware evidence
  exists; no projected verdict

If a required local gate fails outside this list, stop and report it on the
first attempt. Do not edit `.piprd/**` or provision paid resources.

## Non-goals

- No production promotion/default change; a winner requires a later PRD.
- No implementation changes to hash, pinned buffers, kernels, scheduling or
  scientific output.
- No 20-draw matrix, broad profile, CPU evaluator work or setup optimization.
- No paid H100 run during the managed implementation loop.

## Acceptance criteria

**Local, required for `/piprd run` approval:**

1. The opt-in stage is strict, mutually exclusive and uses one binary for all
   modes. Retired device-hash selectors are rejected or absent.
2. Tests/fixtures prove no matrix command can be issued before A/B/C preflight
   parity, diagnostic validation and negative-control rejection succeed. A
   simulated timeout preserves partial evidence and exits before the matrix.
3. The stage emits exactly 27 executions: three one-draw preflights; 18 timed
   commands (three modes × workers 1/4 × three paired repetitions, four draws)
   in the frozen `A-B-C`/`C-B-A` schedules above; three CRN commands (A/B/C,
   workers 4, four draws); and three post-matrix Nsight commands excluded from
   timing. A pre-run assertion proves A/B and B/C adjacency in every timed
   repetition. No 20-draw or unrelated stage is reachable from this flag.
4. The analyzer rejects missing arms, mismatched commands/commits, parity
   failure, accepted negative controls, missing timings, unbounded pinned or
   staging bytes and malformed Nsight data. Synthetic fixtures cover exact
   boundary pass/fail, threshold miss, aggregate >2% workers-1 regression,
   C's frozen host/Nsight 0.90 mechanism OR gate, and incomplete evidence.
5. The analyzer emits Markdown and JSON with absolute times, the median of
   adjacent within-repetition ratios, workers-4 5% B/A and C/B gates,
   workers-1 2% regression vetoes, C's exact host/Nsight 0.90 mechanism gate,
   phase/resource evidence and residual risks.
6. `bash -n` and shell tests pass. The collector has bounded per-arm timeouts,
   rejects `KEEP_VM=1` before artifacts, and has an idempotent EXIT/TERM/INT
   teardown tested with stubbed success, failure, timeout, TERM and INT paths.
   Benchmark and teardown statuses are recorded separately; final status uses a
   nonzero benchmark status first, otherwise teardown status, and can never be
   zero after teardown/reconciliation failure.
7. The runbook states that `/piprd run` is not paid-plan approval and documents
   exact approval, CLI transport, evidence, secure-counter restoration,
   teardown and orphan-reconciliation steps.
8. Run and pass relevant script/unit tests,
   `python3 scripts/check-prd-allowlist.py` on this PRD, Markdown-link checks,
   and the repository's standard local check command.

**Hardware, explicitly deferred until separate operator approval:**

9. Preflight A/B/C complete trees and digests are byte-identical; the negative
   control is rejected before the matrix starts.
10. The bounded matrix, CRN correctness set, post-matrix Nsight traces,
    VRAM/RSS/pinned/staging accounting, both allocation counts, checksums, exit
    codes and machine-readable verdict are complete.
11. Teardown evidence shows zero remaining Hyperstack VMs/orphans and no ongoing
    billing. The performance-counter setting is restored.
12. Only a treatment passing the predeclared gate may be proposed for a
    production-promotion PRD; otherwise the result closes this route.

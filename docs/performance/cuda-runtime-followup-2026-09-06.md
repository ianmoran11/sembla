# CUDA runtime follow-up — 2026-09-06

Implemented and checked locally. The first H100 candidate passes the full GPU
corpus, complete output comparisons, and the frozen performance gate. Its
whole-process grouped sweep time falls **20.4% at 1M slots** and **8.3% at 10M**.
These are single adjacent pairs, not repeated speedup estimates.

The final code also restores the original order of a retained state copy after
the first measurements exposed higher peak memory. Local checks pass for this
correction. An optional repeated comparison failed during its first baseline
warmup, so **the final correction's peak memory and whole-process performance
were not remeasured**. No repeated result is claimed. The VM was destroyed,
provider reconciliation found zero VMs, and temporary credentials were removed.

## Measured comparison

The baseline is `697307b55bb83a08dbc49e362d4c812088360be2`, which already includes
the previously measured contest-target optimization. The primary measured
candidate is `f84bb05b4d7d3c9fb4d6ce829570569ab42ac408`. The final memory-order
correction is `d584369a7b2c850997073ae70dfdda937774885d`; it changes construction
copy order and timing attribution, with no generated-kernel or execution change.
CPU, runtime, IR, fixtures, examples, and Cargo.lock are unchanged between the
baseline and both candidates. Earlier CPU work in the developer checkout is
preserved and excluded from this comparison.

Both arms use one H100 PCIe, the same input bytes, 24 ticks, 20 draws, seed 9009,
independent noise and grouped observations. Each same-backend baseline/candidate
pair ran consecutively. Whole-process intervals include setup and finalization.

| Slots | Baseline CUDA | Measured candidate CUDA | Wall-time reduction | Baseline CPU | Candidate CPU |
|---|---:|---:|---:|---:|---:|
| 1M | 4.57 s | 3.64 s | 20.4% | 153.58 s | 160.95 s |
| 10M | 25.16 s | 23.06 s | 8.3% | 1768.76 s | 1755.76 s |

Later-draw CUDA medians are 130.530 → 109.557 ms at 1M (16.1% less), and
1016.132 → 905.973 ms at 10M (10.8% less). These use the existing 3 ms polling
of published draw files, not CUDA events. Draws also have distinct seeds.
At 1M, first-draw/setup time falls 1892.225 → 1362.415 ms and accounts for over
half the whole saving. The candidate was profiled before the baseline's first
GPU sweep, so startup/driver warmup can bias this initial pair. The unchanged
CPU arms vary by +4.8% at 1M and −0.7% at 10M.

The earlier 15–25% overall estimate was optimistic for the 10M workload. The
measurements establish a useful gain for these commands, with limited confidence
about its size across fresh sessions or other workloads. Cache reuse and the
exclusive-effect shortcut below do not benefit this retained demographic worker.

All **103 files per sweep tree** match before/after for each backend at both
scales. CPU/CUDA comparisons normalize only the established `backend_identity`
manifest field; same-backend comparisons are byte-exact. Exported pairs match,
and the intentionally corrupted grouped sidecar is rejected.

The unchanged 10M/24-tick no-grouped gate reports CUDA 6.20/6.28/6.22 s and CPU
50.12/49.64/50.99 s. Its median CPU/CUDA ratio is **8.058×**, passing the 3×
threshold. Results, summaries and execution hashes match across all six runs.
This is a backend comparison at the primary candidate, not its gain over baseline.

Evidence:

- [Primary artifacts](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/README.md)
- [Independent sweep analysis](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/review/sweep-analysis.json)
- [Verification log](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/review/verification.log)
- [Incomplete repeat attempt and limits](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/review/repeated-validation-attempt/README.md)

The remote primary snapshot is commit
`02ab404eec35f1204571ae2520453f0c12e916fc` on
`evidence/hyperstack-20260906T073508Z`. Local review/closeout records are additive;
`SHA256SUMS.remote` remains unchanged.

## Changes

- **One winner scan per rule.** Fired-count reduction now also sets the activity
  flag consumed by effect validation. It runs after conflict resolution, before
  validation, and its counts are retained for the final report. This removes
  the separate activity scan and its atomic operation per winner. Rules without
  effects still contribute to fired counts; empty rules retain zero counts.
- **Skip empty transition validation.** Code generation records whether each
  rule actually emitted any expression checks. Rules with none omit the four
  validation passes and their status commit. Checked expressions retain the
  existing ordering and complete-column checks.
- **Reduce safe effect preparation to one pass.** A rule can omit the three
  diagnostic recovery passes and commit only when each written attribute has
  exactly one effect destination in the entire model. SetAttr addresses the
  executing row, so these destinations cannot collide. The earlier ordered
  effect validator still checks expressions and ranges. Repeated destinations
  retain all passes. The demographic model's destinations are shared, so this
  particular shortcut does not benefit its preparation stage.
- **Reduce grouped atomics.** Banded extrema now reduce thread-local bounds
  within each block before updating global bounds. Histograms with at most
  2,048 bins accumulate in shared memory and flush once per occupied block bin;
  larger key spaces keep the global-atomic path. Slot-local cardinalities select
  the path in fused batches. Integer counts, band definitions, filtering, output
  order, and empty-state handling are unchanged.
- **Reuse compiled PTX within a process.** A bounded one-entry cache retains the
  last successful translation, keyed by the actual generated-source SHA256.
  Compilation is serialized to avoid duplicate work across simultaneous
  constructors. Contexts, modules, streams, and mutable buffers remain per
  backend. Failed compilations are not cached. This helps multiple independent
  backends using the same source; it does not avoid the first compilation in a
  fresh CLI or add another benefit to an already retained single sweep worker.
- **Attribute startup and command cost.** Optional
  `--lifecycle-timing-json PATH` writes a separate diagnostic document. It
  records preparation, backend construction/execution/materialization, and
  hashing/export/publication. Nested CUDA construction timers cover host state
  validation and retained copies, code generation, context/identity, NVRTC/cache,
  module/stream, function lookup, packing, and device allocation/upload. Existing
  scientific artifacts and tick-timing schemas are unchanged. Report paths
  cannot alias inputs or other known outputs, including hard links.

Public reference CUDA generation and its checked-in golden remain unchanged.
The backend compiles an execution variant; its hash and optional source dump
cover exactly that variant, including fused rewriting. Observation generation
and CLI tick-timing serialization move into focused modules. No new dependencies,
model contracts, canonical bytes, or scientific fixtures are introduced.

The hardware test also exposed a pre-existing fused-slot alignment defect:
when a mixed-type packed state ended between eight-byte boundaries, later slots
misaligned typed loads. State, input, and aggregate byte arenas now round each
slot stride up to eight bytes with checked arithmetic. Logical lengths and
canonical state hashes remain unchanged. The regression exercises empty,
one-row, and non-block-multiple populations and changes fused widths 2 → 1 → 2.
The final test reports zero Compute Sanitizer errors.

## Profile attribution and memory correction

The primary 5M/two-tick no-grouped trace has **216 launches**, versus 336 in the
retained September 5 profile at the comparison baseline: 120 launches removed
(35.7%). Separate effect-activity scans and empty transition validators are
absent. Summed kernel time is 9.148 ms; the unprofiled tick-loop timer is 9.496 ms.
The grouped tick loop is 10.201 ms, with grouped extrema taking 0.143 ms across
four launches and grouped histograms 0.213 ms across six launches in its trace.
Effect preparation still runs 80 launches: this model shares its destinations
and does not qualify for the exclusive-effect shortcut.

The older no-grouped/grouped tick loops are 10.831/14.662 ms, but were collected
on a different H100 session. Those profile durations explain the mechanism;
the adjacent sweep pairs above provide the controlled wall-time comparison.

The primary grouped lifecycle report covers 3442.489 ms after option parsing:
1187.418 ms input/model preparation, 2063.299 ms backend construction/execution/
materialization, and 191.772 ms hashing/export/publication. CUDA construction
accounts for 1695.389 ms, including 461.290 ms host state validation/copies,
579.500 ms NVRTC (cache miss), 134.244 ms packing and 199.555 ms allocation/upload.
Compilation is about 17% of this short command, not all of startup. These timers
cover a different size and boundary from the 20-draw sweeps.

The initial 10M sweep's peak RSS rises from 2,571,240 to 2,996,452 KiB. Inspection
found that collecting construction timing had moved the pristine-state clone
ahead of the temporary packed upload copy. The final `d584369` correction restores
the baseline order, allowing those allocations to avoid overlapping, and adds
the late copy's measured duration to the same host-state phase. It introduces
no additional retained buffer. Its actual peak RSS remains unconfirmed because
the attempted repeated run did not reach the final candidate's measurements.

The repeat protocol preserved the baseline executable after the original
collector removed its build worktree. Native sweep timing resolves that compiled
worktree path to obtain repository identity, so the missing checkout makes it
fail. This diagnosis follows from the retained protocol and CLI source; the
baseline's per-command stderr was not retrieved before automatic teardown.
The corrected candidate's build and hardware-corpus subprocesses returned zero
before the script entered that first baseline warmup, but their remote logs were
also not transferred. The retained hardware corpus is therefore the primary
`f84bb05` evidence, and no warmed performance or memory result is accepted.
A future repeat must retain the build checkouts and collect partial diagnostics
before resuming teardown.

## Validation and closeout

The final clean candidate passes `./scripts/check-rust.sh`,
`./scripts/check-determinism.sh`, and CUDA-feature library tests (40 passed,
four hardware tests ignored locally). The developer checkout also passes the
full Rust checks with `RUST_TEST_THREADS=1` and determinism. One earlier parallel
run hit a temporary-directory collision in an unchanged state-artifact test;
the serial rerun passed. No unrelated test or fixture was changed.

The primary H100 corpus passes negative/error-ordering, rollback, resource and
fused checks. The new reduction test covers empty, one-row and 1,027-row inputs,
negative bands, 408-bin shared and 4,152-bin global histograms, no-effect fired
counts, resets and fused widths 2 → 1 → 2. Its retained Compute Sanitizer run
reports zero errors. Grouped and no-grouped profile outputs also match CPU.
Collector flag tests (12 cases), shell parsing, reference-generation fixtures,
lifecycle output/alias checks and `git diff --check` pass.

Reproduce primary checksum/output verification after local review files are
included:

```sh
evidence=docs/evidence/demographic-bench/hyperstack-l4-20260906T061831Z
python3 "$evidence/review/verify.py" "$evidence"
```

The approved session followed the [runbook](../../spikes/precision/infra-hyperstack/RUNBOOK.md)
and [provisioning instructions](../../spikes/precision/infra-hyperstack/README.md),
using one CANADA-1 H100 at $2.506720430/hour including its public IP and an armed
four-hour destruction watchdog. The full primary corpus preceded profiles,
adjacent 1M/10M sweeps and the frozen gate. Initial failed NVRTC/test attempts
are retained separately and supply no accepted performance result.

The collector retrieved and verified the primary archive, destroyed both paid
resources, and verified empty Terraform state. At 07:38:58 UTC,
[provider verification](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/review/provider-verification.json)
returned HTTP 200 with zero VMs. The watchdog was disarmed, disposable credentials
were cleaned, the generated console-password item was deleted, and the original
non-creating Terraform configuration was restored. The session cost is roughly
**$4.50**, an elapsed-time estimate rather than an invoice. See the
[closeout record](../evidence/demographic-bench/hyperstack-l4-20260906T061831Z/review/session-closeout.json).

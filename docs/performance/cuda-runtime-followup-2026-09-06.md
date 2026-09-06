# CUDA runtime follow-up — 2026-09-06

Status: implemented and locally checked. The new execution kernels have not yet
run on CUDA hardware. GPU differential checks, same-host before/after sweeps,
and the frozen performance gate remain required before a performance verdict.

The comparison baseline is `697307b` (the already measured contest-target
optimization). Separate CPU runtime changes in the developer checkout must be
excluded from the CUDA benchmark candidate so its CPU oracle is unchanged.

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

## Evidence and limits

The retained September 5 H100 no-grouped profile has 336 launches and 10.620 ms
of summed kernel durations. Validation/preparation account for 26.41% and 210
launches; fired counting plus activity detection account for 14.22% and 40
launches. Those shares explain where to investigate, not a prediction of
whole-command savings. The grouped tick loop takes 14.662 ms versus 10.831 ms
without groups; a new grouped Nsight trace is needed to separate extrema and
histogram costs.

In the retained 10M/20-draw native sweep timing, the whole interval is 25.291 s,
with 19.977 s summed draw bodies. Final downloads total 5.014 s and CPU hashes
5.259 s. The 5.314 s outside draw bodies cannot all be attributed to NVRTC.
The new lifecycle report is intended to resolve that attribution before larger
startup or finalization changes. Existing rejected pinned-buffer and serial
device-hash experiments are not reopened by this patch.

The earlier 15–25% overall estimate remains speculative. These optimizations
overlap and depend on the workload; the PTX cache and exclusive-effect shortcut
do not apply to every demographic command. Only controlled wall-time results
can establish the combined gain.

Reference artifacts:

- [No-grouped kernel summary](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/profile/nsys-kern-sum.txt)
- [Grouped tick timing](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/profile/timing-grouped-cuda.json)
- [10M native sweep timing](../evidence/demographic-bench/hyperstack-l4-20260905T121420Z/sweep/10000000/current-cuda-native-timing.json)
- [Previous measured review](cuda-runtime-review-2026-09-05.md)

## Validation

Local checks passed:

- `./scripts/check-rust.sh` (architecture, source budgets, formatting, both
  Clippy configurations, workspace tests, dependency and lock policies).
- `./scripts/check-determinism.sh`.
- `cargo test --locked -p sembla-cuda --lib --features cuda`: 40 passed,
  four hardware tests ignored.
- Collector flag regression tests: 12 passed; changed shell scripts parse.
- Generated reference fixture comparison and the new lifecycle output/parity
  and alias-protection integration test.

The new ignored hardware test compares CPU reports and state hashes for empty,
one-row, and 1,027-row populations, negative band values, small/shared and
large/global histograms, rules with no effects, resets, and fused widths
2 → 1 → 2. The differential collector invokes it alongside the existing
negative/error-ordering, rollback, resource, and fused tests.

The GPU plan should run the full differential corpus first, then grouped and
no-grouped 5M/2-tick profiles with lifecycle and Nsight artifacts, controlled
1M/10M grouped 20-draw baseline/candidate sweeps, and the unchanged three-run
10M/24-tick gate. Verify scientific outputs across both versions and both
backends before interpreting time differences. The collector adds diagnostics
without changing the frozen gate protocol. Paid provisioning requires approval
of the exact saved plan under the infrastructure README.

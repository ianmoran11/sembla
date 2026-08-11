# CUDA transition-family optimization track

This folder is a profile-gated investigation of CUDA per-transition launch,
candidate, and claim costs. It contains two ordered measurement-infrastructure
PRDs suitable for pi-piprd, followed by two explicit **non-PRD** direct-spike
protocols. Production optimizer PRDs are deliberately absent until direct
same-result evidence justifies them.

Run from the Sembla repository with:

```text
/piprd run docs/prds-cuda-transition-families/01-profile/0001-build-transition-profile-collector.md
```

Pi-piprd starts from the named PRD and automatically advances to
`0002-run-h100-profile-and-select-spikes.md` in the same directory. `README.md`
is ignored. The repository must be clean before the run starts.

PRD 0001 is locally implementable. PRD 0002 has mandatory H100 criteria; a
no-GPU run must not approve it as hardware-pending. Start this managed run only
when the existing Hyperstack collector can be executed, or expect it to stop at
PRD 0002.

## Why this track exists

The latest committed Australian H100 evidence is
`docs/evidence/australian-population/calibrated-2026-08-07/`. It records exact
CPU/CUDA equality and 7.730 CUDA ticks/s versus 3.583 CPU ticks/s at hundredth
scale, but no Nsight trace, utilization, kernel share, publication share, or
end-to-end phase breakdown.

The plan contains 418 transitions over one 352,460-row `person_slot` table. The
generic CUDA layout creates one logical candidate per transition and row and one
claim instance per contested transition and row. Static accounting gives
147,328,280 logical candidates and 140,984,000 claim instances per lane; the
current host schedule has at least 7,942 transition-indexed launches per tick.
These are strong hypotheses, not measured bottleneck findings.

`DECISIONS.md` §L6 and §M1 bind:

1. profile the intended shape to find candidate work;
2. build a small committed, runnable arm computing the same result;
3. assert equality and measure that arm directly on at least two materially
   different shapes before choosing a general rule; and
4. only then draft production performance PRDs.

Spikes are evidence, not implementations, and carry no PRD acceptance
obligations. That is why this folder does not misuse `/piprd run` to implement a
candidate spike or pre-scope production work.

## Ordered PRDs

1. [`01-profile/0001-build-transition-profile-collector.md`](01-profile/0001-build-transition-profile-collector.md)
   — add the reproducible collector, generic structural accounting, parsers,
   schemas, and local failure tests without making a performance claim.
2. [`01-profile/0002-run-h100-profile-and-select-spikes.md`](01-profile/0002-run-h100-profile-and-select-spikes.md)
   — run the exact H100 profile and independently authorize dense dispatch,
   stable compaction, both, or neither.

PRD 0002 writes
`docs/evidence/cuda-transition-families/profile-gate.json` with an ordered
`authorized_spikes` array. A `stop` or `inconclusive` verdict closes or pauses the
track without automatically entering another managed directory.

## Direct spike protocols — not PRDs

After an operator verifies PRD 0002's evidence, execute only the explicitly
authorized protocol(s) on an ordinary spike branch/workflow:

- [`spikes/dense-family-dispatch.md`](spikes/dense-family-dispatch.md) — direct
  dense family-dispatch arm, preserving dense candidate/claim storage.
- [`spikes/stable-candidate-compaction.md`](spikes/stable-candidate-compaction.md)
  — direct deterministic physical compaction arm, independent of dispatch.

Do not pass these files to `/piprd run`. Each spike is committed, runnable,
equality-checked, directly measured, and retained as positive, negative, or
inconclusive evidence per §M1.

Dense dispatch and compaction are independent. A capacity-only profile may
authorize compaction without authorizing dispatch; a negative dispatch result
must not close independently justified capacity work.

## Binding semantic invariants

Every collector, spike, and any later optimized path must preserve:

1. **Logical candidate identity.** The observable identity remains
   `candidate_offsets[rule_id] + row`; a physical queue ordinal never enters
   diagnostics, conflicts, reports, effects, or RNG.
2. **Philox coordinates.** Draws remain keyed by
   `(seed, tick, rule_word, entity_id, draw_idx)`.
3. **Global semantic order.** An execution plan is an ordered sequence of maximal
   contiguous compatible family fragments and singleton legacy occurrences.
   Sharing generated code across noncontiguous fragments must not move them
   around intervening transitions.
4. **Exact diagnostics.** Declaration/row/claim order, minimum-error key,
   status/payload/message, winner ordering, effect ownership, and double-write
   diagnostics are unchanged.
5. **Exact arithmetic.** Native-`f64`, checked integers, aggregate order, and
   CPU/CUDA differential expectations remain unchanged.
6. **Explicit fallback.** Unsupported constructs use an accounted legacy path
   chosen before mutation. Runtime failure never replays a partially mutated
   tick.
7. **Lane isolation.** Descriptors, queues, counters, scans, and arenas belong to
   one `CudaBackend` lane under supported free-running streams and retained fused
   tests.
8. **Zero-row behavior.** Scalar validation observable on an empty table remains
   observable; empty/overflow/capacity cases fail or fall back deterministically.
9. **No wire-format change.** Families are an internal lowering. Model, plan,
   manifest, backend identity, and scientific schemas do not gain a family
   concept without a later explicit provenance decision.

## Comparison contract

Candidate-versus-legacy CUDA arms share backend identity and compare all
scientific/output-tree bytes directly.

CPU-versus-CUDA sweeps reuse `normalize_sweep_tree` from
`spikes/precision/infra-hyperstack/run-demographic-benchmark.sh`: only the
established `backend_identity` manifest field may be normalized. File sets and
all other bytes remain exact. Hidden timing/evidence files use a versioned
explicit allowlist; scientific values, state/hash, candidate/status
payload/message, RNG, and ordering are never normalized. A deliberate one-byte
perturbation must fail.

## Existing work to reuse, not recreate

This track builds on completed CUDA validation, host-path, observation,
final-state, evaluator, and sweep-throughput work. It must not reimplement:

- segmented argmin conflict resolution;
- lock-free four-pass minimum-error reporting;
- generic `rule_id` validation dispatch;
- existing effect-active semantics;
- device-side fired/deferred reduction;
- scalar/grouped device observation;
- retained backends and free-running nonblocking-stream draw concurrency;
- packed final-state hashing/readback; or
- rejected fused grid-y draw batching.

Fused grid-y changed draw geometry and measured negative. It remains closed.
Transition families, if supported, group contiguous logical transition fragments
within a draw and do not revive that mechanism.

## Future production roadmap — intentionally not PRDs yet

Only positive, exact direct spike evidence may justify later `/piprd plan` work.
The measured result decides which entries exist; do not create placeholder PRDs
for negative mechanisms.

Potential sequence after evidence:

1. materialize deterministic model-generic metadata with explicit ineligibility
   reasons and no runtime change;
2. productionize only directly measured dense-family phases while retaining a
   legacy default;
3. if the separate compaction spike wins, implement its frozen structural rule;
   otherwise omit compaction entirely;
4. integrate measured winners with conservative capacity accounting, lane-local
   state, mixed fallback, and the complete differential corpus;
5. run a frozen-binary automatic-selector H100 gate; and
6. in a separate promotion PRD, change the default exactly as decided, rebuild,
   and run a final-device exactness smoke test so the shipped binary is verified.

Observation fusion and CUDA Graphs remain separate. Existing evidence found
device observation cheap on another H100 shape; no follow-up is justified without
a post-candidate direct measurement.

## Measurement protocol

Hardware comparisons use adjacent arms from one locked release binary, commit,
model/state/parameter bytes, seed, tick count, stream mode, and worker count.
Report at least three timed repetitions after one warm-up. Preserve raw records;
do not substitute profiler extrapolation for whole wall.

The corpus includes:

- exact Australian 2010 hundredth plan/state/parameters for 12 ticks;
- a moderate-transition contested demographic shape;
- a few-transition SIR shape;
- sparse and dense direct-spike fixtures; and
- failing guard, hazard, claim, effect, reference, checked-integer, double-write,
  and zero-row scalar-validation fixtures.

Evidence records repository/binary hashes, dirty state, GPU/driver/CUDA,
model/state/parameter hashes, exact commands, launch/function counts,
generated-source/PTX sizes, setup/NVRTC, kernel/API summaries, whole wall,
ticks/s, peak VRAM/RSS, queried free memory and sample point, worker count, and
complete comparisons.

## Global non-goals

- No Australian-specific production kernel, state/age constant, benchmark-name
  check, or one-shape selector threshold.
- No changed IR, plan, manifest, RNG, precision, diagnostic, or scientific output
  contract.
- No automatic worker default, dynamic VRAM tuning, multi-GPU, CUDA Graph,
  persistent kernel, observation fusion, or fused grid-y revival.
- No model rewrite or regeneration/blessing of scientific goldens.
- No speedup claim from static arithmetic, local CUDA checks, or profile shares
  alone.

## Required checks for the managed PRDs

```text
cargo test --locked
cargo test --locked -p sembla-cuda
cargo check --locked --features cuda
cargo clippy --locked --features cuda --all-targets -- -D warnings
bash scripts/check-rust.sh
python3 scripts/check-markdown-links.py
python3 scripts/check-prd-allowlist.py <current-prd-file>
git diff --check
```

If a check requires a file outside a PRD's allowed list, amend the PRD before
resuming; do not rely on an out-of-band scope exception.

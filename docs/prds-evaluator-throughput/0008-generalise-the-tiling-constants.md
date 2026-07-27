# PRD 0008: Derive the tiling constants from the model, not from one benchmark

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind.

PRD 0001 introduced whole-tick tiling with two constants (`eval.rs:19,22`):

```rust
pub(crate) const TICK_TILE_ROWS: usize = 1_024;
pub(crate) const TICK_TILE_THRESHOLD: usize = 1_000_000;
```

Both were measured on `demographic_slots` and neither knows anything about the
model being run. `tick_tiling_enabled` is `row_count >= threshold`, nothing more.

**The tile size is the risky one.** 1,024 was chosen so this model's live set
fits L1 — PRD 0001's evidence records a 20,480-byte conservative peak against an
assumed 32,768-byte L1. That peak is a property of how many guards, hazards and
views a tick evaluates. A model with substantially more of them has a larger
live set, and at 1,024 rows it would spill out of L1, making tiling
**counterproductive rather than merely useless**. The figure was computed by
hand for the evidence; nothing in the code derives it.

**The threshold is mis-set.** It is `>= 1_000_000` and the binding benchmark is
exactly 1,000,000 rows, so the case that justified it only just qualifies.
Meanwhile `threading_spike` measured parallelism paying from ~262,144 rows —
2.4× on guards, 5.3× on the racing clock. Models between roughly 250k and 1M
rows get no tiling and no parallelism at all, for no measured reason.

**And every performance decision in this folder came from one model.** The
eleven models in `examples/` are SIR-scale, far below the threshold, so they
never exercise any of this and cannot act as canaries.

## Goal

Tile size and the tiling threshold are derived from the model. A
differently-shaped model gets a tile that fits its live set, and gets tiled when
tiling would pay.

## Specification

### 1. Derive the live set from the IR

Compute, for the expressions a tick tiles, the peak concurrent intermediate
width plus the retained root widths — the same quantity PRD 0001's evidence
reported by hand as `deepest_benchmark_guard_conservative_peak` and
`final_roots_total`.

Per row, that is:

- the maximum, over tiled expressions, of the concurrent live intermediates
  during evaluation of that expression, times their element width; plus
- the sum of the retained root widths, one per tiled expression.

A conservative over-estimate is acceptable and preferred: it yields a smaller
tile, which is safe. A node-count-times-width upper bound is **not** acceptable
— for this model it gives ~1 MB against a measured 20 KB, which would collapse
the tile to uselessness.

### 2. Size the tile to a cache budget

`tile_rows = clamp(budget / live_set_bytes_per_row, min, max)`.

Round to a power of two or a multiple of 64 so partial tiles stay cheap. Keep
`SEMBLA_EVAL_TILE_ROWS` as an override.

State the budget as a named constant with its reasoning — PRD 0001 assumed
32,768 bytes of L1 data cache. Do not detect cache size at runtime: it varies by
platform and would make tile size, and therefore the *parallel partitioning*, a
function of the host. Partitioning must derive from row index and tile size
alone, and tile size must be a function of the model. A host-dependent tile size
would not change results — PRD 0001's tests prove tile size is
result-invariant — but it would make performance evidence incomparable between
machines, which is nearly as bad.

**The demographic model must not regress.** Report the derived tile size for it;
if the formula does not land near 1,024, the formula is wrong, not the constant.

### 3. Threshold on work, not rows

Replace `row_count >= TICK_TILE_THRESHOLD` with an estimate of work per tick —
rows times tiled-node count is the obvious proxy. Calibrate the cut-off against
`threading_spike`'s measured crossover rather than re-deriving it: guards turn
positive between 131,072 and 262,144 rows, the racing clock earlier still.

Report the threshold decision for both benchmark models in §4.

### 4. Add a second benchmark shape

This is the part that stops the problem recurring. Add one fixture with a
deliberately different shape — **many views over moderate rows**, which is
precisely the shape the current tile constant would mishandle. Twenty to sixty
views over a few hundred thousand rows.

It is a performance canary, not a semantic addition: it may be synthetic, and it
does not need to be scientifically meaningful. Give it a name that says so.

Measure both models before and after. A change that helps the demographic model
and hurts the new one has failed.

### 5. Update the measurement protocol

`docs/prds-host-evaluator-performance/README.md` carries the protocol every
folder inherits. Amend it: performance work is measured on **at least two model
shapes**, and a change that improves one while regressing the other is reported
as such rather than averaged.

`DECISIONS.md` §M1 says optimisation is scoped from direct measurement. Extend
it: measurement on a single model shape is not sufficient evidence for a
constant that every model will use.

## Allowed files

- `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-runtime/src/executor.rs` — only if the live-set walk needs it
- `crates/**/tests/**` (tests only)
- `fixtures/**` — the §4 canary model only
- `docs/prds-host-evaluator-performance/README.md` — the §5 protocol amendment
- `DECISIONS.md` — the §M1 extension only
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2.

## Non-goals

No change to what tiling computes — PRD 0001's determinism tests across tile
sizes must pass unchanged, and they are what makes this PRD safe. No change to
worker count or partitioning. No new dependencies. No runtime cache detection.
No IR, Lean, CUDA, or CLI changes.

## Acceptance criteria

1. Tile size is derived from the model's live set; the derivation is tested
   against at least three model shapes with hand-checked expected magnitudes.
2. The derived tile size for `demographic_slots` is near 1,024, and its measured
   performance does not regress.
3. The threshold is work-based and its cut-off cites `threading_spike`.
4. A canary fixture per §4 exists and is measured.
5. PRD 0001's tile-size and worker-count determinism tests pass unchanged.
6. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
7. `cargo test --locked` and `scripts/check-rust.sh` green.
8. Protocol and §M1 amendments per §5 are present.
9. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

**This PRD is not expected to make anything faster.** Its purpose is that the
existing gains survive contact with a differently-shaped model. A result of
"demographic unchanged, canary improved" is the target; "demographic unchanged,
canary unchanged" would mean the canary is not stressing what it was built to
stress, and the canary should be reshaped rather than the result accepted.

It is low-risk for a specific reason worth stating: PRD 0001 already requires
identical output across three tile sizes, so **changing how the tile size is
chosen cannot change results**. The blast radius is performance only.

The most likely way to get this wrong is §1 — an over-conservative live-set
bound that shrinks the tile until dispatch stops being amortised, quietly
undoing PRD 0001. That is why §2 requires reporting the derived size for the
demographic model against the known-good 1,024.

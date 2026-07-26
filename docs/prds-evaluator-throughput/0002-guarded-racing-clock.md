# PRD 0002: Skip the racing-clock `ln` for candidates that cannot fire

## Context

Read `docs/prds-evaluator-throughput/README.md` first; its constraints bind.

`executor.rs` line 909 computes a racing clock for every enabled candidate:

```rust
let race_time = exp_f64(seed, tick, validated.rule_word, entity_id, 0, lambda);
if race_time.partial_cmp(&model.model().dt) != Some(Ordering::Less) { continue; }
```

`exp_f64` is `-uniform_f64(..).ln() / lambda`. In the post-0004 profile `log` is
554 of 6088 samples — the largest non-allocation item — and after the previous
folder's work it is 62% of what remains.

Almost all of it decides a foregone conclusion. Every hazard in the benchmark
model is a constant (two literals, eight parameters), and at those rates
**97.5% to 99.9% of enabled candidates never fire**:

| hazard | λ | fire probability |
|---|---:|---:|
| mortality_young | 0.001 | 0.100% |
| emigration_rate | 0.002 | 0.200% |
| internal_departure | 0.0025 | 0.250% |
| mortality_adult | 0.003 | 0.300% |
| mortality_old | 0.012 | 1.193% |
| internal_arrival | 0.018 | 1.784% |
| overseas_arrival | 0.020 | 1.980% |
| birth_rate | 0.025 | 2.469% |

In exact arithmetic, `-ln(u)/λ < dt` ⟺ `u > exp(-λ·dt)`. Because λ is constant
per transition, that threshold is **one `exp` per transition per tick** instead
of one `ln` per row.

## The trap, and why the naive form is rejected

Substituting the threshold test outright would make the firing decision depend
on this platform's `exp` agreeing with this platform's `ln`.

`crates/sembla-runtime/examples/ln_threshold_spike.rs` found no disagreement —
over 2M real draws per hazard, and walking 4,096 ULPs either side of each
threshold, which is adversarial by construction. **That is not sufficient.**

`rng::exp_f64`'s `f64::ln` is `DECISIONS.md` §E7's documented exemption: it is
the *platform's* `ln`, not the pinned `libm`. §E7 exists because CI found a
one-ULP cross-platform difference between macOS/aarch64 and Linux/x86_64 in
exactly this family of functions — and the differential harness runs on Linux.
A clean result on one platform proves nothing about the other.

## Goal

The `ln` is skipped for candidates that provably cannot fire, without the
firing decision ever being made by anything other than the comparison the oracle
makes today.

## Specification

### 1. The threshold is a filter, never the decision

Per transition per tick, compute `threshold = exp(-λ · dt)` once, and from it a
conservative bound `lo = threshold · (1 − margin)`.

Per candidate:

- if `u < lo` → cannot fire. Skip the `ln`.
- otherwise → compute `race_time` exactly as today and compare exactly as today.

Every decision near the boundary is still made by `ln`, so the result is
**bit-identical by construction on any platform** and §E7's cross-platform
question never arises. That is the whole point of this shape; do not simplify it
into the naive substitution.

### 2. Choose the margin so the claim is structural

`1e-12` relative is roughly 4,500 ULPs against the ~1 ULP at issue, and the
spike measured it costing 15,037 `ln` calls out of 5,000,000. State the chosen
margin and justify it as *provably* wider than any plausible `exp`/`ln`
disagreement — an argument, not a measurement.

The margin must be a named constant with a comment explaining what it protects
against, not an inline literal.

### 3. Handle the degenerate hazards deliberately

Two transitions declare `hazard = 1e300`, so `threshold = exp(-1e300)` is
exactly `0.0`, and `uniform_f64` excludes zero. They always fire, provably,
with no draw at all. Both have `contests = 0`, so their `race_time` is used only
for the `< dt` test and never for contest ordering.

Whether to special-case this is the implementer's call, but it must be a
recorded call. If taken, note that the RNG is counter-based, so *not* drawing
perturbs no other draw — §E1's coordinate purity is what makes this safe, and
that reasoning belongs in the notes.

Where a transition **is** contested, the firing candidate still needs its exact
`race_time` for the argmin in §E3. The skip applies only to non-firing
candidates. Do not skip a draw whose value is consumed.

### 4. Preserve every diagnostic

Error types, messages, and the `EntityIdOverflow` path are unchanged. The
per-row `entity_id` conversion and its overflow check must still happen for
every enabled candidate, whether or not its `ln` is skipped.

### 5. Measure under the README protocol

Five runs each side, fastest uncontended run as the headline, user time primary,
in-run. Report the fraction of enabled candidates for which `ln` was computed,
per transition — that number is the evidence the filter works, and a value near
100% means the margin is far too wide.

## Allowed files

- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-runtime/src/rng.rs` — only if a split of `exp_f64` into draw and
  transform is genuinely required; prefer leaving it intact
- `crates/sembla-runtime/tests/**` (tests only)
- `docs/evidence/**` (new evidence only)
- `docs/prds-evaluator-throughput/README.md` (status notes only)

## Non-goals

**No change to the RNG.** Not the algorithm, not the round count, not the
uniform construction. That is the deferred decision in the README and it
regenerates every golden in the project; this PRD regenerates none.

No change to `exp_f64`'s result for any input whose value is used. No parallel
work — that is 0001. No IR, Lean, CUDA, or CLI changes. No new dependencies.

## Acceptance criteria

1. `ln` is computed only for candidates at or above `lo`; a test demonstrates
   the filter both admits and rejects.
2. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`. `git diff --stat` shows none of them.
3. A test asserts the firing set is unchanged for a hazard sweep spanning the
   model's range, including a degenerate `1e300` hazard and a contested
   transition.
4. Diagnostics unchanged, including the entity-id overflow path.
5. §2's margin is a named constant with its protective argument stated, and §3's
   choice is recorded in the implementation notes.
6. Before/after under the README protocol, plus the per-transition fraction of
   candidates for which `ln` was computed.
7. `cargo test --locked` and `scripts/check-rust.sh` green.
8. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

The spike measured 12.6× on the draw path in isolation — 23.6 ms to 1.9 ms at
5M rows, with `ln` computed for 15,037 rows instead of 5,000,000. End-to-end
this is worth roughly the `log` share of the profile, so single-digit percent
before 0001 lands and more after, since threading shrinks everything around it.

Two things this PRD is *not* trying to buy. It does not reduce the number of
Philox draws — `rng_batch_spike` shows those are near the scalar limit at
~10 ns and are not a branch-prediction artefact. And it does not touch the
`1e300` transitions' Philox cost unless §3 is taken.

The reason to do it early is not size. It is that the change is completely
independent of the evaluator's shape, so it neither blocks nor is blocked by
tiling, and its correctness argument is self-contained.

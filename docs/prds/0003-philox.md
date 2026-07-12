# PRD 0003: Philox RNG by coordinates

## Context

Sembla's reproducibility design (`DESIGN.md` §4.2, §5.2, §5.3) rests on one
mechanism: every random draw is a **pure function of coordinates**
`(seed, tick, rule_id, entity_id, draw_idx)` via the counter-based
Philox4x32-10 generator (Salmon et al., *Parallel Random Numbers: As Easy as
1, 2, 3*). No stateful streams anywhere. This makes randomness
order-independent by construction and enables exact common random numbers
across policy scenarios.

## Goal

A `rng` module in `sembla-runtime` implementing Philox4x32-10 with the
coordinate-keying scheme, uniform and exponential samplers, and
known-answer tests.

## Specification

- Implement Philox4x32-10 from the paper (or port a reference
  implementation; a dependency-free local implementation is preferred so the
  bit behavior is frozen by our own tests, not a crate's version).
- Coordinate mapping (frozen once tests land):
  `key = (seed_lo, seed_hi)` from the u64 seed; counter =
  `(tick: u32, rule_id: u32, entity_id: u32, draw_idx: u32)`. Document the
  exact packing in rustdoc.
- `fn draw_u32x4(seed: u64, tick: u32, rule_id: u32, entity_id: u32,
  draw_idx: u32) -> [u32; 4]`
- `fn uniform_f64(...) -> f64` in the open interval (0, 1) — never exactly
  0.0 or 1.0 (document the conversion; use 53-bit mantissa construction from
  two u32 lanes).
- `fn exp_f64(..., lambda: f64) -> f64` = `-ln(U)/λ` (the racing-clock
  sampler, `DESIGN.md` §4.3). λ ≤ 0 returns `f64::INFINITY` (transition never
  fires) — this is the defined semantics, not an error.
- Known-answer test vectors for Philox4x32-10 from the Random123 reference
  distribution (the standard KAT values for counter/key all-zeros and
  all-ones cases) hardcoded in tests.

## Non-goals

Any other distribution (normal, etc. — added when a model needs them),
stateful RNG adapters, GPU implementation (PRD 0012 reimplements Philox in a
shader and cross-checks against this module's vectors).

## Acceptance criteria

1. Known-answer tests pass against the Random123 reference vectors for
   Philox4x32-10.
2. Purity test: calling every public function twice with identical arguments
   yields identical results; calls with any single coordinate changed yield
   different results (statistically — test over 1000 coordinate pairs, assert
   no collisions of the full 128-bit output).
3. `uniform_f64` over 1e6 draws: all outputs strictly in (0,1); mean within
   0.5 ± 0.002; a 100-bucket histogram has no bucket deviating more than 5%
   from expectation.
4. `exp_f64` over 1e6 draws at λ=2.0: sample mean within 0.5 ± 0.005;
   λ=0.0 returns `INFINITY`.
5. No use of `rand` or any external RNG crate in `sembla-runtime`
   (enforced by a test or documented grep in `scripts/check.sh`).
6. Rustdoc documents the coordinate packing exactly, and notes the CRN
   property (`DESIGN.md` §5.3): identical coordinates ⇒ identical draws
   across scenario variants.

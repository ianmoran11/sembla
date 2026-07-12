# PRD 0008: SIR end-to-end at 1M agents

## Context

The v0.1 flagship model (`DESIGN.md` §6, §9): an SIR epidemic over a
synthetic population of ~1M people with static employer assignment, infection
mediated by the employer group-by aggregate (the §7 `infect` pattern), run on
the full PRD 0002–0007 stack. This PRD also establishes the v0.1 success
criteria for determinism and CPU performance.

## Goal

A checked-in SIR model IR, a deterministic synthetic-population generator, a
CSV results pipeline, and passing determinism/performance/epidemiology
acceptance tests.

## Specification

- `examples/sir.json`: one box; tables `person` (attrs: `health:
  Enum{S,I,R}`, `employer: Ref{employer}`) and `employer` (may be
  attribute-less). Transitions: `infect` — table `person`, guard
  `health = S`, hazard `β × count(person sharing employer where health = I)
  / workplace_size_normalizer` (implementer picks the standard
  frequency-dependent form; document the chosen formula in the file and
  README) — and `recover` — guard `health = I`, constant hazard γ. Contested
  resources: none needed (both transitions self-write different rows... note
  `infect` and `recover` guards are disjoint, so no same-cell conflict).
- Synthetic population generator in `sembla-cli` (`sembla synth-pop`):
  N persons, E employers, workplace sizes drawn from a documented
  distribution (e.g. lognormal-ish bucketing), initial infections I₀ seeded
  — **all generated deterministically from a u64 seed via the PRD-0003 RNG**
  (reserve a synthetic `rule_id` namespace for generation draws; document
  it). Output: a population file the runtime can load (format:
  implementer's choice, documented; CSV or a simple binary).
- `sembla run examples/sir.json --population pop.bin --seed N --ticks K
  --out results.csv`: per-tick rows `tick, S, I, R, fired_infect,
  fired_recover, deferred_total`, plus a final line to stdout with the
  SHA-256 of the results file and of the final state.
- A `--dt` override flag (dt is semantic — §4.3 — so it must be visible).

## Non-goals

Realistic Australian demography (v0.1 explicitly accepts a toy synthetic
population — §9), calibration, plotting, birth/death, multi-box (PRD 0009).

## Acceptance criteria

1. `cargo test --workspace` passes.
2. **End-to-end determinism (v0.1 success criterion #2)**: an integration
   test generates a 100k-person population and runs 100 ticks twice from
   scratch; results-file hashes and final state hashes are identical. A
   *different seed* produces a different hash.
3. **Epidemic sanity**: at parameters with basic reproduction clearly > 1
   (documented calculation in the test), starting from I₀ = 100 of 100k:
   S is monotonically non-increasing, R monotonically non-decreasing,
   I rises then falls, and the final attack rate exceeds 50%; with β = 0 the
   infection count never grows.
4. **Lumping is live**: the run at 1M persons evaluates the infection
   pressure via the group-by aggregate path (assert via the PRD-0005 cache
   counters that per-employer aggregation ran once per tick, not per row).
5. **Performance floor (v0.1 success criterion, generous)**: released-mode
   (`--release`) run of 1M persons completes ≥ 10 ticks at ≤ 2 seconds per
   tick on the build machine, measured and printed by a
   `cargo bench`-style or integration harness (record the number in the test
   output; the threshold is a regression tripwire, not a target).
6. `docs/examples/sir.md` documents: the model, the hazard formula, how to
   generate the population, run, and verify the determinism property with
   two shell commands.

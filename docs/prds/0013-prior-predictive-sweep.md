# PRD 0013: Prior-predictive sweep runner

## Context

Prior-predictive checking (`DESIGN.md` §3, §9, §10.4) means: draw parameter
vectors θ⁽ᵏ⁾ from the declared priors, run the **same IR** under each draw,
and look at the distribution of outputs. Everything needed already exists:
first-class params with prior metadata (PRD 0002), θ-parameterized runs
(PRDs 0005–0008), and coordinate-keyed randomness (PRD 0003). This PRD is
CLI plumbing plus prior samplers — no new semantics. It is also the
foundation black-box calibration methods (ABC/SBI) will later drive.

## Goal

`sembla sweep` runs K prior draws of a model reproducibly from a single
seed, emitting per-draw results, a parameter manifest, and cross-draw
summary quantiles.

## Specification

- **Prior samplers** in `sembla-runtime` for the PRD-0002 families —
  Uniform (direct), Normal (inverse-CDF or Box–Muller from PRD-0003
  uniforms; document the choice, it's frozen by tests), LogNormal
  (exp of Normal). Int-typed params reject priors at validation if not
  already rejected in PRD 0002 (check; add if missing).
- **Reserved coordinate namespace** for parameter draws (document in
  rustdoc and `docs/prds/README.md` conventions): `rule_id = u32::MAX`,
  `entity_id` = the parameter's declaration index, `tick` = the draw index
  k, `draw_idx` = sampler-internal counter. Consequences (tested): draw k
  is identical whether the sweep has K=10 or K=10,000 draws, and parameter
  draws can never collide with simulation draws.
- `sembla sweep <model.json> --population ... --seed S --draws K --ticks T
  --out <dir>/`:
  - `<dir>/manifest.csv`: one row per draw — k, each parameter's sampled
    (or default, if prior-less) value.
  - `<dir>/draw_<k>.csv`: the standard PRD-0008 per-tick results for run k
    (run under seed S — CRN across draws: identical simulation coordinates
    ⇒ identical shocks, so output variation across draws is attributable to
    θ alone; note this in the docs).
  - `<dir>/summary.csv`: per tick, the 5/25/50/75/95% quantiles of each
    aggregate column across draws.
  - stdout: SHA-256 of the manifest and of the summary.
- `--params <file.json>` may pin a subset of parameters (pinned params are
  not drawn; recorded as pinned in the manifest header).
- Draws run sequentially in k order (single-threaded oracle discipline —
  parallelism is a v0.2 concern).

## Non-goals

Posterior inference / calibration algorithms (open question, `DESIGN.md`
§10.4), plotting, adaptive sampling, parallel execution, new IR constructs.

## Acceptance criteria

1. `cargo test --workspace` passes; all earlier PRDs' tests still pass.
2. Sampler statistical tests: 1e5 draws per family match their analytic
   mean/variance within documented tolerances; LogNormal(μ, σ) checked via
   the log-domain mean.
3. Reproducibility: the same `sembla sweep` invocation twice ⇒ byte-identical
   manifest, summary, and every per-draw file. Changing S changes them.
4. Namespace stability test: draw k=3's θ from a K=5 sweep equals draw k=3's
   θ from a K=50 sweep (same seed).
5. CRN-across-draws test: two draws with (artificially) identical θ produce
   identical per-draw results files.
6. Pinning test: `--params` pinning `gamma` yields a manifest where `gamma`
   is constant and marked pinned while `beta` varies.
7. End-to-end on the PRD-0008 SIR model at 100k persons, K=20, T=50
   completes and the summary quantile bands are monotone (5% ≤ 50% ≤ 95%
   columnwise) — asserted in an integration test.
8. `docs/examples/sir.md` gains a prior-predictive section: commands, what
   the summary shows, and the CRN-across-draws caveat.

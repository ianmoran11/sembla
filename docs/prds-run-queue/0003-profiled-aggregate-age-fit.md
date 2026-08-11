# PRD 0002: Profiled aggregate migration-age fit

## Context and dependencies

PRD 0001 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first; `DECISIONS.md` §O3 and the
standard-library/quarantine constraints bind. The frozen prior registry is
`data/abs/params/priors.json`; it is read-only and its hash is an input.

## Context

The existing `data/abs/gravity_fit.py` fits fifteen positive spatial parameters
per year from all 56 O-D cells while holding `peak_months = 360` and `k = 1e-5`.
The all-age O-D table cannot identify the age profile because its origin effect
is absorbed by each origin push. The separate
`interstate_state_age_sex.csv` extract contains 2 sexes × 16 age bands for
arrivals and departures in each state/year, but its totals conflict with the O-D
vintage in 2020. Conditional within-state/direction composition retains the age
information without forcing those totals to agree.

The retained NPE varied all seventeen parameters and accepted only three yearly
posteriors. This PRD implements the declared two-dimensional aggregate
alternative without changing any retained parameter or evidence artifact.

## Goal

A deterministic standard-library fit estimates one pooled positive
`peak_months, k` pair over 2010–2024 while re-profiling all fifteen annual
spatial parameters for every candidate, emits robust uncertainty and complete
source-level residuals, and writes separate full parameter files under
`data/abs/params/profiled/`.

## Specification

### 1. New profile fitter

Add `data/abs/profile_fit.py`. It may import pure helpers from sibling
`data/abs/` modules but must not import a Sembla crate, parser, runtime API or
third-party package.

Use transformed coordinates:

```text
z_peak = log(peak_months)
z_k    = log(k)
```

so every evaluated candidate is finite and positive. Read prior centres and
log-space spreads from the frozen prior registry named in Context; do not
duplicate them as literals in a second registry.

For each candidate `φ`:

1. recompute exact monthly age-weighted origin exposures using the same
   birth-month spreading and twelve-tick ageing convention as
   `gravity_fit.py`;
2. re-run the existing Poisson/IRLS spatial fit for every run year 2010–2024;
3. construct predicted departure compositions from age-sex origin exposure and
   the age profile;
4. construct predicted arrival compositions by mixing origin exposure through
   the candidate year's re-profiled O-D rates; and
5. evaluate the conditional multinomial log likelihood independently for every
   `(year, state, direction)` group after normalizing its 32 sex/age cells.

A group with a positive observed total but a non-positive/non-finite predicted
total is a hard error. Preserve source counts in residual output, but never fit
the raw composition total jointly with the O-D total.

### 2. Deterministic optimization and identification

Normalize transformed coordinates by the declared prior metadata:

```text
x_peak = (log(peak_months) - prior_location_peak) / prior_spread_peak
x_k    = (log(k)           - prior_location_k)    / prior_spread_k
```

The frozen search domain is `[-8, 8]²`. Minimize negative conditional log
likelihood divided by the total observed composition count; retain unnormalized
log likelihood for inference/reporting.

Use this exact deterministic pattern search:

- exactly nine unique starts, the Cartesian grid `{-2, 0, 2}²` (the centre is
  not run a second time);
- initial normalized step `1.0`;
- at each iteration evaluate the eight axial/diagonal neighbours at the current
  step that remain inside the domain;
- move to a candidate only when normalized objective improves by more than
  `1e-12`; ties use lexicographic `(x_peak, x_k)` order;
- when no candidate improves, halve the step;
- converge when step is below `1e-6`;
- fail that start after 20,000 objective evaluations; and
- require all nine starts to converge, terminal normalized objectives to agree
  within `1e-10`, and terminal coordinates to agree within L-infinity `1e-3`.
  Otherwise status is `multimodal_or_unstable`.

For one-dimensional 95% profile intervals, use the exact one-degree-of-freedom
likelihood-ratio cutoff `3.841458820694124` (log-likelihood drop
`1.920729410347062`). At each fixed coordinate, re-optimize the other coordinate
with the same bounded pattern rule, starting at the full optimum. Bracket each
endpoint outwards in normalized increments of `0.25`; use the first outward
likelihood-ratio crossing, then bisect to normalized coordinate width below
`1e-5` or 80 iterations. If no crossing occurs before
`-8` or `8`, report that side open; never clamp it into a closed interval.
Retain a two-dimensional profile surface on the fixed `[-8,8]²` grid with step
`0.25` and mark the exact two-degree-of-freedom 95% cutoff
`5.991464547107979`.

Calculate year-cluster robust covariance in normalized coordinates without a
bootstrap: use central finite-difference cluster scores at `h = 1e-4`, a central
finite-difference total observed information
`H = -∂² log L / ∂x∂xᵀ` at `h = 1e-3`, and sandwich
`H⁻¹ (Σ score_y score_yᵀ) H⁻¹ × 15/14`. Report the information matrix, cluster scores,
finite-difference steps, covariance and transformed intervals using
`NormalDist().inv_cdf(0.975)`. Singular, non-finite or negative-diagonal
covariance is an identification failure. A
naïve inverse Poisson Hessian must not be presented as complete uncertainty.

Identification requires all of:

- the nine-start stability gate;
- a strict interior optimum;
- finite two-sided profile intervals for both parameters; and
- finite valid year-cluster robust covariance.

This gate is frozen before results. Failure produces `identified: false` and
names each failed condition; it does not fail artifact generation or invite
threshold tuning.

### 3. Artifacts

Write canonical UTF-8/LF/final-newline artifacts:

- `data/abs/params/profiled/<year>.json`: complete 377-parameter files for
  2010–2024, with the fifteen re-profiled spatial slots and pooled
  `peak_months, k` changed; every fixed slot equals the existing annual source
  exactly;
- `data/abs/params/profiled/fit-report.json` with format
  `sembla.abs-profile-fit/v1`;
- `data/abs/params/profiled/input-hashes.json` naming every consumed extract,
  prior registry and gravity implementation hash; and
- a concise generated Markdown summary under the same directory.

The report must include:

- optimizer starts and convergence;
- pooled point estimate, fixed-grid/profile intervals and year-cluster robust
  covariance/intervals;
- identification gate results;
- all fifteen annual spatial estimates;
- observed/predicted conditional composition and signed/Pearson-style residual
  for every state/direction/sex/age cell;
- annual O-D observed/expected cells and deviance after re-profiling;
- gravity-only versus profiled O-D WAPE; and
- the separately published 2020 margin conflict, explicitly validation-only.

Do not edit `data/abs/params/gravity/**`, `data/abs/params/20*.json` or any
retained calibrated evidence.

### 4. Synthetic and regression tests

Add `data/abs/tests/test_profile_fit.py` covering:

- recovery of known positive `peak_months, k` from a synthetic multi-year
  composition fixture with re-profiled spatial nuisance parameters;
- all nine starts, neighbour/tie order, step halving, convergence and
  20,000-evaluation failure behavior exactly as frozen;
- exact profile cutoff/bracketing/bisection and finite-difference cluster
  covariance on a hand-derived quadratic likelihood;
- an intentionally flat age composition reporting open intervals and
  `identified: false`;
- strict positivity under every optimizer/profile/covariance evaluation;
- at a fixed candidate, multiplying every observed composition count in one
  group by a common source-scale factor leaves observed/predicted shares
  unchanged while its log-likelihood contribution receives the expected weight;
- the 2020 raw total conflict never entering the count likelihood;
- fixed 360/17 parameter split in every output file;
- deterministic byte equality across two runs; and
- malformed/missing age bands, sexes, states, directions, years and zero
  predicted denominators failing with explicit messages.

Do not weaken the existing gravity tests. Add a focused test proving that each
candidate invokes/reuses the authoritative gravity fit rather than a copied
spatial solver.

### 5. Regeneration and documentation

- Extend `scripts/check-abs-data.sh` so an offline regeneration includes the
  profiled directory and fails on nondeterministic or stale bytes.
- Create `docs/guides/australian-population-aggregate-calibration.md` describing
  the profile likelihood, conditioning, uncertainty and non-identification
  gate. State that this is an experiment until PRD 0004's aggregate/micro
  evidence and selection report land.
- Link the guide from the existing calibration guide without changing its
  retained-run account.

## Allowed files

- `data/abs/profile_fit.py` (new)
- `data/abs/gravity_fit.py` (API extraction only; existing CLI/output exact)
- `data/abs/tests/test_profile_fit.py` (new)
- `data/abs/tests/test_gravity_fit.py` (API/CLI parity only)
- `data/abs/tests/fixtures/profile_fit/**` (new, if needed)
- `data/abs/params/profiled/**` (new)
- `scripts/check-abs-data.sh`
- `docs/guides/australian-population-aggregate-calibration.md` (new)
- `docs/guides/australian-population-calibration.md` (link only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No micro-runs, NPE training, deterministic expectation model or production
  parameter selection; PRDs 0003–0004 own those.
- No scientific/numerical change to the gravity model or its existing CLI/
  outputs; only a tested reusable candidate-exposure API extraction is allowed.
  No migration-hazard, target-ledger, extract, prior, current annual parameter
  file or retained-evidence change.
- No dyadic O-D affinity, free O-D matrix, annual age parameters or
  fertility/mortality fitting.
- No third-party Python dependency or network access.

## Acceptance criteria

1. Full repository, ABS, allowlist and diff checks from the README pass.
2. The fit uses all fifteen years, all 56 annual O-D cells and every complete
   state/direction/sex/age composition group, with no raw cross-source total
   reconciliation.
3. Every candidate re-profiles the fifteen spatial parameters through the
   authoritative gravity implementation; no copied solver exists.
4. The exact nine-start normalized pattern search, fixed profile grid/cutoffs,
   endpoint algorithm and finite-difference year-cluster covariance run with the
   frozen identification gate whichever result is obtained.
5. All fifteen complete parameter files retain every fixed slot exactly and are
   byte-reproducible; existing gravity/annual/calibrated files are unchanged.
6. Cell-level composition and O-D residuals, O-D WAPE comparison and the 2020
   validation-only conflict are retained in the v1 report.
7. Synthetic recovery, flat-likelihood non-identification, malformed-input,
   fixed/free and two-run byte tests pass.
8. Documentation calls the result an experiment pending PRD 0004 and makes no
   claim that profile convergence alone establishes microsimulation validity.

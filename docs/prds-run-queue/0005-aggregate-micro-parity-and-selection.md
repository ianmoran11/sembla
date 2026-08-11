# PRD 0004: Aggregate/micro parity and calibration selection

## Dependencies

PRDs 0001–0003 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first. This PRD is the evidence
gate between a fast aggregate companion and a changed maintained calibration
recommendation. It must report an adverse result without tuning the experiment.

## Context

PRD 0002 supplies a pooled profile estimate and an identification verdict. PRD
0003 supplies analytic expected raw counts and ratio-of-expectations
compositions. The count expectations may close exactly under the model's
exogenous row-local semantics; normalized compositions are nonlinear ratios and
are only an approximation to the mean of a stochastic ratio.

The retained broad NPE result is already known to be unreliable year by year.
The replacement does not need NPE to “approve” the direct spatial fit. It does
need retained evidence that the aggregate companion matches the simulator for
raw quantities and a predeclared rule for whether the pooled age estimate or its
prior centre is selected.

## Goal

Run a fixed replicated aggregate-versus-micro experiment, classify every output
as exact-parity/approximated/failed, select a positive aggregate parameter path
without using held-out targets for selection, and publish a reproducible
comparison against gravity-only and retained NPE evidence.

## Specification

### 1. Fixed parity design

Add `data/abs/aggregate_calibrate.py` and
`scripts/calibrate-australian-population-aggregate.sh`. The script must record
all commands, versions, input hashes, semantic seeds, wall time and backend.
CPU is the required path; CUDA may be reported only after the existing exact
backend-differential gate.

Use exactly these five cases at one-in-a-hundred scale:

| Case | Run year | Parameter point |
|---|---:|---|
| A | 2010 | gravity spatial fit + age prior centre |
| B | 2010 | PRD 0002 profiled point |
| C | 2020 | gravity spatial fit + age prior centre |
| D | 2020 | PRD 0002 profiled point |
| E | 2024 | PRD 0002 profiled point |

Generate the required 2020 and 2024 starting states by the existing verified
annual chain using the same parameter path whose case is tested. For each case,
the expectation runner and all micro replicates consume the exact same start
state and annual parameter file.

Run 64 independent semantic seeds per case. Seeds are derived by a documented
SHA-256 coordinate containing model/plan hash, state hash, parameter hash, run
year, case ID and replicate index 0–63. Do not stop early, add cases or increase
replicates after looking at a failed cell. An interrupted run resumes missing
coordinates without replacing completed outputs.

### 2. Raw-count parity gate

Use exactly the ordered 820 IDs in
`data/abs/contracts/raw-parity-v1.json` from PRD 0003. Verify its format,
ordered-ID hash, model hash and summary-contract hash before any run. The family
contains all declared ending stock cells, all 418 transition firings and both
composition numerator/denominator families; it contains no duplicated headline
sum. A missing, extra or reordered ID fails the case. Structural zeros remain in
all 820 multiplicity slots.

For each ID with nonzero empirical variance, calculate its standardized mean
error using the empirical standard error and compare it with:

```text
NormalDist().inv_cdf(1 - 0.001 / (2 × 820))
```

This is the frozen two-sided Bonferroni critical value for familywise error
0.001. Do not store a rounded substitute or change `820` after observing zeros.
For a zero-variance ID, require
`abs(expectation - constant) ≤ 1e-12 × max(1, abs(expectation))`.
Comparisons use unrounded binary64 calculations; canonical JSON uses Python's
shortest round-tripping finite-number representation and no separate “report
precision.”

A case passes raw parity only when every one of the 820 finite quantities
passes. Report standardized errors, intervals, zero-variance differences and
the maximum-error ID. A non-finite expectation or replicate statistic is a hard
case failure.

Normalized compositions are reported separately as
`ratio_of_expectations_approximation`. Compare their micro mean, empirical bias
and interval, but do not call them exact and do not let a ratio-only discrepancy
overturn otherwise exact raw-count closure. A non-finite or zero denominator is
still a hard model/report failure.

### 3. Selection rule frozen before held-out evaluation

Write `data/abs/params/aggregate/selection.json` with format
`sembla.abs-aggregate-selection/v1` and complete selected annual parameter
files.

The rule is:

1. if any of the five raw-count parity cases fails, select the existing positive
   gravity spatial files with `peak_months, k` at their declared prior centres
   and set `aggregate_companion_status: failed`; retain the discrepancy report;
2. if raw parity passes but PRD 0002's identification gate is false, make the
   same prior-centre selection with
   `age_profile_status: aggregate_unidentified`;
3. if raw parity and identification both pass, select PRD 0002's profiled files
   and set `age_profile_status: aggregate_identified`.

In every branch, the fifteen spatial quantities are direct annual gravity/profile
optima and every parameter is finite and positive. No NPE coordinate enters the
selected files. Do not use held-out stocks, old NPE performance or a preferred
scientific narrative to choose a branch.

Every selected file contains all 377 parameters and preserves all 360 fixed
annual slots exactly. Existing annual, gravity and profiled files remain
unchanged.

### 4. Independent scientific comparison

After selection, run 32 paired full 2010→2025 one-in-a-hundred chains under:

- gravity + prior-centre age profile;
- the selected aggregate path; and
- the retained NPE point path where its existing parameter artifacts are
  available.

Use common random numbers between comparable transition identities and
independent replicate coordinates across pairs. This comparison is reporting,
not selection. Report:

- fitted and held-out metrics separately;
- O-D WAPE and deviance;
- fitted migration-composition deviance;
- held-out single-year stock WAPE/MAE/RMSE;
- terminal national and state drift;
- paired means, standard errors and intervals; and
- runtime/simulation count relative to the old NPE sweep.

If selected aggregate parameters perform worse on held-out evidence, retain and
state that result. Do not fall back post hoc or change the selection rule.

### 5. Evidence artifact

Create `docs/evidence/demographic-spine-foundation/aggregate-calibration/` with:

- `README.md` explaining the predeclared experiment and result;
- canonical `parity-report.json`, `selection.json`, `chain-comparison.json`,
  `execution.json` and `SHA256SUMS`;
- per-case raw and normalized residual tables;
- exact reproduction commands; and
- a status of `aggregate_selected`, `aggregate_age_unidentified` or
  `aggregate_parity_failed` derived mechanically from §3.

Raw per-replicate run files may remain outside git when too large, but their
hash inventory, reduced summaries and commands must be retained. An unavailable
required local executable is a failed PRD environment, not scientific evidence.
Paid hardware is not required.

### 6. Maintained workflow

Update the aggregate-calibration guide with the measured parity classification,
selection branch and exact reproduction workflow. Update the existing
Australian calibration guide to distinguish:

- the retained historical NPE experiment; and
- the newly selected aggregate-first maintained recommendation.

Do not delete the old NPE commands or present their evidence as invalid; mark
them historical/alternative. Link the new evidence from the Australian model
document.

### 7. Tests

Add tests covering:

- deterministic semantic seed derivation and resume without coordinate changes;
- exact 820-ID/hash contract enforcement before values are filtered;
- the frozen Bonferroni critical-value expression and zero-variance `1e-12`
  relative/absolute handling;
- raw-count failure, unidentified-age and identified-age selection branches;
- no held-out metric accessible to the selection function;
- complete 377-slot selected files with fixed slots unchanged;
- all selected parameter values finite and positive;
- ratio outputs always labelled approximate;
- deterministic evidence reduction from a small synthetic replicate set; and
- immutable hash checks over retained NPE/gravity/profile inputs.

## Allowed files

- `data/abs/aggregate_calibrate.py` (new)
- `data/abs/tests/test_aggregate_calibrate.py` (new)
- `data/abs/params/aggregate/**` (new)
- `scripts/calibrate-australian-population-aggregate.sh` (new)
- `scripts/check-abs-data.sh`
- `docs/evidence/demographic-spine-foundation/aggregate-calibration/**` (new)
- `docs/guides/australian-population-aggregate-calibration.md`
- `docs/guides/australian-population-calibration.md`
- `docs/models/australian-population.md` (evidence link/status only)
- implementation notes/artifacts created by the managed run

## Non-goals

- No tuning after parity output, no held-out-based parameter selection and no
  claim that ratio-of-expectations is exact.
- No NPE retraining, log-space residual NPE or modification of
  `calibration/npe`.
- No model/hazard/target/IR/runtime change and no full-scale run.
- No overwrite of annual, gravity, profiled or retained calibrated files.
- No dyadic-affinity model comparison.

## Acceptance criteria

1. Full repository, ABS, allowlist and diff checks from the README pass.
2. All five fixed cases complete 64 independently seeded replicates from
   hash-identical state/parameter inputs; interruption/resume preserves
   coordinates.
3. Raw-count parity uses the complete predeclared family and familywise 0.001
   rule; normalized ratios are separately and honestly classified.
4. The mechanical three-branch selection runs without access to held-out
   results, emits complete positive parameter files and never uses an NPE
   coordinate.
5. The 32-pair full-chain comparison reports fitted and held-out evidence and
   retains an adverse result without changing selection.
6. The new evidence directory is hash-complete and reproducible; retained
   gravity, NPE, target, model and profile artifacts are unchanged.
7. Tests exercise every selection branch, multiplicity/zero-variance behavior,
   seed/resume determinism, fixed-slot fidelity and retained-input hashes.
8. Maintained docs clearly separate the historical NPE experiment from the new
   aggregate recommendation and state the selected status without overclaiming
   exact normalized summaries.

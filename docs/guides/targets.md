# Australian population targets and scoring

The Australian population target ledger is the explicit boundary between ABS
evidence and simulation output. It never feeds execution. The builder and scorer
are standard-library-only Python under `data/abs/`:

```bash
cd data/abs
python3 targets.py
python3 score.py \
  --run /path/to/run.csv \
  --targets targets/2010.json \
  --model ../../fixtures/australian-population/australian_population.hundredth.json \
  --mode fitting --recipe full --out /tmp/score.json
```

## Annual semantics

There are fifteen standard ledgers, `2010.json` through `2024.json`. Ledger `y`
contains:

- flows during the run from 30 June `y` to 30 June `y+1`; and
- ERP stocks at the ending boundary, 30 June `y+1`.

Thus the 2024 ledger contains the terminal 2025 stock without inventing a 2025
flow. Birth and death targets retain their calendar-registration-year proxy
label; migration targets are financial-year observations. The artifact does not
claim those periods are interchangeable.

The additional `2010.spatial_holdout_nt.json` ledger supports a named spatial
transfer check. Initial 2010 ERP belongs to the state artifact, not to a
post-run scoring ledger.

## Format and hashes

Every ledger has `format = "sembla.targets/v1"` and records:

- exact model-byte and `sources.json` SHA-256 digests;
- model name and required `grouped-observations` feature;
- an adjacent `execution.json` contract pinning raw model/plan bytes, canonical IR
  hash and plan-semantic hash (raw contract SHA-256
  `779a74bca8bf89d2965c09c72dea8c434645f15c1667c3e3078db80f4d55ed9a`);
- geography contract `australian_states_and_territories/v1`, with ordered
  variants `nsw,vic,qld,sa,wa,tas,nt,act`;
- hundredth scale, factor and count lattice quantum 100;
- run year, stock boundary, tick/period coordinates and source-period alignment;
- an ordered target list with observation selectors, aggregation, exact source
  value, source vintage, fitted/held-out role, projection membership and
  discretisation metadata;
- ordered `full` and `reduced` fitted target IDs; and
- source diagnostics that are evidence but not duplicate fitted controls.

Target hashes use the exact byte domain:

```text
SHA-256(b"sembla.targets/v1\0" + canonical_artifact_bytes)
```

A ledger never self-hashes. `data/abs/targets/index.json` records raw and
domain-separated hashes, byte counts, role counts and projection dimensions.
Canonical JSON uses sorted keys, two-space indentation, UTF-8 and one trailing
LF.

## Frozen target inventory

Each standard ledger carries 2,797 entries:

| family | entries | role/projection |
|---|---:|---|
| State × sex × five-year ending stocks | 336 | fitted, `full` |
| State × sex × single-year ending stocks | 1,616 | held out structurally |
| Births by state | 8 | fitted, `full` and `reduced` |
| Deaths by state × five-year event-age band | 168 | fitted, `full` |
| Overseas arrivals/departures by state | 16 | fitted, `full` and `reduced` |
| Directed interstate O-D cells | 56 | fitted, `full` and `reduced` |
| State/direction/sex/age interstate compositions | 512 | fitted, `full` |
| Derived state stock totals | 8 | fitted, `reduced` |
| Derived training-geography age profile | 21 | fitted, `reduced` |
| Derived state death totals | 8 | fitted, `reduced` |
| Derived interstate composition moments | 48 | fitted, `reduced` |

The 1,096-component `full` vector uses every raw fitted cell. The selected
165-component `reduced` vector contains eight state stocks, 21 national
five-year stocks, eight births, eight state death totals, sixteen overseas
flows, all 56 O-D cells, and 48 composition moments. The moments are female
share and the first and second moments of the published 16-band age ordinal for
each state and direction. The open `75+` category is ordinal 15; no fictitious
mean age is assigned to it.

## Observation mapping

Existing grouped views supply five-year stocks, births, overseas flows and O-D
flows. PRD 0006 adds only three sink observations:

- `population_single_year_cells` — area, sex, 12-month age band;
- `deaths_state_age_cells` — area, sex, 60-month event-age band; and
- `interstate_age_sex_flows` — previous area, area, sex, 60-month event-age
  band.

Selectors are ordered exactly like grouped CSV keys. Equality, wildcard,
exclusion and open-tail (`>=`) matches are explicit. This is necessary for ERP
and deaths `100+` and interstate `75+`; no finite key is pretended to represent
an open tail. Grouped CSVs are sparse, so an absent valid key scores as zero,
while duplicate, malformed, unknown or hash-mismatched rows fail.

## Scale and discretisation

The scorer scales model counts up by exactly 100 and never rounds ABS targets
down. A count target records both lattice quantum 100 and its nearest attainable
absolute error. For example, a target ending in 49 has a minimum attainable
absolute error of 49; one ending in 51 has 49.

Composition and moment targets preserve exact integer numerators and
denominators. Their simulation floor depends on the observed denominator, so the
artifact explicitly records `denominator_dependent_ratio` rather than inventing
a fixed floor. Source rounding resolution is separate from model
discretisation and is not mislabeled as statistical uncertainty. The extracts
publish no formal uncertainty, so none is synthesized.

## Held-out policy

Single-year stock cells are always `heldout`; the model is fitted on five-year
bands, making within-band shape a structural validation. A mixed-role ledger is
still usable for fitting: `--mode fitting --recipe full|reduced` reads only that
ordered fitted projection. Explicitly requesting a held-out target ID in fitting
mode is a hard error.

The NT variant holds out every direct NT stock/flow/composition cell and every
O-D edge touching NT. Aggregate training targets explicitly exclude NT, avoiding
validation leakage. Its fitted dimensions are 952 (`full`) and 140 (`reduced`).
Future-year rolling-origin splits are later multi-ledger validation operations;
they do not reclassify the required `y+1` calibration stock in an annual ledger.

## Preserved source discrepancies

The detailed O-D series is the fitted spatial count evidence. Separately
published all-age state margins remain diagnostics. In 2020 the worst state and
direction disagreement is NSW arrivals, **27,626** people. The ledger records
both published values and the signed difference and never adjusts an O-D cell to
force reconciliation.

Registered-death `not_stated` counts are also diagnostics. They are not assigned
to an invented age band. Reduced state death totals may use the published total,
while the full age profile uses only stated age bands.

## Scorer output

`data/abs/score.py` validates the complete target schema against exact model
bytes, exact `sources.json` bytes, family-specific source IDs and matching
release/vintage metadata, plus model-declared grouped keys. It then validates
the run manifest's
canonical IR and plan-semantic identities, feature set, tick count, model scale,
scalar-results hash, summary-observation hash, exact grouped-output inventory,
every grouped-file hash, header, typed key and unique row. Unknown enum values,
malformed integers and duplicate manifest entries fail rather than becoming
sparse zeros. It emits
canonical `sembla.target-score/v1` JSON containing:

- per-target observed value, target, signed and absolute error;
- MAE, RMSE and maximum absolute error overall and by family and role;
- fixed population-size bins and per-state residual detail;
- copied source diagnostics; and
- an ordered summary vector with target IDs, simulated values and targets.

Correlation is not used as a headline score. The summary-vector order comes
from the artifact projection, not dictionary iteration, and is stable across
runs.

## Full versus reduced prior-predictive evidence

The exact pre-measurement declaration is
`data/abs/targets/sensitivity/predeclaration.json`, SHA-256
`759b3f77887335a79865bdde48d3fecb56627f34b2acfc1cada5424956bbdadb`.
It froze all materialized prior draws, 17-parameter order, perturbations, paired
and noise seeds, 108-run count, input hashes, normalization, metrics and gates
before the first ensemble run. The measured evidence is
`data/abs/targets/sensitivity/evidence.json`, SHA-256
`9cbb5389eb7182e7d64d6b16a9bb5c5179bdf13b9d7a8353204b894b010d3403`.
It includes all 108 compact observed vectors and their shared ordered target
vectors, so every reported noise, effect, rank, correlation and gate value can
be recomputed without the uncommitted run directory. It also pins the exact
release binary, Rust source tree, measurement script, scorer, IR, plan and
execution-contract hashes. The release binary advanced during PRD 0008 (the
sweep argument parser gained `--draw-workers`; simulation semantics are
unchanged), so the complete 108-run ensemble was re-executed from an empty
cache with the new binary, SHA-256
`dc533aec372cb6b3dff07d788b1a823a42dcdd978c745b7dbd946fd0f4a6ea8a`: every run
hash, observed vector, analysis value, gate and the recommendation are
byte-identical to the first measurement, and the evidence differs only in the
pinned binary and Rust source-tree hashes. The first measurement's binary
`48dc4fa8db73d62e11f1e54f7817ac97299aa639994fe2b7972bba9b4570bbd0` remains the
recorded provenance of the PRD 0006 and 0007 evidence. Both declaration and evidence remain in one
uncommitted implementation series, so Git history alone does
not timestamp their order; the implementation-session workflow transcript is
the temporal record.

The diagnostic used three prior base points. For each free parameter it ran a
paired ±0.5-prior-standard-deviation perturbation under common random numbers,
plus six independent prior-median seeds for noise. It performed no fitting,
optimization, posterior construction or parameter selection.

| vector | dimension | noise RMS | effective rank | mean absolute draw correlation |
|---|---:|---:|---:|---:|
| full | 1,096 | 1.431933 | 3.348158 | 0.585817 |
| reduced | 165 | 0.146963 | 1.720749 | 0.582701 |

The predeclared reduced gate required effect-to-noise ≥ 1.0 and retained effect
ratio ≥ 0.1 for every free parameter:

| parameter | full E/N | reduced E/N | retained effect | reduced gate |
|---|---:|---:|---:|:---:|
| `interstate_base` | 0.083 | 1.612 | 2.003 | pass |
| `k` | 0.056 | 0.558 | 1.014 | fail |
| `peak_months` | 0.052 | 0.914 | 1.808 | fail |
| `pull_act` | 0.024 | 0.612 | 2.567 | fail |
| `pull_nt` | 0.053 | 0.292 | 0.562 | fail |
| `pull_qld` | 0.003 | 0.078 | 2.578 | fail |
| `pull_sa` | 0.010 | 0.256 | 2.645 | fail |
| `pull_tas` | 0.015 | 0.378 | 2.573 | fail |
| `pull_vic` | 0.004 | 0.118 | 3.030 | fail |
| `pull_wa` | 0.006 | 0.139 | 2.412 | fail |
| `push_act` | 0.014 | 0.201 | 1.523 | fail |
| `push_nt` | 0.005 | 0.115 | 2.299 | fail |
| `push_qld` | 0.008 | 0.209 | 2.598 | fail |
| `push_sa` | 0.051 | 0.472 | 0.956 | fail |
| `push_tas` | 0.009 | 0.236 | 2.606 | fail |
| `push_vic` | 0.014 | 0.351 | 2.580 | fail |
| `push_wa` | 0.012 | 0.302 | 2.587 | fail |

Therefore the frozen rule recommends **`full`**. This is not evidence that the
full vector is strongly identified: at hundredth scale its own measured
per-parameter effects were also below the noise gate. It is evidence that the
165-component reduction is not justified by this experiment. PRD 0008 must
retain full evidence, use additional replication or larger-scale confirmation,
and report weak identification rather than relabeling these failures as fitted
parameters.

Reproduce the evidence with a resumable work directory:

```bash
python3 scripts/measure-target-sensitivity.py \
  --work /tmp/sembla-target-sensitivity-work
```

The final accepted ensemble was rerun from an empty cache through the hardened
scorer. A warm rerun from that complete cache reproduces the evidence bytes
exactly.

## Non-claims

A fitted control is reconstruction, not held-out validation. Single-age and NT
holdouts must be reported separately. The target ledger does not establish
causal migration mechanisms, individual fertility, omitted relationships or
sub-state validity.

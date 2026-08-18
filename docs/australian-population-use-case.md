---
title: Australian population use case for Sembla
aliases:
  - Australian synthetic population
  - ABS-calibrated population
status: scoped
tags:
  - sembla
  - australia
  - synthetic-population
  - microsimulation
  - ABS
  - calibration
created: 2026-07-23
---

# Australian population use case for Sembla

> [!summary]
> Build a reproducible person-level Australian population for each 30 June from 2010 onward, represented at minimum by **SA2 area × age group × sex**, and use it as the starting state for policy simulations in Sembla.
>
> The recommended first release is an **external population-construction pipeline producing annual snapshots**, not a demographic generator inside Sembla. Official ABS Estimated Resident Population (ERP) should be the hard population control. Sembla's runtime can represent the attributes, but its current population file and CLI loader cannot ingest them.

## Recommendation in one page

1. Use the ABS **Regional population by age and sex** series as the primary control. It currently provides annual 30 June ERP by SA2, five-year age group and sex from **2001 to 2024**.
2. Use **ASGS Edition 3 (2021) SA2s** as the canonical longitudinal geography. The current ERP cube already backcasts 2001–2024 to this geography, avoiding most manual crosswalking.
3. For the minimum requested dimensions—area, age and sex—construct records directly from the joint ERP cells. IPF is unnecessary because the desired joint table is already published. Integer cell counts can match the published estimates exactly.
4. Use the 2011, 2016 and 2021 Census only to add structure not present in ERP, such as households, dwellings, labour force, education or relationships. Those extensions require reweighting or synthetic reconstruction and have materially greater uncertainty.
5. Keep ABS acquisition, harmonisation, synthesis, privacy handling and validation in an **external Python/R/SQL pipeline**, consistent with Sembla's design decision that population generation is an external product.
6. Add a generic, versioned population import contract to Sembla. The current `SEMBLA_POP` v1 file stores only SIR `health` and `employer` columns.
7. Start with independent annual snapshots for 2010–latest. Add longitudinal ageing, births, deaths and migration only after Sembla has an explicit dynamic-population design; current execution assumes fixed table row counts.
8. Treat population fitting and behavioural-model inference as different tasks. Exact fit to published ERP margins is constrained reconstruction; Sembla's NPE workflow is better suited to uncertain behavioural or transition parameters.

> [!important]
> A population that exactly matches published area × age × sex controls is not thereby realistic on household, income, employment, disability or other omitted relationships. Exact calibration fit must be reported separately from held-out validation and source uncertainty.

## Proposed use case

### Name

**ABS-calibrated Australian demographic base population**

### User story

A policy modeller selects a reference year and receives a reproducible synthetic Australian population whose counts agree with the official population by SA2, age group and sex. The population can then be enriched with additional attributes and supplied to Sembla models for health, service demand, labour market, transport or other policy counterfactuals.

### Minimum population schema

| Table | Field | Suggested type | Meaning |
|---|---|---:|---|
| `person` | `person_id` | `Int` external key, optional | Stable synthetic key; never a real-person identifier or Sembla runtime identity |
| `person` | `age_band` | `Enum` | ABS five-year group, preferably preserving the published category |
| `person` | `sex` | `Enum {Male,Female}` for this ERP series | Construct from the male and female tables; do not treat the published `Persons` total as a third category or infer gender |
| `person` | `area` | `Ref(area)` | Canonical ASGS 2021 SA2 |
| `person` | `alive` | `Enum` or implicit | Only needed in a later dynamic model |
| `area` | `sa2_code_2021` | external dictionary/code | Stable area identifier |
| `area` | `state` | `Ref(state)` or category | Hierarchical aggregation |
| `area` | `name` | external metadata | Human-readable label; not part of simulation identity |

In the current Sembla runtime, stochastic `entity_id` is the row ordinal. An imported `person_id` would be an ordinary attribute for joins and audit only. The population bundle must therefore define a canonical row order, and paired common-random-number runs must reuse exactly the same rows in exactly the same order.

Recommended provenance beside every snapshot:

- reference date and source release date;
- ABS product, file URL and SHA-256;
- estimate status (`final`, `revised`, `preliminary`);
- ASGS edition and any correspondence file/quality indicator;
- construction algorithm, seed and software version;
- source category dictionaries and missing-value policy;
- validation report and immutable population-content hash.

### Core outputs

- a versioned annual population snapshot;
- an area table and category dictionaries;
- a source/provenance manifest;
- exact-fit results for all published ERP controls, explicitly labelled as fit to official estimates;
- held-out validation and uncertainty results, where richer attributes exist;
- Sembla-ready typed tables after the importer is implemented.

## Relevant ABS data releases

### Primary stock controls

| Dataset | Coverage and resolution | Recommended role | Important cautions |
|---|---|---|---|
| [Regional population by age and sex](https://www.abs.gov.au/statistics/people/population/regional-population-age-and-sex/latest-release) and [2024 methodology](https://www.abs.gov.au/methodologies/regional-population-age-and-sex-methodology/2024) | Annual at 30 June; current cube covers 2001–2024; SA2/LGA; five-year age groups by sex | **Primary annual calibration target** | 2001–2021 final, 2022–2023 revised and 2024 preliminary in the 2024 release. Preliminary age/sex estimates arrive roughly 14 months after the reference date and are later revised/rebased. Calculations are made at single year of age, but public output is generally five-year groups. Component data are confidentialised and constrained; estimates under three should be treated as synthetic additivity values, and very small areas require caution. |
| [National, state and territory population](https://www.abs.gov.au/statistics/people/population/national-state-and-territory-population/latest-release) | Quarterly ERP and components; state/territory age-sex series, including annual single-year age files | Higher-level controls, quarterly timing and single-age profiles | Post-Census estimates are progressively revised. State single-age patterns do not identify the distribution within every SA2 five-year band. |
| [Regional population](https://www.abs.gov.au/statistics/people/population/regional-population/latest-release) | Annual SA2 totals; latest total series can be one year ahead of age-sex release | Latest SA2 total control and recent component diagnostics | Do not mix a newer total vintage with an older age-sex vintage without a documented reconciliation rule. |

ERP, rather than raw Census counts, is the correct population-stock estimand. Census-year ERP adjusts Census usual-resident counts for under/overcount, residents temporarily overseas and the timing difference between 30 June and Census night. Exact reconstruction therefore means exact agreement with the **published, modelled estimates**—including their confidentiality and small-cell treatment—not exact knowledge of the underlying population.

### Census structural sources

| Dataset | Coverage | Recommended role | Important cautions |
|---|---|---|---|
| [Census DataPacks](https://www.abs.gov.au/census/guide-census-data/about-census-tools/datapacks) | 2011, 2016 and 2021; CSV tables down to SA1 with metadata and boundaries | Household/person margins and held-out cross-tabs | Census counts are not ERP. Detailed tables are perturbed and may not add exactly. Definitions and collection methods change. |
| 2021 Census Time Series Profile, available through [DataPacks](https://www.abs.gov.au/census/find-census-data/datapacks) | Selected 2011, 2016 and 2021 variables expressed on 2021 boundaries/classifications | Quick structural comparison on a common geography | It is a selected profile, not a complete set of arbitrary joint tables. |
| [Census microdata and TableBuilder](https://www.abs.gov.au/statistics/microdata-tablebuilder/available-microdata-tablebuilder/census-population-and-housing) | Basic 5% Census microdata plus controlled-access products | Seed joint relationships and household records | Basic files are confidentialised samples with broad geography; large households and rare records receive special treatment. Detailed DataLab access needs approval and output clearance. |

### Demographic-flow sources

| Dataset | Useful detail | Role in a later dynamic model | Limitations |
|---|---|---|---|
| [Births, Australia](https://www.abs.gov.au/statistics/people/population/births-australia/latest-release) | Annual births; regional summaries; state fertility detail | Birth-rate estimation and diagnostics | Registration year and occurrence year differ. Raw registered births are not necessarily the adjusted component used in ERP. |
| [Deaths, Australia](https://www.abs.gov.au/statistics/people/population/deaths-australia/latest-release) | Deaths by age/sex at higher geography; regional summaries | Mortality estimation and diagnostics | Recent occurrence data are incomplete; registration timing matters. |
| [Overseas Migration](https://www.abs.gov.au/statistics/people/population/overseas-migration/latest-release) | Age, sex and state flows; annual/quarterly products | Overseas-arrival and departure models | Estimates are revised after traveller histories mature; COVID-era patterns and methods require explicit treatment. |
| National/state and [Regional population](https://www.abs.gov.au/statistics/people/population/regional-population/latest-release) migration components | State interstate migration by age/sex; recent regional net internal and overseas migration | Internal/overseas movement constraints | No clean public 2010-onward SA2 × age × sex history for every component was identified. Internal migration is modelled from administrative sources rather than directly observed. |

Use flows to propagate and diagnose a dynamic population, but force annual results back to ERP stock controls. Published births, deaths and migration will not necessarily reconcile exactly to ERP because components are modelled, confidentialised, constrained, revised and rebased.

### Geography and access

- [ASGS Edition 3 correspondences](https://www.abs.gov.au/statistics/standards/australian-statistical-geography-standard-asgs/edition-3-july-2021-june-2026/access-and-downloads/correspondences) provide cross-edition ratios and `Good`/`Acceptable`/`Poor` conversion quality. Preserve these quality indicators.
- ASGS replaced ASGC in July 2011. The relevant Census editions are 2011, 2016 and 2021; boundaries and codes change around each Census.
- Prefer the ABS-provided 2001–2024 ERP time series already expressed on ASGS 2021 boundaries. Retain native-geography source data for audit.
- The [ABS Data API](https://www.abs.gov.au/statistics/application-programming-interfaces-apis/data-api-user-guide) is a free, unauthenticated SDMX REST beta service and can return CSV. Its base URL changed in November 2024 and API data can lag the website. Pin dataflow IDs, cache raw responses and verify reference periods against release pages.
- For large SA2 products, ABS itself recommends the downloadable Excel cubes over interactive Data Explorer use.
- [Population Projections, Australia, 2022 base–2071](https://www.abs.gov.au/statistics/people/population/population-projections-australia/latest-release) can support future scenarios. ABS explicitly states that these are projections under assumptions, not forecasts; they must not be used as historical calibration observations.

## Construction and calibration design

### Stage A — minimum area × age × sex population

For each reference year:

1. Load the SA2 × five-year-age ERP tables for **males** and **females**, plus the published **persons** table as a reconciliation control.
2. Validate category coverage, hierarchical totals, estimate status and that male + female cells reconcile to persons under the source's rules.
3. Create synthetic records from male and female cells only; never generate the persons aggregate again.
4. Assign deterministic synthetic keys from `(population version, year, SA2, age band, sex, within-cell index)`.
5. Materialise one canonical deterministic row order. If construction randomises rows, do it once before hashing and reuse the exact order in every paired scenario because Sembla random coordinates use row ordinal.
6. Emit the snapshot, source manifest and exact-fit report.

This construction has no statistical fitting problem for the requested dimensions: the published cells are integer population controls. The result can match every published target exactly, but that is fit to confidentialised/modelled ERP estimates rather than proof of the unobserved true cell counts. If exact single-year age is required, either obtain an appropriate ABS extract or impute within five-year bands using state patterns and label the result as modelled—not observed at SA2.

### Stage B — richer Census-anchored population

Once households or additional attributes are requested:

1. Use the nearest 2011/2016/2021 Census as the structural anchor.
2. Build coherent person, family, household/dwelling and non-private-dwelling controls.
3. Reconcile perturbed or inconsistent tables with explicit slack rather than forcing every published cell as simultaneously exact.
4. Include annual ERP age-sex-area controls in the fractional multilevel IPF/IPU or entropy-calibration fit.
5. Perform final whole-household integerisation, combinatorial optimisation and local repair against the reconciled controls.
6. Validate that repair preserved household membership, ERP controls and hierarchical totals; iterate the fit/repair if needed.
7. Rebuild or strongly recalibrate at every Census anchor.

IPF is a transparent baseline, but it preserves seed associations, cannot create sampling-zero combinations and does not by itself satisfy person and household controls simultaneously. Combinatorial optimisation produces whole households and can fit selected constraints very closely, but costs more and still cannot validate relationships omitted from the controls.

### Stage C — annual demographic evolution

A later longitudinal model should apply:

```text
P(t+1) = P(t) + births - deaths + net overseas migration + net internal migration
```

At person level this requires ageing, births, deaths and origin-destination moves, plus household transition logic if households are modelled. Ending populations must be reconciled to annual SA2 age-sex ERP and state/national totals. At each new Census, retain both:

- an **as-published-at-the-time** vintage for real-time replay; and
- a **latest final/rebased** vintage for retrospective research.

## How well can it be calibrated?

### Three different meanings of calibration

| Layer | Calibration target | Assessment |
|---|---|---|
| Population reconstruction | Annual ERP area × age × sex counts | **Excellent fit to the published estimates by construction.** Exact cell and hierarchical total agreement is achievable for the minimal schema. This neither validates omitted relationships nor removes ERP modelling, confidentiality or revision uncertainty. |
| Rich synthetic population | Census household/person margins and joint relationships | Selected fitted margins can be exact or near-exact. Held-out joints, rare groups and very small areas can remain materially wrong. |
| Dynamic or policy model | Transition rates or behavioural parameters inferred from observed summaries | Feasibility depends on identifiability, observation choice and simulation budget. Existing Sembla NPE machinery is a starting point, not evidence that an Australian model will be calibrated. |

### Expected calibration strength by scale

- **Australia/state and SA2 age-sex controls:** exact or rounding-level fit to the published estimates is realistic when treated as hard controls; it does not eliminate source uncertainty.
- **SA2 fitted household/person margins:** use sub-percent to low-single-digit aggregate error only as an initial pilot target, not an expected result. Revise the target after measuring compatibility, seed coverage and integerisation error on Australian controls.
- **SA1:** Census-year totals can be fitted, but annual post-Census age-sex detail is less defensible and rare-cell integerisation/perturbation dominates. Treat it as soft or simulated detail.
- **Mesh Block/address:** count-consistent placement may be possible, but public ABS evidence does not support precise attribute calibration or reconstruction of actual addresses.
- **Held-out joint distributions:** one UK comparison reported percentage-classification errors around 10–20% for some fine-area Output Area relationships, varying substantially by method, table and scale ([Harland et al., Tables 4–5](https://www.jasss.org/15/1/1.html)). This is a warning that exact fitted margins can coexist with poor omitted joints, not an Australian performance guarantee.
- **Annual forward prediction:** no credible error promise should be made before rolling-origin tests, for example fit through 2015 and assess 2016, then repeat through 2021/2024. Migration and fast-growing or atypical SA2s will dominate error.

### Required validation report

Separate fitted controls from evidence not used in fitting. Report at least:

- signed and absolute cell error;
- MAE/RMSE and maximum error;
- total absolute error divided by two, or percentage classification error;
- relative error only above a documented minimum denominator;
- divergence between distributions, with smoothing for zero cells;
- residual maps and error by population size;
- logical and referential-integrity failures;
- extreme weights, duplicated donor records and structural/sampling zeros;
- held-out Census cross-tabs;
- temporal validation from one Census anchor to the next;
- uncertainty intervals across bootstrap/synthesis/dynamics replicates.

Do not use correlation alone: large areas can produce a high correlation while small or rare cells are poor.

### Uncertainty

A production population should be an ensemble or at least support replicate generation. Sources include Census sample design, perturbation, imputation, geographic correspondences, integerisation, alternative admissible joint distributions, migration shocks and demographic-rate uncertainty. Synthetic records are plausible model entities, not reconstructed real people.

## Fit with Sembla today

| Capability | Current status | Consequence |
|---|---|---|
| Typed person attributes | Supported by the IR/runtime as `Int`, `Real`, `Enum` and `Ref` | Age, sex, area, household and weights are representable. |
| Multiple relational tables | Supported | `person`, `area`, `household`, `workplace` and similar tables fit the ACSet/columnar design. |
| Generic external population import | **Not supported** | The normal CLI cannot load the proposed attributes. |
| Current population file | `SEMBLA_POP` v1 contains only `health`, `employer` and employer count | It is a deterministic SIR benchmark, not a demographic contract. See [`population.rs`](../crates/sembla-runtime/src/population.rs). |
| Dynamic births/deaths | Deferred; runtime state currently has fixed row counts | Start with independent yearly snapshots. Do not disguise deletion with a production preallocation workaround without a semantic design. |
| Views and summaries | Supported | Models can report demographic totals and policy outcomes without feeding observation back into state. |
| Parameter sweeps and external proposals | Supported | Useful for prior predictive checks and external inference. |
| NPE calibration | External reference pipeline exists | It consumes parameter/summary pairs with independent simulation noise. It is suitable for uncertain model parameters, not for replacing deterministic ERP fitting. See [`calibration/npe/README.md`](../calibration/npe/README.md). |
| Common random numbers | Supported | Valuable after population construction for paired policy scenarios using the same population content, canonical row order and seed. Stable external person keys alone are insufficient because runtime random coordinates use row ordinal. |

This division matches Sembla's architecture: [`DESIGN.md`](../DESIGN.md) explicitly places Census/HILDA-style population generation, reweighting, privacy and validation outside the runtime boundary.

## Required Sembla extension

The preferred integration is a **generic versioned population bundle**, while preserving `SEMBLA_POP` v1 replay compatibility. The bundle should contain:

- schema/version identifiers;
- box, table and column names;
- typed column descriptors and row counts;
- enum dictionaries;
- reference targets and validated foreign keys;
- column files in a deterministic, scalable encoding;
- content hashes and canonical manifest;
- source-provenance and geography metadata;
- an explicit missingness/category policy.

The CLI loader should validate the bundle against the selected model and convert it to Sembla `TableInit`/`ColumnInit` values. An interim external Rust adapter can construct those initializers programmatically, but it will not be an audit-grade normal CLI path unless population hashing and manifest replay are preserved.

> [!warning]
> Do not overload the existing `health` and `employer` columns to encode age, sex or area. The CLI checks the SIR schema, and such an encoding would be semantically false and non-extensible.

## Delivery scope

The estimates below are order-of-magnitude single-developer scopes, not commitments.

| Phase | Deliverable | Indicative scope | Exit criterion |
|---|---|---:|---|
| 0. Data spike | Download and normalise 2010–latest SA2 age-sex ERP on ASGS 2021 | 1–2 weeks | Reproducible raw cache, parser, checksums and reconciliation report |
| 1. Static builder | Deterministic annual person snapshots plus manifests and exact-fit validation | 2–4 weeks | Every published male/female SA2 × age cell matches, persons totals reconcile, and repeated build is byte-identical |
| 2. Sembla importer | Generic population bundle and model-aware loader | 3–6 weeks | Small and full-scale snapshots validate, run and replay through manifests |
| 3. Demonstration model | One area-age-sex policy model with views, summaries and CRN comparison | 2–4 weeks | End-to-end documented run and paired counterfactual |
| 4. Census enrichment | Household/person synthesis and held-out validation | 2–6 months | Household consistency, documented privacy controls and empirical hold-out results |
| 5. Dynamic population | Birth/death/migration semantics and annual data assimilation | 2–4+ months after design approval | Rolling-origin validation and Census-to-Census backtest |

A useful MVP stops after phase 3. It provides calibrated starting populations for policy simulation without claiming a complete Australian demographic projection system.

## Principal risks and mitigations

| Risk | Mitigation |
|---|---|
| Confusing Census counts with ERP | State the estimand in every artifact; use ERP as stock control and Census as structural evidence. |
| Geography drift | Canonical ASGS 2021 SA2, native-source preservation, official correspondences and quality flags. |
| ABS revisions silently changing results | Immutable raw vintages, dated release identifiers, hashes, controlled rebasing and dual real-time/final outputs. |
| Small-cell false precision | Soft constraints, denominator rules, uncertainty ensembles and explicit synthetic labels. |
| Missing public SA2 age-sex flow history | Calibrate annual stocks; use higher-level flow rates with pooling; seek customised ABS data only if required. |
| Household/person inconsistency | Multilevel fitting, whole-household integerisation and relational repair/validation. |
| NPE overconfidence | Independent simulation noise, simulation-based calibration, held-out recovery and richer summaries where identifiable. |
| Privacy leakage or misinterpretation | No real-person identity claims, no inferred exact addresses, disclosure-risk review and source-access compliance. |
| Runtime fixed cardinality | Static annual snapshots first; design births/deaths explicitly before longitudinal implementation. |

## Open decisions before implementation

1. Is annual **SA2 × five-year age × sex** sufficient, or is single-year age mandatory?
2. Should 2010–2024 be represented on common 2021 boundaries only, or must native historical areas also be executable?
3. Is the population a sequence of independent annual snapshots or a longitudinal set of persistent synthetic persons?
4. Which attributes beyond age, sex and area are in the first policy model?
5. Are households required in the MVP?
6. Is public ABS data sufficient, or will the project pursue DataLab/customised extracts?
7. What is the first downstream policy model and therefore which held-out relationships matter?
8. What disclosure and release policy applies to synthetic records and provenance artifacts?

## Sources and further reading

### ABS primary sources

- [Regional population by age and sex](https://www.abs.gov.au/statistics/people/population/regional-population-age-and-sex/latest-release)
- [Regional population by age and sex methodology, 2024](https://www.abs.gov.au/methodologies/regional-population-age-and-sex-methodology/2024)
- [National, state and territory population](https://www.abs.gov.au/statistics/people/population/national-state-and-territory-population/latest-release)
- [Regional population](https://www.abs.gov.au/statistics/people/population/regional-population/latest-release)
- [Census DataPacks](https://www.abs.gov.au/census/guide-census-data/about-census-tools/datapacks)
- [Census microdata and TableBuilder](https://www.abs.gov.au/statistics/microdata-tablebuilder/available-microdata-tablebuilder/census-population-and-housing)
- [ASGS Edition 3 correspondences](https://www.abs.gov.au/statistics/standards/australian-statistical-geography-standard-asgs/edition-3-july-2021-june-2026/access-and-downloads/correspondences)
- [ABS Data API guide](https://www.abs.gov.au/statistics/application-programming-interfaces-apis/data-api-user-guide)
- [Births, Australia](https://www.abs.gov.au/statistics/people/population/births-australia/latest-release)
- [Deaths, Australia](https://www.abs.gov.au/statistics/people/population/deaths-australia/latest-release)
- [Overseas Migration](https://www.abs.gov.au/statistics/people/population/overseas-migration/latest-release)
- [Population Projections, Australia](https://www.abs.gov.au/statistics/people/population/population-projections-australia/latest-release)

### Methods and Australian applications

- Tanton, [A review of spatial microsimulation methods](https://microsimulation.pub/articles/00092)
- Harland et al., [Creating Realistic Synthetic Populations at Varying Spatial Scales](https://www.jasss.org/15/1/1.html)
- Namazi-Rad et al., [Building a large synthetic population from Australian Census data](https://arxiv.org/abs/2008.11660)
- BITRE, [Population synthesis for travel demand modelling in Australian capital cities](https://www.bitre.gov.au/publications/2020/population-synthesis-travel-demand-modelling-australian-capital-cities)

## Bottom line

This is a strong use case for Sembla's relational population state, reproducibility and counterfactual design, but it should enter through a carefully versioned external-data boundary. The **minimal area-age-sex population can match published ABS ERP exactly** and be delivered relatively quickly. The difficult work begins when adding households, omitted joint attributes or genuine longitudinal demographic dynamics; those layers need separate evidence, uncertainty and validation, and should not inherit credibility merely from exact ERP fit.

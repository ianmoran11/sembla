# Justice event schema implementation sequence

**Status:** Proposal  
**Projects:** TidyCell and Sembla  
**Related proposal:** [Justice event schema and aggregate-statistics mapping](justice-event-schema-and-aggregate-statistics.md)

## Guiding approach

Implement this through vertical slices, beginning with aggregate semantics rather than the complete event database.

```text
Contracts and codelists
        ↓
Small manually verified semantic-cell set
        ↓
TidyCell semantic compiler and validator
        ↓
Reviewed Prisoners semantic corpus
        ↓
Non-redundant calibration target ledger
        ↓
Sembla current-state observation fixture
        ↓
Initial population synthesis and stock calibration
        ↓
Flow data and richer process model
        ↓
Evidence-triggered Sembla runtime extensions
```

## Phase 0 — Freeze the V1 scope

Limit V1 to:

- ABS *Prisoners in Australia*;
- stock at midnight on 30 June;
- person counts;
- 2019–2025;
- reporting jurisdiction;
- sex;
- Indigenous status;
- age band;
- legal status;
- most serious offence or charge;
- ANZSOC 2011 and preliminary 2023.

Explicitly defer:

- victims and offence incidents;
- court queues;
- sentence and remand duration statistics;
- rates, medians, and age standardisation;
- event-stream execution in Sembla.

**Gate:** A human can describe 20 representative cells without ambiguity using only the proposed concepts.

## Phase 1 — Define the shared contracts

Create versioned JSON schemas for:

1. `justice.classification/v1`
2. `justice.statistic-definition/v1`
3. `justice.semantic-table/v1`
4. `justice.semantic-cell/v1`
5. `justice.methodology/v1`
6. `justice.simulation-binding/v1`

The semantic-table artifact should describe a table once—universe, measure, axes, and member mappings—then expand deterministically into semantic cells.

Define stable identifiers such as:

```text
statistic: prisoners.count
dimension: legal_status_abs
member: legal_status_abs/sentenced
classification: anzsoc/2011/division/01
methodology: prisoners-australia/2025
```

Every artifact should have:

- `schema_version`;
- canonical JSON rules;
- SHA-256 identity;
- explicit source provenance;
- migration policy;
- no silent codelist substitution.

**Ownership recommendation:** Start with the contracts in TidyCell because TidyCell produces the semantic evidence. Export a self-contained, hash-pinned bundle for Sembla rather than making Sembla import TidyCell code.

**Gate:** Valid examples round-trip canonically; malformed and unknown-version examples fail.

## Phase 2 — Build the codelists

Implement:

- jurisdictions and reporting geographies;
- legal statuses and ABS precedence categories;
- sex and not-stated values;
- Indigenous status and not-stated values;
- age-band definitions;
- measure and unit types;
- publication-value statuses such as published, suppressed, unavailable, and not applicable;
- ANZSOC 2011;
- ANZSOC 2023;
- explicit edition crosswalks.

Model `Australia`, `Total`, `All persons`, and similar values as rollups, not ordinary category members.

**Gate:** Every member in the manually selected tables maps to either a canonical member or an explicit unresolved value.

## Phase 3 — Create a small semantic gold set

Select approximately 5–8 representative tables:

1. prisoner counts by state, sex, and legal status;
2. counts by most serious offence;
3. counts by most serious charge;
4. Indigenous-status counts;
5. age-band counts;
6. one ANZSOC 2023 table;
7. one deliberately difficult multi-level-header table.

Manually annotate approximately 50–100 cells. Freeze:

- workbook SHA-256;
- sheet name;
- cell address;
- recipe digest;
- raw header labels;
- canonical semantic descriptor.

Also create negative fixtures:

- unsentenced × expected time to serve;
- sentenced population using a charge where an offence is required;
- ANZSOC 2011 statistic with a 2023-only code;
- two sex members attached to one cell;
- rate without denominator;
- total represented as an ordinary member.

**Gate:** Two independent reviews agree on the semantic gold descriptors.

## Phase 4 — Implement the TidyCell semantic pipeline

Add a provider-free pipeline:

```text
approved recipe
   → execute recipe
   → semantic table mapping
   → expand table axes into semantic cells
   → validate
   → canonical artifact
```

Validation layers:

1. JSON shape;
2. codelist membership;
3. applicability and satisfiability;
4. cube completeness and uniqueness;
5. hierarchy and rollup consistency;
6. arithmetic checks;
7. workbook and recipe provenance.

Preserve both:

- original spreadsheet labels;
- canonical mapped concepts.

Never discard an unresolved label or guess silently.

Integrate semantic review into the existing Visual Recipe Editor rather than creating a second review system.

**Gate:** All positive gold fixtures pass and every negative fixture fails for the intended reason.

## Phase 5 — Scale across the Prisoners corpus

Apply the semantic pipeline to approved recipes across 2019–2025.

Generated-but-unapproved recipes can be used to develop the schema, but should not become final calibration evidence until human-approved.

Produce a coverage report covering:

- tables and cells semantically resolved;
- unresolved labels;
- unsupported measures;
- ANZSOC-edition conflicts;
- duplicate observations across annual workbooks;
- source revisions;
- failed recipes;
- approval status.

**Gate:** Every selected calibration cell is approved and semantically resolved, or explicitly excluded with a reason.

## Phase 6 — Build the target dependency graph

Do not treat every published cell as independent evidence.

Create canonical observation keys from:

```text
statistic + universe + reference time + dimensions + classification edition
```

Identify:

- repeated historical observations in later workbooks;
- totals and their components;
- counts and percentages derived from those counts;
- counts and rates sharing a fixed denominator;
- ratios and their underlying rates;
- ANZSOC 2011 and 2023 representations of related quantities.

Assign every cell one role:

- `fitted`;
- `heldout`;
- `diagnostic`;
- `redundant`;
- `unsupported`.

Freeze the role assignment before calibration.

Start with an independent **count-only** projection. Rates and medians can remain diagnostic initially.

**Gate:** No duplicate target keys, no fitted/held-out overlap, and no obvious deterministic double weighting.

## Phase 7 — Compile to a Sembla justice target ledger

Create a justice equivalent of `sembla.targets/v1`, following the existing population-target conventions:

- exact model and plan hashes;
- exact source-bundle hash;
- observation name and selectors;
- reference tick or date;
- scale and discretisation;
- fitted or held-out role;
- source workbook, sheet, cell, and recipe;
- stable target-vector order.

Compile count cells to Sembla grouped views.

Current grouped views support count observations with up to four keys. Higher-dimensional cells should initially be represented through multiple filtered views or deterministic external projection—not by changing the runtime immediately.

**Gate:** A tiny synthetic fixture produces exactly the expected semantic cells and a byte-stable target score.

## Phase 8 — Implement the Sembla stock-state MVP

Use a current-state projection rather than normalized event history:

```text
PrisonerSlot
- occupancy
- generation
- age_months
- sex
- indigenous_status
- reporting_jurisdiction
- legal_status
- most_serious_item_kind
- most_serious_anzsoc_division
- sentence_band
- remand_band
- event_marker
```

Initial transitions should be same-row state changes that current Sembla can express.

Do not yet depend on:

- arbitrary event-row creation;
- scheduled clocks;
- cross-row writes;
- append-only event streams;
- exact court scheduling.

Declare observations as sinks and prove that enabling them does not alter simulation state.

**Gate:** Deterministic run-twice equality, observation non-interference, and exact recovery of known synthetic counts.

## Phase 9 — Construct the initial prisoner population

The spreadsheets provide marginals and selected cross-tabs, not a complete joint micro-population.

Use a documented synthesis procedure such as constrained reweighting, iterative proportional fitting, or maximum-entropy reconstruction. Distinguish:

- jointly observed relationships;
- relationships inferred from overlapping margins;
- unconstrained relationships;
- inconsistent margins caused by rounding, revisions, or differing universes.

Prefer an ensemble of plausible initial states rather than pretending one synthetic joint distribution is observed truth.

Use some cross-tabs for construction and reserve others for validation.

**Gate:** Fitted margins are reconstructed within declared tolerances; held-out margins remain untouched until evaluation.

## Phase 10 — Run the stock-only calibration pilot

Before full neural posterior estimation:

1. run prior-predictive sweeps;
2. measure parameter sensitivity;
3. measure simulation noise;
4. remove or fix parameters unsupported by the target vector;
5. test reduced and full population scales;
6. check `dt` sensitivity;
7. freeze the calibration design.

With stock data alone, describe this honestly as stock reconstruction or weak process calibration. It cannot separately identify offending, court delay, admissions, and release mechanisms.

**Gate:** Every free parameter measurably affects at least one fitted target above simulation noise.

## Phase 11 — Add flow evidence and process detail

Next incorporate sources such as:

- prison receptions and releases;
- remand admissions;
- sentenced and unsentenced receptions;
- court lodgements and finalisations;
- sentencing outcomes;
- parole entries, exits, and breaches;
- corrective-services supervision flows.

Then introduce the richer canonical trajectory:

```text
person
  → justice_matter
  → charge
  → court_event/disposition
  → sentence_order
  → custody_episode
  → custody_placement
```

Implement versioned ABS census-projection rules over that trajectory.

Only at this point should transition hazards for court, sentencing, remand, and release be treated as identifiable calibration parameters.

## Phase 12 — Add runtime features only when required

Create separate Sembla design tracks for:

- scheduled clocks;
- keyed queue ordering;
- dynamic row lifecycle;
- append-only event streams;
- exact per-box court scheduling;
- grouped non-count statistics.

Each extension should name the specific target or process that cannot be represented by current state, markers, or counters.

## Recommended first vertical slice

Begin with one table such as:

> Prisoners by reporting jurisdiction, sex, and legal status at 30 June.

Deliver the complete path:

```text
workbook cell
→ approved recipe
→ semantic cell
→ satisfiability validation
→ frozen target
→ Sembla grouped observation
→ deterministic comparison
```

Include one negative test such as `unsentenced × expected_time_to_serve`.

This small slice tests the architecture end-to-end before committing to the entire ontology.

## What not to do first

- Do not implement the complete normalized justice database first.
- Do not change Sembla’s runtime before proving a current feature is insufficient.
- Do not treat candidate recipes as approved evidence.
- Do not calibrate to every table simultaneously.
- Do not combine ANZSOC editions through an implicit crosswalk.
- Do not infer process hazards from prisoner stock snapshots alone.

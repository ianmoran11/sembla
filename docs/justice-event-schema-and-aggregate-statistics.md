# Justice event schema and aggregate-statistics mapping proposal

**Status:** Proposal  
**Projects:** Sembla and TidyCell  
**Use case:** Calibrating a criminal-justice microsimulation against ABS *Prisoners in Australia* statistics, with semantic validation of spreadsheet recipes.

## Recommendation

Do not make one physical schema serve simultaneously as the event history, ABS statistical ontology, and Sembla runtime representation. Define a shared semantic contract, then compile or project into each representation.

```text
ABS spreadsheet
  └─ TidyCell recipe
       └─ semantic statistical cells ─────────┐
                                              ├─ compare/calibrate
Simulation event/episode history              │
  └─ ABS-methodology census projection        │
       └─ semantic statistical cells ─────────┘
```

The strongest initial deliverable is therefore the **semantic-cell and census-projection contracts**, rather than a complete criminal-justice database.

## Issues to address in the initial schema

### Cardinalities

Foreign keys such as `individual.offence` and `individual.procedural_event` allow only one offence and one event per person. In practice:

- a person can have many matters and events;
- a matter can have many charges;
- a sentence can cover multiple charges;
- multiple people can participate in one incident;
- one custody episode can have several warrants, placements, and sentence orders.

Foreign keys should normally point from the many-side back to the person or matter, or use junction tables.

### Distinguish temporal object types

These concepts have different semantics:

- **Court appearance:** instantaneous event with `occurred_at`;
- **Custody episode:** interval `[started_at, ended_at)`;
- **Facility placement:** interval within a custody episode;
- **Sentence:** legal order with terms and effective dates;
- **Legal status:** derived from active warrants and orders at a reference time.

Consequently, `court_event`, `sentencing`, and `prison_stay` should not share identities through one-to-one `is` relationships.

### Distinguish offence and charge

ABS distinguishes:

- sentenced prisoners → **most serious offence**;
- unsentenced prisoners → **most serious charge**.

For most jurisdictions, the sentenced most serious offence is selected using the longest sentence and then the National Offence Index. Unsentenced charges use National Offence Index rules. A timeless `individual.offence` value therefore loses important legal and methodological semantics.

Prefer:

```text
justice_matter
  ├─ charge
  ├─ disposition / conviction
  └─ sentence_order
```

“Most serious offence or charge” should be a derived census attribute, not source truth.

### Version ANZSOC explicitly

`ANZSOC.class_code` should not be an incrementing integer:

- codes have leading zeroes, for example `"0911"`;
- ANZSOC has Division, Subdivision, and Group levels;
- ANZSOC 2011 and 2023 contain materially different categories;
- crosswalks between editions can be many-to-many.

A suitable shape is:

```text
classification_scheme(id, name, edition, valid_from, valid_to)

classification_node(
  scheme_id,
  code text,
  parent_code text?,
  level,        // division, subdivision, group
  label
)

classification_crosswalk(
  from_scheme_id,
  from_code,
  to_scheme_id,
  to_code,
  relationship,
  provenance
)
```

Use `(scheme_id, code)` as the node identity. Never silently replace an ANZSOC 2011 code with a 2023 code.

### Mechanical corrections

- Replace the duplicate `date_end` in `procedural_event_span` with explicit start and end fields.
- Correct `juridiction` and `jurisdication` to `jurisdiction`.
- Correct `ANZONC` to `ANZSOC`.
- Replace empty DBML types with explicit dates, timestamps, integer durations, text codes, or references.
- Give every duration an explicit unit, preferably integer days or months.
- Do not model `victim` as free text. Victims would require participant-role relationships, but the Prisoners publication cannot identify victim dynamics, so this can be deferred.
- Do not conflate court jurisdiction, correctional jurisdiction, reporting jurisdiction, prison location, and usual residence.

## Proposed canonical domain model

A useful trajectory layer would be:

```text
person
  ├─ person_attribute_assertion
  ├─ justice_matter
  │    ├─ charge
  │    │    └─ classification_assignment
  │    ├─ court_event
  │    ├─ disposition
  │    └─ sentence_order
  │         └─ sentence_charge
  └─ custody_episode
       ├─ custody_legal_basis
       └─ custody_placement
```

Key distinctions:

- `custody_episode`: continuous episode from reception to release;
- `custody_placement`: facility and security-classification intervals, allowing transfers without inventing a new custody episode;
- `legal_basis`: remand warrant, sentence order, post-sentence detention, or another legal basis;
- `sentence_order`: maximum term, minimum or non-parole term, and indeterminate or life flags;
- `charge`: legal allegation or count classified under a specified ANZSOC edition;
- `court_event`: hearing, conviction, sentencing, appeal, or another instantaneous procedural event.

Use half-open intervals, `[valid_from, valid_to)`, consistently. Where relevant, distinguish effective time from record or ingestion time.

The simulation can flatten demographics onto a person row, but the common evidence schema should support source- and time-qualified assertions—particularly for Indigenous status and not-stated values.

## ABS census projection

Introduce a derived view that applies a named version of the ABS counting methodology:

```text
prisoner_census_state(
  person_id,
  reference_date,
  methodology_version,
  reporting_jurisdiction,
  legal_status_abs,
  most_serious_item_kind,    // offence or charge
  anzsoc_scheme,
  anzsoc_code,
  aggregate_sentence_days,
  expected_time_to_serve_days,
  time_on_remand_days,
  prior_imprisonment_status,
  security_classification,
  facility_or_location,
  age_at_reference,
  sex_reported,
  indigenous_status_reported,
  country_of_birth_code
)
```

This view is not source truth. It is generated from the underlying trajectory using versioned ABS rules.

This is necessary because the ABS *Prisoners in Australia* methodology defines:

- census scope at midnight on 30 June;
- legal-status precedence for prisoners with multiple warrants;
- jurisdiction-specific most-serious-offence handling;
- current-episode boundaries;
- aggregate-sentence and expected-time-to-serve derivations;
- inclusions and exclusions from the prisoner census.

The simulation and spreadsheet mapping can then target the same census-state concepts.

Reference: <https://www.abs.gov.au/methodologies/prisoners-australia-methodology/2025>

## Semantic aggregate-statistics contract

The domain schema does not by itself state what an aggregate spreadsheet cell means. Introduce a separate versioned contract, for example:

```yaml
schema: justice.semantic-cell/v1

statistic: prisoners.count
universe:
  concept: abs.prisoner_census
  reference_date: 2025-06-30
  boundary: end_of_day
  methodology: prisoners-australia/2025

measure:
  kind: count_distinct
  entity: person

dimensions:
  reporting_jurisdiction: nsw
  legal_status_abs: sentenced
  sex: female
  most_serious_item:
    kind: offence
    scheme: ANZSOC
    edition: "2011"
    level: division
    code: "01"

value:
  number: 1234
  unit: persons
  status: published

source:
  workbook_sha256: "..."
  sheet: Table_8
  cell: R12C6
  recipe_digest: "..."
  publication_vintage: "2025"

calibration_role: fitted   # fitted | heldout | diagnostic
```

Measures should distinguish at least:

- count;
- percentage or proportion;
- crude rate;
- age-standardised rate;
- rate ratio;
- mean;
- median;
- duration.

Rates require a named denominator. Age-standardised rates require the standard population and method. Ratios require numerator and denominator definitions. Totals should be rollup expressions, not ordinary category members.

A source-independent `justice.semantic-cell/v1` artifact can be compiled into Sembla’s existing `sembla.targets/v1` pattern: observation name, selectors, aggregation, time, source, scale and discretisation, and fitted or held-out role.

## Recipe semantic validation

A semantic descriptor enables several complementary validation layers.

### 1. Shape validation

Validate required fields, data types, dimension uniqueness, and measure-specific requirements.

### 2. Codelist validation

Check that every member exists in the declared classification edition—for example, whether `"0911"` exists in the specified ANZSOC edition.

### 3. Applicability and satisfiability

Examples:

- expected time to serve requires a sentenced population;
- time on remand requires the relevant remand or unsentenced universe;
- unsentenced prisoners use a charge classification, not an offence classification;
- an ANZSOC 2023 code cannot silently populate an ANZSOC 2011 statistic;
- two members of the same exclusive dimension cannot both apply to one cell.

### 4. Cube validation

Detect duplicate combinations, missing expected cells, and totals incorrectly represented as category members. Verify that table axes form the declared product or explicitly identify structural omissions.

### 5. Arithmetic validation

Where publication rounding permits, check:

- components against totals;
- percentages against denominators;
- rates and ratios against compatible populations;
- rollups against hierarchy members.

### 6. Geometry and provenance validation

Semantic plausibility does not prove that the recipe selected the correct spreadsheet cells. Keep TidyCell’s recipe execution, workbook SHA-256, recipe digest, and source-cell checks as a separate layer.

JSON Schema is suitable for syntax. A small deterministic rule layer such as CEL, Datalog, or versioned declarative predicates is better suited to cross-field constraints than SQL checks alone.

## Fit with Sembla

The conceptual model aligns with Sembla’s ACSet and columnar philosophy in `/Users/ian/projects/sembla/DESIGN.md` §4.1. Individuals are rows and relationships are typed references.

It should not, however, be translated directly into the current executable IR:

- transitions currently update attributes on their own row;
- arbitrary event-row insertion and cross-row writes are unavailable;
- scheduled clocks remain deferred;
- append-only event-stream observations remain deferred in `DECISIONS.md` §K9;
- grouped observations currently count committed state using one to four grouping keys.

For an initial executable model, compile the richer ontology into a fixed state table such as:

```text
PrisonerSlot(
  occupancy,
  generation,
  age_months,
  sex,
  indigenous_status,
  reporting_jurisdiction,
  legal_status,
  most_serious_anzsoc_division,
  sentence_band,
  remand_band,
  one_tick_event_marker
)
```

Use grouped views for the 30 June stock and keep source reconciliation, suppression handling, structural-zero expansion, and loss functions outside the runtime so observations remain sinks.

Keep the normalized history independent of the runtime until flow evidence and runtime requirements justify row lifecycle, scheduled clocks, and event-stream semantics.

## Calibration cautions

The *Prisoners in Australia* publication is predominantly a stock snapshot. It cannot by itself identify the complete process:

```text
offending → charge → court delay → conviction → sentence → custody → release
```

Identifying those mechanisms will require flow evidence from sources such as Corrective Services, Criminal Courts, receptions and releases, remand admissions, sentencing, and parole datasets.

Avoid accidental double weighting of dependent evidence:

- a count and a percentage calculated from that count;
- a count and a rate with a fixed denominator;
- a total and every exhaustive component;
- the same historical year repeated in several annual workbooks;
- ANZSOC 2011 and 2023 representations treated as independent observations of the same latent quantity.

Start with independent published counts and freeze fitted and held-out roles before inference. Report reconstruction against fitted cells separately from validation against held-out cells.

## Suggested implementation stages

1. Define and version `justice.semantic-cell/v1`.
2. Canonicalize jurisdiction, sex, Indigenous status, legal status, age bands, and ANZSOC 2011 and 2023.
3. Hand-map 10–20 representative count cells from a pinned ABS release.
4. Implement shape, codelist, applicability, satisfiability, cube, and arithmetic validation.
5. Compile a non-redundant count subset into `sembla.targets/v1`.
6. Build a simple current-state `PrisonerSlot` model and grouped observations.
7. Add sentence and remand durations.
8. Add flow datasets before attempting to calibrate detailed court and custody transitions.
9. Add full event-history runtime semantics only after a concrete flow target demonstrates that state markers or counters are insufficient.

## Relevant project references

- `/Users/ian/projects/sembla/DESIGN.md` §4.1 — ACSet and columnar state semantics
- `/Users/ian/projects/sembla/DESIGN.md` §4.6 — observations are sinks
- `/Users/ian/projects/sembla/DECISIONS.md` §F — intended court and queueing architecture
- `/Users/ian/projects/sembla/DECISIONS.md` §G — parameters and external calibration
- `/Users/ian/projects/sembla/DECISIONS.md` §K9 — deferred event-stream and scheduling constructs
- `/Users/ian/projects/sembla/docs/guides/targets.md` — existing versioned aggregate target conventions
- `/Users/ian/projects/sembla/crates/sembla-ir/src/model.rs` — current executable IR
- ABS Prisoners methodology: <https://www.abs.gov.au/methodologies/prisoners-australia-methodology/2025>
- ANZSOC 2023 hierarchy and version information: <https://www.abs.gov.au/statistics/classifications/australian-and-new-zealand-standard-offence-classification-anzsoc/2023/about-classification>

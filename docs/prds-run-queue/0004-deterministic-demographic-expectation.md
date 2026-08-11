# PRD 0003: Deterministic monthly demographic expectation runner

## Context and dependencies

PRDs 0001–0002 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first; `DECISIONS.md` §O4 and
the data-pipeline quarantine bind.

## Context

The Australian model is a fixed-cardinality, exogenous-rate tau-leap model.
Within a cell defined by its writable person attributes, all rows have the same
enabled hazards. Contested movement, mortality and emigration transitions share
one row-local resource, so their executed one-tick probabilities are available
analytically. Non-contested transitions are independent at tick start and all
effects read the old state; there are no within-tick cascades.

The aggregate companion must reproduce those executed semantics, not a generic
annual cohort-component approximation. Entry activation, event clearing,
monthly ageing, fixed preclassified vacant slots and simultaneous disjoint
writes all belong to the contract. Exactness is not assumed in prose: this PRD
implements the expectation calculation and unit-level closure evidence; PRD
0004 measures it against replicated microsimulation.

## Goal

A standard-library Python runner reads any valid Australian-population start
state and annual parameter file, propagates expected cell counts through twelve
monthly ticks under the actual transition/contest rules, and emits the same
ordered calibration summaries plus a complete diagnostic state/reward report.

## Specification

### 1. Read existing state artifacts without changing their format

Extend `data/abs/state_artifact.py` with a bounded, streaming reader for the
already frozen `sembla.state/v1` bytes. It must:

- validate magic/schema/table/column metadata, row counts, scalar encodings and
  exact end-of-file;
- expose column iterators or arrays sufficient to aggregate rows without
  retaining duplicate decoded copies;
- preserve Ref values as row ordinals and validate them against declared table
  sizes;
- reject malformed/truncated/extra bytes with a table/column/row-aware error;
- round-trip every existing writer fixture through read→semantic comparison;
  and
- make no writer-byte, schema or hash-domain change.

The reader is an in-repo data tool, not a runtime API and not a second accepted
state format.

### 2. Cell state and transition rewards

Add `data/abs/expectation.py` with format `sembla.abs-expectation/v1`.
Aggregate the exact start rows by every attribute required to determine future
Australian-model behavior, including at minimum:

```text
occupancy, event, sex, age_months, event_age_months, generation,
entry_stream, entry_age_months, area, prev_area
```

`slot_resource` is omitted only after verifying it is a one-to-one row-local
claim key in the supplied state. A duplicate/non-row-local resource mapping is
a hard unsupported-state error, not silently approximated.

Maintain expected counts as finite non-negative `float` weights keyed by the
complete cell tuple. Prune only exact zero; do not introduce a fitted numerical
mass cutoff. Track expected transition firings by the existing transition names
and tick.

### 3. Executed one-tick semantics

For enabled contested hazards `λ₁…λⱼ` sharing the row's `slot_resource`, use:

```text
Λ = Σ λⱼ
P(j wins and fires) = λⱼ / Λ × (1 - exp(-Λ × dt))
P(no contested event) = exp(-Λ × dt)
```

with the zero-Λ case handled exactly. For each enabled non-contested transition,
use `1 - exp(-λ × dt)`, enumerate the finite independent firing combinations,
and apply accepted effects simultaneously from the pre-tick cell.

Required semantic details:

- guards and hazards use the tick-start cell and annual parameter file;
- negative/non-finite hazards fail with the transition and cell named;
- no outcome produced by one transition enables another in the same tick;
- accepted writes to different attributes combine;
- a possible combination writing different values to one attribute is an
  unsupported-model error matching the runtime's refusal, not list-order
  resolution;
- `age_monthly`, `clear_event`, 56 moves, 336 mortality transitions, births,
  overseas arrivals and emigration all use their actual guards/effects;
- movement writes `area`, `prev_area` and event state while preserving row
  identity and generation;
- event-age and generation effects match the model exactly;
- fixed vacant entry slots retain their build-time sex/area/entry-age
  composition; and
- expected mass is conserved across state changes because rows are never
  created/deleted by V1 execution.

Use `math.exp` only in this offline companion. §N3's no-transcendental
foreclosure applies to model expressions, not scientific analysis code.

### 4. One authoritative summary contract

Extract the ordered 126-dimensional calibration-summary definition from
`data/abs/calibrate.py` into `data/abs/summary_contract.py`. Both the existing
micro-output adapter and the expectation runner must call it. Preserve exactly:

- six headline scalars;
- 56 ordered O-D cells;
- normalized interstate sex × 16-age-band composition; and
- normalized ending-stock sex × 16-age-band composition.

The extraction must leave all existing `calibrate.py` outputs and tests
byte-identical. Zero-denominator behavior remains an explicit failure; neither
path manufactures a composition.

The expectation report additionally includes full expected cells, per-tick
transition rewards, total mass, minimum weight, cell count and reconciliation
checks. Canonical JSON uses finite decimal JSON numbers only and records every
input hash, run year, `dt`, tick count and implementation format.

Generate `data/abs/contracts/raw-parity-v1.json` with format
`sembla.abs-raw-parity-contract/v1`. Its exact ordered family is:

1. all 8 × 2 × 21 ending area/sex/model-age-band stock counts in
   area→sex→age-band order;
2. all 418 transition firing counts in the frozen model transition order;
3. the 2 × 16 interstate sex/age composition numerators followed by their one
   total denominator; and
4. the 2 × 16 ending-stock sex/age composition numerators followed by their one
   total denominator.

There are exactly 820 IDs. O-D moves, births, deaths, overseas flows and ageing/
event clearing are represented by their transition IDs rather than duplicated
headline sums. Structural-zero IDs remain present. The contract records its own
ordered-ID SHA-256 and the model/summary-contract hashes. Both micro reduction
and expectation paths must emit a raw-parity map containing exactly these IDs;
no later PRD constructs the family from whichever outputs happen to be
available.

### 5. Command interface

Support at least:

```bash
python3 -m data.abs.expectation \
  --state fixtures/state/australian_population_2010_hundredth.state \
  --model fixtures/state/australian_population_2010_hundredth.state.model.json \
  --params data/abs/params/profiled/2010.json \
  --ticks 12 --out <report.json>
```

The model JSON is used only to validate frozen names, schema, parameter
completeness and transition inventory. The runner remains an explicit
Australian-model companion; it must reject a different model name or transition
shape rather than pretending to be a generic IR interpreter.

Two identical invocations produce byte-identical reports. No command performs
network access or invokes the Sembla executable.

### 6. Tests

Add focused tests for:

- the analytic one-hazard probability;
- two- and three-way competing races summing to one with the no-event branch;
- independent disjoint transitions and simultaneous effect application;
- a forbidden conflicting-write combination;
- frozen tick-start rates and no within-tick cascade;
- movement, death, emigration, birth/arrival, event clearing and ageing on
  hand-derived tiny cells;
- exact expected mass conservation over 12 ticks;
- one-to-one versus duplicate `slot_resource` validation;
- state-reader valid round trips and malformed/truncated/extra-byte failures;
- exact initial grouped counts from the committed 2010 hundredth state;
- complete 418-transition inventory and all 377 annual parameters;
- exact 820-ID raw-parity contract order/hash, structural-zero retention and
  rejection of missing/extra/reordered IDs;
- shared summary-contract equality between a synthetic micro-shaped result and
  the expectation path;
- unchanged existing `calibrate.py` fixture bytes; and
- deterministic report bytes across two executions.

A brute-force finite outcome enumerator for tiny fixtures must independently
match the optimized cell propagator. It is test reference only, not production
fallback.

### 7. Documentation and regeneration

Extend the aggregate-calibration guide with:

- cell sufficiency;
- the contest probability derivation;
- independent-transition combination semantics;
- fixed-pool entry handling;
- exact versus approximate claim discipline; and
- the boundary between unit closure evidence here and empirical parity in PRD
  0004.

Extend `scripts/check-abs-data.sh` to run deterministic expectation unit fixtures
without executing the expensive PRD 0004 replicate experiment.

## Allowed files

- `data/abs/state_artifact.py`
- `data/abs/expectation.py` (new)
- `data/abs/summary_contract.py` (new)
- `data/abs/calibrate.py` (delegation only; outputs unchanged)
- `data/abs/tests/test_state_artifact.py`
- `data/abs/tests/test_expectation.py` (new)
- `data/abs/tests/test_calibrate.py` (shared-contract parity only)
- `data/abs/tests/fixtures/expectation/**` (new, if needed)
- `data/abs/contracts/raw-parity-v1.json` (new)
- `scripts/check-abs-data.sh`
- `docs/guides/australian-population-aggregate-calibration.md`
- implementation notes/artifacts created by the managed run

## Non-goals

- No fitting, profile optimization, micro execution, parity verdict or
  production selection; PRDs 0002 and 0004 own those.
- No generic IR interpreter, Rust backend, ODE solver or state-format change.
- No expected state artifact: fractional cell mass is reported as analysis JSON,
  never written as `sembla.state/v1`.
- No population-dependent feedback, new lifecycle rule, slot reuse or change to
  the Australian model.
- No tolerance selected from PRD 0004 results.

## Acceptance criteria

1. Full repository, ABS, allowlist and diff checks from the README pass.
2. The frozen state writer is byte-unchanged in behavior; the new reader
   validates and semantically round-trips every required valid fixture and
   rejects every malformed fixture explicitly.
3. Analytic contest, independent-transition, no-cascade and effect semantics
   match hand-derived and brute-force tiny references.
4. The 12-tick runner covers the complete Australian transition/parameter
   inventory, conserves expected row mass and emits deterministic v1 reports.
5. Initial aggregate cells exactly recover the committed hundredth state's
   demographic and vacancy counts before propagation.
6. One shared summary contract drives micro and expectation paths; all existing
   calibration outputs remain byte-identical.
7. Unsupported resource mappings, negative/non-finite hazards, conflicting
   writes and zero composition denominators fail rather than being approximated.
8. Documentation states that unit closure evidence is not empirical parity;
   exactness remains unclaimed until PRD 0004.

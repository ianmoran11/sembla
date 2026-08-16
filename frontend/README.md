# Sembla Lean frontend

This Lean 4 package contains Sembla's pure deep IR, the public command-style
modeling language (including the later mathematical finite-domain surface), a
supported compatibility syntax, structure widgets, proofs, and eight
Lean-authored canonical models. The foundational IR-semantics track
pins Mathlib for future checked and semantic layers; the existing raw IR and
surface remain separate. `lean-toolchain` pins Lean 4.13.0, so an `elan`
installation selects the same compiler automatically.

## Setup and build

Install [elan](https://github.com/leanprover/elan), then run from the repository
root:

```sh
cd frontend && lake build
```

Lake resolves pinned Mathlib and ProofWidgets4 dependencies compatible with
Lean 4.13.0. Their complete transitive revisions are recorded in
`lake-manifest.json`.

The default build includes two libraries: `Sembla` is the production import
surface used by `sembla-export` and `sembla-link`; `SemblaTests` imports the
compile-time test and model-validation corpus separately. This keeps tests out
of executable import closures without weakening the default `lake build`
contract. `frontend/scripts/check-imports.py` enforces the split and the main
contract/semantics/composition import directions.

## Public command surface

Human-authored models use `sembla_model`. The overview below covers the command
foundation; the complete current domain/partition/function/alias/relation
surface is documented in the
[mathematical model guide](../docs/guides/mathematical-model-surface.md). The command header defines an ordinary
namespace-respecting `Sembla.IR.Model` constant. `(dt := ...)` is mandatory;
`(name := ...)` is optional and sets the exact runtime model name.

```lean
namespace Example
open Sembla.IR Sembla.DSL

sembla_model WorkplacePolicy
    (name := "workplace_policy")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      visits : Int
      employer : Employer
    system Employer (rows := 50)

    input restriction where
      modifier : ℝ

    infect on Person : health: S →[
      β · freq (health = I) over employer ·
        (1.0 + inputSum restriction field modifier)
    ] I
    recover on Person : health: I →[γ] R

    output activity from Person where
      infected : Int := count where health = I
      total_risk : ℝ := sum (risk)

    views Person where
      infectious := count where health = I
      total_risk := sum risk
      minimum_visits := min visits
      active_risk_max := max risk where health = I

  box policy where
    system Controller (rows := 1) where
      mode : {Open, Restricted}
      modifier : ℝ

    input activity where
      infected : Int
      total_risk : ℝ

    transition restrict on Controller where
      guard mode = Open ∧ inputSum activity field infected > 100
      hazard 1e300
      set mode := Restricted
      set modifier := 0.4

    output restriction from Controller where
      modifier : ℝ := sum (modifier - 1.0)

  wire population activity -> policy activity
  wire policy restriction -> population restriction

  summaries population where
    peak_I := max infectious
    peak_tick := argmaxₜ infectious

end Example
```

There are no list brackets, separator commas, or required empty category
blocks. Model declarations and box declarations may be interleaved. Collection
is multi-pass, so parameters, systems used as Ref targets, ports, and views may
be referenced before their declarations wherever the semantic kernel permits
it. Every emitted IR category is a stable partition of source declarations:
relative textual order within params, boxes, systems, transitions, fields,
effects, views, wires, and summaries is preserved.

### Names, parameters, and expressions

Without a model-name override, the Lean declaration identifier is converted by
the frozen snake-case derivation (`SirWorkplace` becomes `sir_workplace`). A
system identifier uses the same derivation for its table name (`Person` becomes
`person`). Use `(name := "...")` after the model or system identifier whenever
the wire-format name must differ:

```lean
sembla_model observations (dt := 1.0) where
  box population where
    system Person (name := "Person") (rows := 4) where
      status : {active, inactive}
```

Greek identifiers use documented transliterations, so `β`, `γ`, `σ`, and
`«λ_parent»` produce `beta`, `gamma`, `sigma`, and `lambda_parent`. Derivation
collisions are errors rather than name-dependent resolution.

A parameter declaration binds its identifier throughout the model. References
are bare names, not substituted defaults:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
```

The demographic-slot surface additions also accept integer parameters with
integer-literal defaults, including negative literals:

```lean
param retirement_months : Int := 780
param offset : Int := -3
```

`Int` parameters lower to the existing integer IR parameter type and can be
used anywhere the scalar expression typechecker accepts `Int`, including guards
and integer `set` effects. Priors remain real-valued: attaching any prior to an
`Int` parameter is rejected. Defaults and optional `Normal` or `LogNormal`
priors on real parameters remain first-class IR metadata. The current surface
accepts `ℝ`, mathematical multiplication `·`, conjunction `∧`, negation `¬`,
inequality `≠`, and comparisons `≤` and `≥`. Its ASCII operators are `*`, `/`,
`+`, `-`, `=`, `<`, `>`, and `&&`; there are deliberately no ASCII `!=`, `<=`,
or `>=` forms. Expressions also support numeric arithmetic, enum comparisons,
`inputSum`, and the restricted aggregate forms described below. Real values are stored as exact
coefficient/exponent `Scientific` data, preserving supported finite
`f64`-range decimals.

### Indexed parameter and transition families

Finite compile-time domains can expand a demographic table without adding a
tensor node to the IR:

```lean
index age := 0 .. 120
index sex := {male, female}

param β[age, sex] : ℝ where
  [0, male] := 0.10
  [0, female] := 0.09
  -- every remaining cell is required
```

Use the same indexes on an explicitly selected transition:

```lean
infect[age, sex] on Person : health: S →[
  β[age, sex] · freq (health = I) over employer
] I
```

The selected system must have same-named compatible attributes. Expansion is
canonical and produces ordinary names such as `beta_0_male` and
`infect_0_male`. Large complete tables may use strict source-relative CSV or
JSON declarations, each with a mandatory exact-byte SHA-256 pin. See the
[indexed-family guide](../docs/guides/indexed-parameter-families.md) for schemas,
ordering, expansion limits, hashing and regeneration.

### Named mathematical domains and relations

The indexed forms above remain supported. New mathematical models can also use
named enum/range domains as attribute types, projected integer partitions,
parenthesized parameter calls, complete finite expression functions, reusable
`state` aliases, and filtered `relation` declarations. All are checked static
sugar that expands to the same scalar IR; no runtime tensor, domain, alias, or
relation node is introduced. The syntax, restrictions, deterministic ordering,
mixed-prior v2 tables, and Australian production example are in the
[mathematical model guide](../docs/guides/mathematical-model-surface.md).

### Systems, attributes, and forward references

Every system requires `(rows := <natural-number literal>)`. This is an IR
`Table.sizeHint`, not runtime population initialization. A nonempty system uses
an indented `where` block; an empty system omits it. Attribute types are inferred
from exactly these forms:

```lean
state : {Open, Restricted} -- enum/state
risk : ℝ                    -- real
visits : Int                -- integer
employer : Employer         -- Ref to a collected system
```

Enum variants and attributes retain textual order. Ref targets must be systems
in the same box, but can be declared later because collection precedes
resolution.

### Reaction arrows and general transitions

Use a reaction arrow when a transition is exactly one enum equality guard and
one write to that enum attribute:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
recover on Person : health: I →[γ] R
```

`on Person` may be omitted only when the compatible system is unique. The state
attribute may be omitted only when the selected system has exactly one enum
attribute; a system with multiple enum attributes always requires explicit
`health:` syntax, even if only one contains both endpoint variants. Explicit
`on Person` and `health:` are the stable disambiguation forms; ambiguous
inference is rejected rather than chosen by iteration order.

Use the general form for extra guards, multiple effects, or controller rules:

```lean
transition restrict on Controller where
  guard mode = Open ∧ inputSum activity field infected > 100
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

A general transition requires exactly one `guard`, exactly one `hazard`, and at
least one ordered `set`. As a demographic-slot surface addition, numeric effects
accept the existing row-local scalar expression fragment, so old-snapshot
updates such as `set age_months := age_months + 1` are valid. The expression
must have exactly the destination attribute's type; there is no additional
coercion. Enum effects remain variant literals, Ref writes remain rejected, and
aggregate forms such as `countBy`, `freq`, and `inputSum` are rejected in effect
expressions.

### Race-time contests

A general transition may claim one or more Ref-valued row resources in declaration
order:

```lean
transition depart on Slot where
  guard occupancy = present
  hazard departure_hazard
  contest slot_resource by race_time
  set occupancy := vacant
  set cause := departure
```

`race_time` is the only exposed ordering. Keyed orderings and queue disciplines
remain deferred to v0.5 by `DECISIONS.md` §K7. Duplicate claims are rejected, and
reaction-arrow sugar cannot carry contests; use the general transition form.
This addition does not permit Ref writes.

The surface validates resource names, Ref types, and duplicates. The Rust
validator retains the deeper claim/write coverage checks, while the runtime owns
argmin conflict resolution, deferred-loser reporting, and the double-write
defense.

### Grouped observations

Grouped count views are authored in a table-scoped `views` block with one to
four Enum, Ref, or banded Int keys. The presence of `by` selects grouped-view
lowering:

```lean
views PersonSlot where
  population := count where occupancy = present
  population_cells := count
    where occupancy = present
    by sex, area, band(age_months, 60)
```

The individual `view ...` and `grouped view ...` forms remain supported as
long-form compatibility syntax.

The surface always elaborates the complete `GroupedViewDecl`; authoring has no
runtime feature context. Execution is default-off and requires repeatable
`--enable grouped-observations` on `sembla run` or `sembla sweep`, as frozen by
`DECISIONS.md` §K6. Int keys require a positive literal band width; Enum and Ref
keys must not use `band`. Filters are row-local count predicates.

V1 executes grouped observations on CPU only. Each declaration writes
`<out-stem>.grouped.<view>.csv` with header
`tick,<key1>,…,<keyN>,count`, non-empty groups only, and numeric key-tuple
ordering before Enum names are rendered. Grouped observation is a sink: it is
evaluated after commit and cannot consume draws, stage writes, affect conflicts,
or feed scheduling.

### Keyed frequency

`freq (predicate) over key` is the supported frequency-shaped aggregate. It
means the exact legacy `countBy key (predicate) / sizeBy key` plan and does not
introduce a general query language. It requires:

- a transition, output, or view context with a selected source system;
- `key` to be a Ref attribute of that system; and
- a Boolean row-local predicate over the selected system.

Input aggregates, relational aggregates, nested `freq`, non-Ref keys, and
predicates that leave the row-local fragment are rejected with positioned
diagnostics. Keyed comprehensions with row binders (option C(i)) remain deferred
until a real model requires them.

### Inputs, outputs, views, wires, and summaries

Inputs declare ordered schemas with the same enum/real/integer/Ref type family:

```lean
input activity where
  infected : Int
  total_risk : ℝ
  mode : {Open, Restricted}
  subject : Person
```

`inputSum port field column` remains limited to numeric schema fields. Outputs
simultaneously declare an ordered schema and builders:

```lean
output activity from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)
```

Count builders have a filter and no value; sum builders have a value. The
preferred observation syntax scopes a block to one source table. Views use
`count`, `sum`, `min`, or `max`; numeric reductions take their value expression
directly, and an optional `where` filter remains row-local. A `by` clause makes
the entry a grouped count view:

```lean
views Person where
  infectious := count where health = I
  total_risk := sum risk
  minimum_visits := min visits
  active_risk_max := max risk where health = I
  active_age_cells := count
    where health = I
    by employer, band(visits, 5)
```

The existing one-line forms remain available when declarations from different
tables need to be interleaved.

Wires use four endpoint identifiers and ASCII `->`. Schemas must match and a
destination may be delivered to only once:

```lean
wire population activity -> policy activity
```

Model-level summaries fold ordinary view streams with exactly `sum`, `min`,
`max`, `last`, or `argmaxₜ`; `argmaxₜ` returns the earliest tick attaining the
maximum. A scoped block writes the box once:

```lean
summaries population where
  final_infectious := last infectious
  peak_infectious := max infectious
  peak_tick := argmaxₜ infectious
```

The individual `summary name := reduce box.view` form remains supported. Views
are observation sinks and cannot feed transitions except through explicit
output/input ports and one-tick-delayed wires.

## Compatibility and machine-writer paths

`model%` remains a supported low-level compatibility and semantic-kernel form.
It is intentionally retained for old models and the legacy/new syntax-twin
regression suite. Both `model%` and `sembla_model` collect the same surface-model
records and invoke the same uniqueness, scope, expression, schema, ordering,
widget, and IR-emission kernel; neither frontend has separate semantics.

Generated models and low-level tests may construct `Sembla.IR` values directly.
That direct-constructor API is the machine-writer path, not the recommended
human surface. A do-notation builder (option E) is rejected/deferred for human
authoring because it reads as imperative construction rather than mathematics.

Complete ill-formed models under `frontend/Negative/` pin full ordered sets of
positioned errors. Focused positives and syntax twins cover forward references,
name derivation, priors, arrows, frequencies, stable ordering, and command/legacy
byte equality. Run them from the repository root with:

```sh
cd frontend && bash scripts/test-negative.sh
```

## Export, validation, and parity

Lean elaborates, inspects, renders structure widgets, proves specification-level
results, and serializes models. Rust validates whole exported models and
executes them. The Rust runtime remains outside Lean; target 1a is proved at the
specification level while evaluator-level target 1b remains open.

Representative export and validation commands are:

```sh
cd frontend
lake exe sembla-export sir /tmp/sir.json
lake exe sembla-export Sembla.Models.sirPolicy /tmp/sir_policy.json
lake exe sembla-export observations /tmp/observations.json
cd ..
cargo run -p sembla-cli -- validate /tmp/sir.json
cmp examples/sir.json /tmp/sir.json
cargo run -p sembla-cli -- diff-ir examples/sir.json /tmp/sir.json
```

The exporter accepts every concise snake-case and camel-case spelling plus the
existing `Sembla.Models.*` and `Sembla/Models/*` qualified aliases. `diff-ir` is
a useful normalized semantic comparison, but it does not replace literal byte
comparison. The parity script exports all eight canonical models and every
accepted alias, validates both sides, and uses literal `cmp` against checked-in
fixtures before supplemental `diff-ir` checks:

```sh
bash frontend/scripts/check-parity.sh
```

It also runs checked and exported models with fixed seeds, comparing CSV bytes,
summaries, final-state hashes, and output hashes while asserting nontrivial
dynamics and conserved state counts. No fixture regeneration is part of the
workflow. The parity command requires Git plus both pinned Rust and Lean
toolchains. For Rust-only validation without Lake, run:

```sh
./scripts/check-rust.sh
```

The strict complete repository check requires Cargo, Git, and Lake and never
silently skips frontend validation:

```sh
./scripts/check.sh
git diff --check
```

The canonical [local check contract](../docs/contributing/ci.md#local-check-contract) also
lists determinism, the reduced NPE smoke test, and manual GPU evidence with the
required environments.

The canonical-model catalog, formulas, run commands, and current limits are in
[`docs/examples/canonical-models.md`](../docs/examples/canonical-models.md).

## Proofs

`Sembla.Semantics` and `Sembla.Frontend.Builders` are importable architecture
umbrellas for the proposed foundational formalization; PRD 0001 adds no
semantic definitions. `Sembla.Semantics.ProofAudit` provides the deterministic
environment inventory used by `scripts/check-proofs.sh`. It enumerates every
theorem/lemma in the covered module roots, rejects transitive axioms outside
`{propext, Classical.choice, Quot.sound}`, and is paired with mechanical source
policy and cleaned-up negative self-tests.

`Sembla.LumpingProof` proves `groupedCount_eq_naiveCount`, exact agreement of
the grouped and naive coworker-count plans, and `plan_rewrite_congr`, which
transports that equality through any per-row function of the count. This is
theorem target 1a at the specification level, not a theorem about the
deep-embedding evaluator; target 1b remains open.

Run the proof-hygiene guard directly, or use the complete repository check
(which includes it):

```sh
bash frontend/scripts/check-proofs.sh
./scripts/check.sh
```

## Widgets

The frontend pins ProofWidgets4 `v0.0.44` for Lean 4.13. Pure functions in
`Sembla.Widgets` build JSON-encodable props from the already-elaborated `Model`;
`Sembla.WidgetDisplay` turns those props into HTML/SVG and registers infoview
panels. Neither path invokes the Rust runtime or performs simulation.

Widgets default to the restrained `academic` preset. A source file may select
any of the three themes before its model declarations:

```lean
set_option sembla.widget.theme "academic" -- also: "editor" or "notebook"
```

`editor` follows standard VS Code widget chrome; `notebook` is softer and more
rounded. `professional` remains an alias for `academic`. All themes inherit the
active VS Code foreground/background for dark and high-contrast support.

To verify widgets manually:

1. Run `cd frontend && lake build`, open the repository with the VS Code Lean 4
   extension, and open the Lean infoview.
2. In `frontend/Sembla/Models.lean`, place the cursor on the `Person` system
   declaration in `sir`. Expect **State machine — person**, states `S`, `I`,
   `R`, and edges `infect` and `recover`.
3. Place the cursor on the `infect` arrow declaration. Expect its distinct
   transition panel, wrapped `freq` hazard, beta default and LogNormal prior,
   and the aggregate-dependent no-probability-plot explanation.
4. Place the cursor on the `recover` arrow declaration. Expect its distinct
   panel, gamma metadata, prior-density plot, and monotone
   `p(dt) = 1 - exp(-lambda * dt)` chart.
5. In `frontend/Sembla/Demos/Modeling.lean`, place the cursor on the general
   `restrict` transition and confirm it has a distinct panel from the system
   and reaction declarations.
6. Resize the infoview to roughly 280--320 pixels and repeat the checks in dark,
   light, and high-contrast VS Code themes. Headers, state labels, axes, cards,
   series, outlines, and text must remain legible without overlap.

Automated props and rendering-structure assertions live in
`Sembla/WidgetTests.lean` and `Sembla/CommandFrontendTests.lean`. They cover
state graphs, hazards, probability/prior plots, responsive SVG, long labels,
loops and opposing routes, badges and empty states, JSON encoding, and all three
theme presets. Final layout and theme verification remains intentionally manual.

## Observation and complete-model builder boundary

`Sembla.Frontend.Builders.Observation` is the proved, syntax-independent final
frontend boundary. `ObservationRaw` preserves input, output, scalar-view,
grouped-view and summary fields exactly. `CompleteModelSpec` embeds exactly one
`TransitionOverlaySpec`, attaches observations by `Fin` box ordinal, preserves
summaries and opaque raw wires in source order, derives declaration and term
contexts from the complete input-bearing candidate, delegates current race-only
transitions through `buildSurfaceTransition`, and certifies the final raw model
with `checkModel`. Successful certification exposes exact checked erasure.
`CompleteModelSpec.toRaw` is the sole complete raw assembly path; the command
frontend no longer constructs `IR.Box` or `IR.Model` directly. Its private
fragment-to-dependent-index adapter is trusted token/bookkeeping glue.

The following remain trusted and regression-tested rather than verified: parser
expansion, family/alias/partition lowering, current surface-shape compatibility
checks, the two declaration-only compatibility validators described below,
source-token bookkeeping, structured category/path-to-position mapping,
diagnostic rendering, metaprogram evaluation and result splicing, widget
attachment, and composition/wire compatibility checks. Wires are preserved
exactly but the builder makes no `WiresWellFormed`, delivery, linker or
composition claim. The existing negative harness remains the oracle for exact
message positions.

Raw IR V1 has no state-alias declaration, so an alias is recorded as **used**
only when the single transition expansion that emits the raw candidate also
records at least one alias-derived guard atom or effect in emitted provenance.
An alias with no such emitted contribution—including an alias referenced only
by a zero-instance family—is **unused**. The trusted, not proved,
`trustedValidateUnusedStateAliasCompatibility` path runs at most once for each
unused alias in declaration order. It retains only assignment-destination
lookup, Ref-assignment rejection, Enum-member validation, scalar assignment
compatibility, expression name/type validation, and the predicate-`Bool`
requirement. Every emitted guard/effect from a used alias bypasses that
compatibility path and is checked exactly once by the transition builder/final
checker; macro lowering only selects raw encodings and retains source tokens and
ordinals.

Raw IR V1 likewise has no expression-function declaration or finite cell table.
The trusted, not proved, `trustedValidateExprFunctionCompatibility` path runs
exactly once after all compile-time parameter/family/domain/partition/function
declarations are collected and before substitution. In cell source order it
retains only empty-row-scope expression name/type validation, formal/domain
substitution compatibility and the declared cell-result sort requirement;
function-table completeness, duplicate/recursive/aggregate and expansion-cap
rules remain surface expansion checks. The validator is never called from a
function application, lowering, token mapping or diagnostic rendering, and its
result is not checker evidence. Every substituted raw expression is emitted once
and remains fully checked by the authoritative transition/model checker.

# Mathematical model surface

The mathematical surface lets a finite family of parameters and rules read like
the model it represents while preserving Sembla's existing scalar IR. Domains,
partitions, finite functions, state aliases, and relations are **authoring-time
syntax**: Lean validates and expands them into ordinary scalar parameters,
guards, effects, resource claims, and transitions. Rust and CUDA receive no
new domain, tensor, lookup, alias, or relation node.

The older `index` declarations, `β[age, sex]` references, indexed arrows and
indexed general transitions remain supported. Use them when maintaining an
existing model; the named-domain forms below are the clearer public style for
new mathematical models.

## Named finite domains and attributes

Declare ordered enum or inclusive non-negative integer domains at model level:

```lean
domain Area := {nsw, vic, qld}
domain ExactAge := 0 .. 120
```

A domain name may be used directly as an attribute type:

```lean
system Person (rows := 1_000) where
  area : Area       -- lowers to enum {nsw, vic, qld}
  age : ExactAge    -- lowers to Int; this is not a runtime range constraint
```

Enum order and integer order are semantic expansion order. A named range is a
finite compile-time domain but its attribute still lowers to the ordinary `Int`
type; rows outside the declared range are not rejected by the runtime. Domain
and system names may not be ambiguous.

The legacy equivalents remain valid:

```lean
index area := {nsw, vic, qld}
index age := 0 .. 120
param rate[area, age] : ℝ where
  -- one cell for every pair
```

## Projected partitions

A partition names contiguous half-open intervals of one specific `Int`
attribute:

```lean
partition AgeBand projects population.Person.age_months where
  «00_04» := 0 ..< 60
  «05_09» := 60 ..< 120
  «10_plus» := 120 ..
```

The target must have the exact `box.System.attribute` form and resolve to an
`Int` attribute. Bounds are natural-number literals. Every bounded interval
must have an upper bound greater than its lower bound; each later lower bound
must equal the preceding upper bound; and the final, and only the final, cell
must be open-ended. The first lower bound need not be zero, so values below it
match no cell. Labels that begin with digits use Lean escaped identifiers such
as `«00_04»`; their generated suffix is still `00_04`.

`AgeBand` is then an ordered finite domain. In a relation, `AgeBand(band)` in a
`from` list lowers to `age_months >= lower` and, for a bounded cell,
`age_months < upper`. The indexed fallback `age_months ∈ band` has the same
lowering inside a transition family. A projected partition is a predicate over
an existing integer column, not an enum column, and cannot appear after
`become`.

## Parameters, calls, and priors

Mathematical parameter families use typed binders and call notation:

```lean
param mortality (area : Area, band : AgeBand) : ℝ where
  [nsw, «00_04»] := 0.0001 ~ LogNormal (-9.210340371976184) 0.5
  [nsw, «05_09»] := 0.0 ~ Normal 0.0 0.00001
  -- every remaining Cartesian cell exactly once

-- in a bound relation or transition
hazard mortality(area, band)
```

This emits scalar parameters such as `mortality_nsw_00_04`; a call is resolved
statically to the scalar belonging to the current bound values. Defaults and
prior arguments retain exact supported decimal/scientific text. Real scalar
and family cells accept either `Normal location spread` or
`LogNormal location spread`; cells may mix the two families. Integer parameters
and families require integer defaults and cannot carry priors.

Legacy `param mortality[area, band]` and `mortality[area, band]` forms remain
accepted and lower identically. Parenthesized mathematical declarations name
the domain of each binder explicitly, which also prevents accidental reuse of
a same-spelled binder from an incompatible domain.

## Complete finite expression functions

A finite expression function is a compile-time case table returning `ℝ` or
`Int`:

```lean
domain Level := 0 .. 1

function Identity (level : Level) : Int where
  [0] := level
  [1] := level
```

At least one typed argument is required, argument names must be unique, and the
table must contain every Cartesian cell exactly once. Each cell is elaborated
with its formal arguments bound to that cell's domain values, so the example
really returns its formal rather than coincidentally repeating literals. Calls
such as `Identity(level)` or `Identity(1)` accept only bound values or domain
literals and are replaced by the selected scalar expression.

Functions are finite inline expression tables, not runtime functions: they have
no recursion or function-to-function calls, no aggregates, and no row-local
attribute context. Every cell must have the declared numeric result type. They
may refer to scalar parameters, parameter-family cells selected by their formal
bindings, literals, and the supported scalar expression operators.

## State aliases: matching and assignment

State aliases name reusable row-local conditions on one system:

```lean
state Present on Person where
  occupancy := present

state InArea (region : Area) on Person where
  area := region

state Eligible on Person where
  match age_months ≥ 18 * 12
```

An assignment atom (`attribute := value`) has two roles. In `from`, it lowers to
an equality predicate. After `become`, it lowers to an ordered `setAttr` effect.
Assignments must have the attribute's exact type; Ref-valued attributes cannot
be matched by assignment. A `match` atom contributes an arbitrary supported
row-local Boolean predicate and is valid only for matching in `from`, never for
`become`. Aggregates are rejected in all alias bodies.

Aliases may contain multiple atoms and typed domain arguments. An application
must supply every argument, using a compatible enclosing binder or a domain
literal. Alias names are box-local and are not emitted into the IR.

## Relations and finite constraints

A relation expands one rule over typed domains:

```lean
relation move (origin : Area, destination : Area) on Person
    subject to origin ≠ destination where
  from Present, InArea(origin)
  hazard movement(origin) * Pull(destination)
  claim slot_resource by race_time
  set previous_area := origin
  become InArea(destination)
  set event := moved
```

A relation requires at least one binder, exactly one `from` list, exactly one
hazard, and at least one effect (from `set` or `become`). `subject to` accepts a
comma-separated set of binder equality or inequality constraints (`=` and `≠`).
Both operands must be relation binders over the same domain. Constraints filter
the finite expansion at compile time; they do not become runtime guards.

The `from` applications are the runtime guard. Each alias expands in its atom
order, and applications are conjoined in written order. Partition applications
lower to their interval predicates at that same position. Relation binders do
not silently add same-named attribute guards: express those restrictions with
aliases or a projected partition.

Effects and claims preserve body order independently. Each `set` emits one
effect; `become` emits the selected alias's assignment atoms in alias order.
Claims retain declaration order and use the existing `race_time` resource-claim
semantics. Predicate aliases and partitions cannot be used after `become`.

## Deterministic expansion, names, order, and caps

All finite declarations enumerate the Cartesian product in domain source order,
with the leftmost binder varying slowest. Inline and external table row order
does not change emission order. Relation constraints then retain matching
combinations without reordering them. For example, ordered `Area := {nsw, vic,
qld}` and `subject to origin ≠ destination` begins `move_nsw_vic`,
`move_nsw_qld`, `move_vic_nsw`.

Generated names are the derived base name followed by derived member components,
joined with underscores. Numeric-leading escaped labels are legal suffixes.
Flattened-name collisions are errors. Expanded parameters and transitions occupy
the source position of their declaration; guards, effects, and claims retain the
association and order described above.

One finite parameter family, transition family, relation, or expression function
may expand to at most 10,000 Cartesian cells by default. The cap is checked on
the unfiltered Cartesian size, so a relation constraint does not make an
oversized declaration acceptable. Raise the shared cap only around a reviewed
model:

```lean
set_option sembla.maxFamilyExpansion 20000 in
sembla_model LargeReviewedModel (dt := 1.0) where
  -- ...
```

## Pinned external CSV and JSON tables

Large parameter families may be stored outside Lean, but remain part of the
trusted source input:

```lean
param mortality (area : Area, band : AgeBand, sex : Sex) : ℝ
  from json "Data/mortality.json"
  sha256 "<sha256 of exact file bytes>"

param mortality_csv (area : Area, band : AgeBand, sex : Sex) : ℝ
  from csv "Data/mortality-v2.csv"
  schema "sembla.parameter-family/v2"
  sha256 "<sha256 of exact file bytes>"
```

Paths must be relative and are resolved against the Lean file containing the
declaration, not the shell working directory. Absolute paths are rejected.
Relative paths are not sandbox-confined: `..` and symlinks can escape the source
directory. The declaring Lean source and the files it names therefore share one
source trust boundary.

The SHA-256 pin is mandatory, lowercase hexadecimal, and checked over exact
bytes before UTF-8 decoding or parsing. This makes line endings and final
newlines significant. Updating a table requires reviewing the bytes and changing
the hash literal too; that source edit also avoids a stale Lake `.olean` when
only an arbitrary data file changed.

CSV uses the exact dimension columns followed by:

```csv
area,band,sex,default,prior_family,prior_arg_1,prior_arg_2
nsw,00_04,male,0.0001,log_normal,-9.210340371976184,0.5
nsw,05_09,male,0.0,normal,0.0,0.00001
```

The original unversioned CSV form is v1 and permits only `log_normal`. Explicit
CSV v2 requires `schema "sembla.parameter-family/v2"` and permits exactly
`normal` and `log_normal`. JSON carries its version inside the document:

```json
{
  "schema_version": "sembla.parameter-family/v2",
  "dimensions": ["area", "band", "sex"],
  "cells": [
    {
      "key": {"area": "nsw", "band": "00_04", "sex": "male"},
      "default": "0.0001",
      "prior": {"family": "log_normal", "args": ["-9.210340371976184", "0.5"]}
    },
    {
      "key": {"area": "nsw", "band": "05_09", "sex": "male"},
      "default": "0.0",
      "prior": {"family": "normal", "args": ["0.0", "0.00001"]}
    }
  ]
}
```

JSON v1 and v2 are accepted; v1 permits only `log_normal`, while v2 permits the
mixed prior families. Defaults and prior arguments are strings. Both formats
require one and only one in-domain cell for every Cartesian key and reject
missing, duplicate, extra, out-of-domain, malformed, and unknown-field data.
`prior` may be absent/null (JSON) or all three prior columns may be empty (CSV).

Compute a pin with `shasum -a 256 path/to/table`.

## Scoped views and summaries

Use a `views` block when several observations share one source table. Ordinary
and grouped observations use the same declaration shape; the presence of `by`
selects grouped-view lowering:

```lean
box demographic where
  system PersonSlot (rows := 352_460) where
    occupancy : {vacant, present}
    event : {none_, birth, death}
    sex : {male, female}
    age_months : Int
    generation : Int

  views PersonSlot where
    population := count where occupancy = present
    births_this_tick := count where event = birth
    max_generation := max generation

    population_cells := count
      where occupancy = present
      by sex, band(age_months, 60)

summaries demographic where
  final_population := last population
  births_total := sum births_this_tick
  final_max_generation := last max_generation
```

A block-scoped numeric view writes its value expression directly (`sum risk`,
`min visits`, or `max generation`). Grouped observations remain count-only and
accept one to four Enum, Ref, or banded Int keys. `band(column, width)` requires
a positive integer literal and lowers to the same grouped key as the legacy
`band column width` spelling.

`summary`, `view`, and `grouped view` individual declarations remain supported.
The scoped forms are additive syntax sugar: they use the existing semantic
checks and lower to the same ordered `ViewDecl`, `GroupedViewDecl`, and
`SummaryDecl` values without new runtime node kinds or feature changes.

## Production lowering and the Australian model

The production authority for the Australian population model is
[`Surface.lean`](../../frontend/Sembla/Models/AustralianPopulation/Surface.lean)
plus its four pinned JSON v2 tables. It demonstrates eight-state domains, a
21-cell projected monthly-age partition, mixed `Normal`/`LogNormal` mortality
priors, finite push/pull functions, aliases, filtered origin/destination
relations, ordered effects, and row-resource claims:

```lean
relation move (origin : Area, destination : Area) on PersonSlot
    subject to origin ≠ destination where
  from Present, NoEvent, InArea(origin)
  hazard ((interstate_base * Push(origin)) * Pull(destination)) *
    (1.0 / (1.0 + k * ((age_months - peak_months) *
      (age_months - peak_months))))
  claim slot_resource by race_time
  set prev_area := origin
  become InArea(destination)
  set event := interstate_move
  set event_age_months := age_months
```

Lean lowers the complete source to the established 377 scalar parameters and
418 scalar transitions. `Parameters.lean` and `Transitions.lean` are
compatibility projections of that authority. The generated
`data/abs/reference/AustralianPopulationParameters.lean` is reproducible
377-parameter structural evidence only, never a production import; large state
artifacts remain ignored under `data/abs/generated/`.

For a checked compact example, see
[`Step09_MathematicalSurface.lean`](../../frontend/Sembla/Tutorial/Step09_MathematicalSurface.lean).
For external-table details and the legacy indexed spellings, see
[indexed parameter families](indexed-parameter-families.md).

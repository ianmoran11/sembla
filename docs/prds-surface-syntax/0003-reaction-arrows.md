# PRD 0003: Reaction-arrow transition syntax

## Context

Read the folder README first; its constraints bind. PRDs 0001–0002 provide one
semantic kernel, bound parameters, derived names, and mathematical expression
aliases. This PRD implements option A: the dominant one-state-change CTMC form
becomes one reaction line while the general transition declaration remains for
multi-guard/multi-effect rules.

## Goal

Reaction arrows lower deterministically to the exact existing guard/hazard/effect
IR, preserve source anchors, and reject ambiguous or invalid inference with
positioned diagnostics.

## Specification

### 1. Add the four frozen arrow forms

Inside the existing transition list/category, accept:

```lean
infect : S →[β] I
infect on Person : S →[β] I
infect : health: S →[β] I
infect on Person : health: S →[β] I
```

The hazard is a `semblaExpr`, so option-B aliases and bare parameters work.
Whitespace may follow Lean's normal token rules, but the tokens `:`, `→[`, `]`,
and the source/destination identifiers are mandatory. Keep the existing:

```lean
transition name on System where
  guard ...
  hazard ...
  set [...]
```

for compatibility and for any rule not expressible as one enum-state change.

### 2. Exact lowering

After inference, an arrow named `infect`, selected system/table `person`, enum
attribute `health`, source `S`, destination `I`, and hazard `h` must emit exactly:

```lean
Transition.mk "infect" "person"
  (Expr.enumIs "health" "S")
  h
  [Effect.setAttr "health" (Expr.enum "I")]
  []
```

Use the shared PRD 0001 validation/emission path. Do not add an arrow-specific
IR constructor or bypass existing hazard/effect type checks. An arrow always
has one guard, one effect, and no resource claims. Source and destination may
be equal; self-loops are valid.

### 3. Deterministic inference

Build inference from the fully collected enclosing box before selecting
anything. System and state-attribute inference are separate:

1. `on Person`, when present, selects that logical system. Without it, infer a
   unique compatible system; zero or multiple systems are positioned errors
   that instruct the user to add `on System`.
2. `health:`, when present, selects that attribute on the selected/inferred
   system. Without it, the system must have **exactly one enum/state attribute**.
   If it has zero or multiple enum attributes, reject and instruct the user to
   add `attribute:`. Do not choose among multiple enum columns merely because
   only one happens to contain the named variants.
3. The selected attribute must be enum-typed and contain both source and
   destination. Source/destination variants must belong to that same attribute.

When a label is present without `on`, compatible-system inference may use that
attribute name/type/variants, but it must still yield one system. Diagnostics
list relevant systems/state columns in stable source order. Never choose the
first declaration, and never use declaration order as a tie-breaker. Inference
must work across forward-declared systems because collection is multi-pass.

### 4. Source anchors and widgets

Retain separate original tokens for arrow name, optional system, optional
attribute, source, hazard, and destination. Attach the state diagram and hazard
panel to the arrow declaration/name in the same way as the equivalent general
transition. Add widget-data assertions proving that arrow and expanded twins
produce identical `StateDiagramProps` and `HazardPanelProps`.

### 5. Byte twins

Add imported/focused positive models for:

- inferred unique system with exactly one state attribute;
- explicit system with inferred sole state attribute;
- explicit state attribute with inferred unique system;
- explicit system plus state attribute;
- self-loop;
- Greek/bare-parameter hazard;
- a hazard using current legacy `countBy`/`sizeBy`; and
- two fully collected systems where only one is a valid candidate.

For each representative form, compare the arrow model to a general-transition
legacy twin using both structural equality and exact `IR.toJson` equality.
Transition/effect order must match.

## Required negative coverage

Add complete positioned failures and exact script expectations for:

- unknown source variant;
- unknown destination variant;
- source/destination found only on different enum attributes;
- unknown explicit system;
- unknown explicit state attribute;
- explicit non-enum attribute;
- no inferred candidate;
- multiple inferred systems (must prescribe `on System`);
- multiple enum attributes on one selected system even when only one contains
  the variants (must prescribe `attribute:`);
- non-Real hazard; and
- syntax that attempts extra guards/effects after an arrow (a clear
  arrow-limitation diagnostic, not accepted trailing syntax).

Diagnostics should name candidates/restrictions and point at source,
destination, system, attribute, or hazard tokens as appropriate. Add at least
one positive disambiguation counterpart for every ambiguity negative.

## Allowed files

- `frontend/Sembla/DSL.lean`
- surface/positive/negative test modules
- `frontend/Sembla.lean` if test registration is needed
- `frontend/scripts/test-negative.sh`
- widget data tests needed to prove source/props parity

## Non-goals

`freq`, set comprehensions, command blocks, multi-effect arrow semantics,
resource-claim syntax, canonical migration, IR changes, or visual widget work.

## Acceptance criteria

1. All four frozen arrow forms compile and lower to exactly one existing enum
   guard, supplied Real hazard, one existing enum assignment, and no contests.
2. Every arrow/expanded twin is structurally and byte-for-byte identical;
   widgets derived from each twin are equal.
3. Inference uses the complete candidate set and never declaration order;
   explicit system/attribute forms resolve all covered ambiguities.
4. Required negative cases fail with exact positioned diagnostics and self-loops
   remain accepted.
5. Legacy general transitions and all canonical exports remain unchanged and
   byte-identical.
6. No IR/JSON/Rust/dependency/doc changes occur.
7. All required README checks and `git diff --check` pass.

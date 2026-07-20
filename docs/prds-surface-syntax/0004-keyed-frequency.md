# PRD 0004: Keyed frequency aggregate

## Context

Read the folder README first; its constraints bind. PRDs 0001–0003 provide one
semantic kernel, mathematical identifiers/operators, and reaction arrows. The
remaining unreadable part of the canonical infection hazard is the paired
`countBy`/`sizeBy` expression. This PRD implements option C(ii), the exact
frequency idiom used throughout the epidemiological models. Option C(i)'s set
comprehension remains explicitly deferred.

## Goal

`freq (<predicate>) over <ref>` is a typed, row-local expression that lowers to
the exact existing keyed count divided by keyed size, with byte twins and
teaching diagnostics for every fragment violation.

## Specification

### 1. Add exactly one atomic expression form

Add to `semblaExpr`:

```lean
freq (health = I) over employer
```

Parentheses around the predicate and the `over` keyword/key are mandatory. The
form may appear wherever the existing numeric aggregate expression is accepted,
including a reaction-arrow hazard:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
```

Do not add aliases, omitted-parenthesis forms, global frequencies, unkeyed
frequencies, set-builder syntax, or dotted row variables.

### 2. Exact context and type rules

Elaborate `freq` only with a selected system/table context. The key must resolve
to a declared Ref attribute on that selected system. Use the selected system's
exported table name and the key's exported attribute name exactly as legacy
`countBy`/`sizeBy` do.

The predicate must elaborate to Bool in the selected row context. It may use
row attributes, enum variants, literals, model parameters, arithmetic,
comparisons, and Boolean composition already accepted by a legacy `countBy`
filter. Reject predicates containing:

- `inputSum`/input aggregates;
- `countBy`, `sizeBy`, or any relational aggregate;
- nested `freq`; or
- references outside the selected row/model parameter scope.

The rejection must explain:

```text
frequency predicates are row-local; aggregates join on declared Ref keys only
```

and point at the offending nested expression/token. Do not accept and later
rely on the Rust validator to reject it.

### 3. Exact lowering

For selected table `person`, Ref attribute `employer`, and predicate `p`, emit
the same existing expression tree as:

```lean
countBy employer (p) / sizeBy employer
```

That means an `Expr.div` whose numerator is the current keyed
`Expr.agg AggOp.count ... p` and whose denominator is the current keyed
`Expr.agg AggOp.count ... (Expr.bool true)`, with the exact same table/fk/self-fk
strings and order as the legacy parser emits. Reuse the existing aggregate
helper/path; do not duplicate its join-key construction.

Do not introduce zero-denominator handling, normalization, floating-point
changes, new IR cases, or runtime semantics.

### 4. Positive twins

Add imported/focused tests proving exact structural and JSON equality for:

- `freq (health = I) over employer` versus
  `countBy employer (health = I) / sizeBy employer`;
- a predicate using `∧` and a model parameter;
- a derived system/key name and an explicit system name override;
- `freq` inside an inferred reaction arrow and an explicit-system arrow; and
- at least one complete SIR-shaped model.

Assert the emitted expression tree directly so accidental numerator/denominator
reversal or key-name drift cannot hide behind a large JSON comparison. Widget
hazard pretty-printing must be deterministic; either render the mathematical
`freq` spelling from the source/IR pattern or retain the existing expanded
aggregate spelling consistently. Do not redesign widget visuals.

### 5. Required negative coverage

Add complete files and exact positioned expectations for:

- unknown key;
- key exists but is Real, Int, or enum rather than Ref;
- non-Boolean predicate;
- unknown row attribute in the predicate;
- input aggregate in the predicate;
- relational aggregate in the predicate;
- nested `freq`; and
- omitted key/parentheses forms, which must receive a deliberate teaching
  diagnostic if a recovery syntax rule is needed rather than a generic parser
  failure.

The key diagnostics must name the selected system and offending attribute.
There is no public contextless expression host in the current DSL: transition,
output, and view expressions all select a system before elaboration. Preserve
that invariant with code inspection or an internal helper assertion; do not
invent a top-level expression host or require an unreachable negative file.

## Allowed files

- `frontend/Sembla/DSL.lean`
- shared surface/positive/negative tests
- `frontend/Sembla.lean` if test registration is needed
- `frontend/scripts/test-negative.sh`
- narrowly related widget pretty-expression/data tests

## Non-goals

C(i) `#{q ∈ ...}` comprehensions, dotted row binders, arbitrary joins,
additional aggregate operators, command blocks, canonical migration, IR/JSON,
Rust/runtime, or widget styling.

## Acceptance criteria

1. The frozen `freq` form parses only with mandatory predicate parentheses and
   `over` key, and uses the selected system's declared Ref.
2. Direct expression assertions and full-model twins prove exact equivalence to
   legacy `countBy / sizeBy`, including byte-identical JSON.
3. Row-local/type/Ref restrictions are enforced during Lean elaboration with
   exact positioned diagnostics for every reachable listed negative, and no
   contextless public expression host is introduced.
4. Reaction arrows accept `β · freq (...) over ...` and preserve widget/data
   behavior.
5. Legacy aggregate syntax and all canonical exports remain accepted and
   unchanged.
6. C(i), IR/JSON/Rust/dependencies/docs remain untouched.
7. All required README checks and `git diff --check` pass.

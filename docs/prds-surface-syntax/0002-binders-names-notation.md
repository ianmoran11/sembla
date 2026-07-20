# PRD 0002: Parameter binders, derived names, priors, and mathematical aliases

## Context

Read the folder README first; its constraints bind. PRD 0001 exposed one
semantic kernel while preserving the bracketed `model%` adapter. This PRD
implements option B inside that existing structure: declarations become real
surface bindings, runtime strings derive predictably from identifiers, priors
use `~`, and a deliberately small set of mathematical aliases maps to existing
IR nodes.

## Goal

The list-form kernel accepts the frozen option-B syntax and emits byte-identical
IR to its legacy spellings, with deterministic name derivation, unambiguous
bare-name resolution, exact diagnostics, and no canonical-model migration yet.

## Specification

### 1. Parameter binding and tilde priors

Add these `semblaParam` forms alongside the legacy forms:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
```

The declared identifier and its original token enter model parameter scope.
Derive the IR name using the folder README's identifier algorithm (`β` becomes
`"beta"`, `γ` becomes `"gamma"`). `~ LogNormal a b` emits exactly the current
`Prior.logNormal [a, b]` metadata; a missing suffix emits `none`. Reuse the
existing finite-decimal validation and exact `Scientific` elaboration.

The existing forms remain accepted for compatibility:

```lean
param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25)
param gamma : Real := 0.1
```

Add a positive twin proving both forms produce equal `ParamDecl` lists and
byte-identical JSON. Do not migrate `frontend/Sembla/Models.lean` in this PRD.

### 2. Bare parameter references

In `semblaExpr`, a bare identifier may resolve to either:

- a selected-system attribute (`Expr.selfAttr` / existing enum handling); or
- exactly one declared model parameter (`Expr.param` using the derived runtime
  name).

A bare parameter reference must emit `Expr.param`; it must never substitute the
default. Keep `parameter beta` as a legacy compatibility escape.

If a parameter and selected-system attribute have the same surface identifier,
reject the use at that identifier with exactly:

```text
ambiguous identifier '<name>': both an attribute and parameter are in scope
```

Do not choose by declaration order. Unknown names must fail at the original
identifier. Add focused structural assertions over emitted `Expr` trees.

### 3. Derived system/table names and override

Add list-form system declarations that derive the exported table name:

```lean
system Person (rows := 1000) where [
  state health : {S, I, R}]
system Employer (rows := 50) where []
system LegacyPerson (name := "Person") (rows := 4) where []
```

`(rows := ...)` is mandatory; `(name := ...)` is optional and is the only new
explicit table-name override. Retain `system Person as "person" rows(1000)` as
legacy compatibility syntax.

Implement the folder README's snake-case/Greek derivation as one pure helper
used by both parameter and system parsing. Positive tests must include at least:

- `Person → person`
- `PolicyController → policy_controller`
- `HTTPServer → http_server`
- `Person2D → person2_d`
- preserved identifiers containing internal underscores/digits;
- every Greek code point the helper accepts, including `β`, `γ`, `λ`, `μ`,
  `σ`, `τ`, and `θ`, plus `λ_parent → lambda_parent`;
- explicit override preserving `"Person"` exactly; and
- derived/derived and explicit-override/derived collisions within one box,
  while equal table names in separate boxes remain valid.

Enforce the README's accepted identifier grammar. Reject Lean primes, escaped
punctuation/whitespace identifiers, leading/trailing/repeated separators,
unsupported non-ASCII characters,
and any post-derivation collision. Diagnostics must name the source identifier,
derived name where applicable, and both declarations in collision cases.

### 4. Exact notation aliases

Add only the aliases frozen by the README:

```lean
ℝ
lhs · rhs
lhs ∧ rhs
lhs ≠ rhs
lhs ≤ rhs
```

They lower through the existing type checker to Real, `Expr.mul`, `Expr.and`,
`Expr.ne`, and `Expr.le`. Preserve the corresponding existing precedences and
type diagnostics. In particular, `health ≠ I` must use enum-literal resolution
and emit `Expr.ne (Expr.selfAttr "health") (Expr.enum "I")`; it is not a
generic pair of bare-name lookups. Validate membership of `I` in `health`'s
enum variants and reject incompatible enum/scalar comparisons. The reversed
shorthand `I ≠ health` is outside this PRD.

Positive twins must compare each alias to its legacy/IR equivalent, including
enum inequality. Negative tests must prove that ordered comparisons remain
numeric, `∧` requires Booleans, `·` follows existing numeric compatibility,
and `≠` reports unknown enum variants/incompatible operands at the relevant
token.

Do not add adjacent attractive aliases (`∨`, `¬`, `≥`, `!=`, `<=`) or new
surface types.

### 5. Widget and source-token behavior

A hazard panel built from a bare Greek parameter must still show the derived
runtime name and the same default/prior density as the equivalent legacy model.
Diagnostics and widgets must attach to the original binding/reference tokens,
not generated ASCII names.

## Required positive coverage

Extend PRD 0001's imported surface test module and/or add named files under
`frontend/Positive/` covering:

- prior and priorless bindings;
- bare parameter lowering;
- legacy/new exact JSON twins;
- derived and overridden table names;
- all accepted aliases and derivation mappings;
- forward references using a derived table name; and
- unchanged declaration order.

Register standalone positive files in `frontend/scripts/test-negative.sh` (the
script also runs positive elaboration probes).

## Required negative coverage

Add complete files under `frontend/Negative/` and exact expected diagnostics for:

- unknown bare identifier;
- parameter/attribute ambiguity;
- duplicate parameter runtime name after derivation;
- duplicate table runtime name after derivation;
- unsupported identifier character;
- malformed/unsupported separator pattern;
- non-finite/out-of-range tilde-prior terms;
- non-Boolean `∧`;
- non-numeric `≤`; and
- incompatible `·` operands;
- unknown enum variant in `attribute ≠ variant`; and
- incompatible enum/scalar `≠` operands.

Each failure must point at the relevant new syntax token. Generic parser errors
are insufficient for cases listed above.

## Allowed files

- `frontend/Sembla/DSL.lean`
- PRD 0001's surface test module and `frontend/Sembla.lean` if registration is
  needed
- `frontend/Positive/*.lean`
- `frontend/Negative/*.lean`
- `frontend/scripts/test-negative.sh`
- narrowly related widget data tests (not rendering/style code)

## Non-goals

Reaction arrows, `freq`, command-style blocks, canonical migration, new prior
families, arbitrary Lean terms, IR/JSON changes, or widget redesign.

## Acceptance criteria

1. Every frozen option-B form parses and lowers through the shared PRD 0001
   kernel; legacy spellings still compile.
2. Legacy/new twins have equal `Model` values and exact byte-identical
   `IR.toJson` output.
3. Bare parameters emit `Expr.param` with derived runtime names and retain
   default/prior metadata; ambiguity is rejected rather than shadowed.
4. Name derivation and override behavior matches every binding README example,
   with collision/unsupported-character diagnostics covered exactly.
5. Unicode aliases emit only their specified existing IR nodes and preserve
   existing type rules.
6. Canonical models/fixtures, IR/JSON, Rust, dependencies, and public docs are
   unchanged in this PRD.
7. All required README checks and `git diff --check` pass.

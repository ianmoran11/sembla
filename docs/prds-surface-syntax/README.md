# Mathematical surface-syntax PRDs

Ordered PRD set implementing the **recommended** Lean surface-language work in
[`docs/design/surface-syntax-options.md`](../design/surface-syntax-options.md).
Run it from the Sembla repository with:

```text
/piprd run docs/prds-surface-syntax
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding.

## Authority and scope

- `docs/design/surface-syntax-options.md` is the syntax authority for this run.
- `DESIGN.md` §§4.2 and 5.5 bind the closed relational fragment and the rule
  that accepted syntax must never be inert.
- `DECISIONS.md` §§A1, A5, and G3 bind the Lean/infoview choice, the
  frontend-agnostic IR boundary, and declarative prior metadata.
- Existing canonical JSON, exporter aliases, output ordering, rule ordering,
  widget behavior, and execution hashes are frozen compatibility contracts.
- This track changes the human-facing Lean surface and its elaborator only. It
  must not change `frontend/Sembla/IR.lean`, `frontend/Sembla/Json.lean`, any
  Rust crate, `examples/*.json`, runtime/backend behavior, proof statements, or
  dependency manifests.

The implemented recommendation is **B, A, C(ii), then D**. Option C(i)'s keyed
set comprehensions remain deferred because no checked-in model needs dotted
row-binder scope. Option E's do-builder remains rejected for human authoring.
Do not add either opportunistically.

## Run order

1. `0001-kernel-and-contract.md` — extract one reusable semantic kernel and
   freeze the no-drift test contract.
2. `0002-binders-names-notation.md` — option B: parameter bindings, tilde
   priors, derived names, and exact mathematical aliases.
3. `0003-reaction-arrows.md` — option A: one-line reaction arrows with
   deterministic state/system inference.
4. `0004-keyed-frequency.md` — option C(ii): `freq (...) over ...` lowering to
   the existing keyed count/size expression.
5. `0005-command-frontend.md` — option D: the complete indentation-structured
   `sembla_model` command frontend.
6. `0006-migrate-models-and-docs.md` — migrate all human-facing models and land
   the authority documentation without changing exported bytes.

Later PRDs depend on every earlier PRD. Do not combine, reorder, or begin the
canonical migration before PRD 0005's command twin and diagnostics are green.

## Semantic-kernel boundary

The existing enclosing, multi-pass `model%` elaborator remains accepted as the
low-level compatibility surface and semantic kernel. PRD 0001 must extract a
single collected surface-model representation plus a single validation/IR
emission path. Every new notation and the command frontend must feed that path.
There must be no second IR builder, duplicated type checker, or separate name,
ordering, schema, or widget semantics.

Legacy spellings remain accepted in focused compatibility tests and generated
fixtures. They stop being the public authoring style after PRD 0006. Direct IR
construction remains the machine-writer path.

## Byte identity is the primary acceptance contract

Every sugar must elaborate to byte-identical `Sembla.IR.Model` JSON, including:

- model, table, parameter, port, view, and summary names;
- exact `Scientific` coefficients/exponents;
- parameter, box, table, transition, effect, schema, output, view, wire, and
  summary order;
- transition order, because it determines stable rule IDs; and
- explicit `null`/empty-list structure already pinned by canonical JSON.

`diff-ir` is supplemental. It may not replace literal string equality in Lean
sugar twins or shell `cmp` in `frontend/scripts/check-parity.sh`. Each syntax
PRD must land a legacy-versus-new twin before PRD 0006 migrates canonical
models.

## Frozen option-B syntax and resolution

Within the existing list-form kernel and later command blocks, accept:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
```

A declared parameter is referenced by its bare identifier (`β`, not
`parameter beta`). Defaults remain metadata and must not be substituted into
hazards. The legacy `Real`, `prior LogNormal(...)`, `parameter beta`, and
`system Person as "person" ...` forms remain compatibility spellings.

Add exactly these notation aliases at existing corresponding precedences:

- `ℝ` → the existing Real surface type;
- `·` → `Expr.mul`;
- `∧` → `Expr.and`;
- `≠` → `Expr.ne`; and
- `≤` → `Expr.le`.

Do not add `∨`, `¬`, `≥`, ASCII `!=`/`<=`, new prior families, coercions, or
term-level Lean expressions in this track. Enum inequality is explicitly valid
in attribute-first form: `health ≠ I` lowers to
`Expr.ne (Expr.selfAttr "health") (Expr.enum "I")`, with the same variant
membership and compatible-type checks as `health = I`. The reversed shorthand
`I ≠ health` is not added.

Bare identifiers resolve against the selected system's attributes and the
model's parameters. If exactly one declaration matches, use it. If both an
attribute and parameter match, reject at the identifier with
`ambiguous identifier '<name>': both an attribute and parameter are in scope`.
Unknown names retain a positioned unknown/undeclared diagnostic. Never resolve
ambiguity by declaration order or silent shadowing.

## Frozen runtime-name derivation

Parameters and systems derive runtime names from Lean identifiers unless an
allowed override is present.

- ASCII identifiers use conventional snake case: boundaries occur before an
  uppercase letter following a lowercase letter/digit and between an acronym
  and a following capitalized word. Examples: `Person → person`,
  `PolicyController → policy_controller`, `HTTPServer → http_server`,
  `Person2D → person2_d`.
- The accepted source grammar for derivation is: an ASCII letter or one
  documented Greek code point first, followed by ASCII letters/digits,
  documented Greek code points, or single underscores. Lean primes, escaped
  identifiers containing punctuation/whitespace, leading digits/underscores,
  and repeated/trailing underscores are rejected. A system with an otherwise
  unsupported identifier may use the explicit string override; parameters have
  no override in this track.
- Existing internal underscores and digits are preserved.
- Greek components transliterate to lowercase English names through an
  explicit code-point table, not locale or Unicode-name heuristics. The
  required canonical cases are `β → beta`, `γ → gamma`, `λ → lambda`,
  `μ → mu`, `σ → sigma`, `τ → tau`, and `θ → theta`. Additional Greek letters
  may be accepted only when every mapping is documented and tested; all other
  non-ASCII characters are diagnosed at the identifier.
- Mixed identifiers are deterministic; the migration-critical case
  `λ_parent → lambda_parent` is required.
- Collisions after derivation are errors naming both source identifiers and the
  derived runtime name. Parameter collisions are model-global. System/table
  collisions (including explicit-override versus derived collisions) are
  checked within each box, matching the IR namespace; equal table names in
  distinct boxes remain valid.
- A system may use `(name := "...")` as its sole explicit table-name override.
  The override is required wherever derivation would change frozen bytes,
  notably the `observations` fixture's table name `"Person"`.
- Command models may use `(name := "IR model name")`; canonical declarations
  use it whenever the Lean constant name does not derive to the frozen model
  name.

The implementation may choose the internal helper names, but the derivation
algorithm and examples above are public behavior and require positive and
negative tests.

## Frozen reaction-arrow syntax

Support these forms inside the list kernel and command frontend:

```lean
infect : S →[β] I
infect on Person : S →[β] I
infect : health: S →[β] I
infect on Person : health: S →[β] I
```

An arrow lowers to exactly one enum-equality guard, the supplied Real hazard,
and exactly one assignment from source to destination on the same enum
attribute. It cannot encode additional guards/effects. The existing general
transition form remains required for those cases.

System and attribute restrictions are independent. `on Person` selects the
system; without it, infer the unique compatible system in the enclosing box.
When no `health:` label is present, the selected/inferred system must have
exactly one enum/state attribute, matching the design proposal; do not infer
among multiple enum columns merely because only one contains the named
variants. `health:` explicitly selects the state attribute, and without `on`
the system must then be uniquely inferable. Source and destination must be
variants of that same attribute. Zero or multiple systems, or multiple state
columns without an explicit label, are positioned errors that prescribe the
specific missing restriction. Inference must not depend on collection order.
Self-loops are valid.

## Frozen keyed-frequency syntax

Support exactly:

```lean
freq (health = I) over employer
```

Parentheses and `over` are mandatory. In a selected system context, `employer`
must be a declared Ref attribute. Lower to the exact tree already emitted by:

```lean
countBy employer (health = I) / sizeBy employer
```

The predicate must be Boolean and row-local. It may use the same scalar
fragment accepted by a legacy `countBy` filter, but may not contain input
aggregates, relational aggregates, or nested `freq`. Errors must explain that
frequency predicates are row-local and joins use declared keys only. Denominator
and runtime arithmetic semantics remain unchanged.

## Frozen command frontend

The primary declaration form after PRD 0006 is:

```lean
sembla_model SirWorkplace
    (name := "sir_workplace_frequency_dependent")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param γ : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.25

  box sir where
    system Person (name := "person") (rows := 1_000_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (name := "employer") (rows := 50_000)

    infect on Person : health: S →[β · freq (health = I) over employer] I
    recover on Person : health: I →[γ] R

    view S := count Person where health = S
    view I := count Person where health = I
    view R := count Person where health = R

  summary peak_I := max sir.I
  summary peak_tick := argmaxₜ sir.I
```

The header's `(name := ...)` is optional; `(dt := ...)` is mandatory. Command
blocks contain no list brackets, separator commas, or mandatory empty
categories. Pin the remaining declaration forms as follows:

```lean
input restriction_modifier where
  modifier_offset : ℝ

transition restrict on Controller where
  guard mode = Open ∧ inputSum infection_count field infected > 500
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4

output infection_count from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)

view active := count Person where health = I
view risk_total := sum Person using risk
view active_risk_max := max Person where health = I using risk

wire population infection_count -> policy infection_count
summary final_I := last population.I
```

System and input-schema attributes support the complete current attribute
family: enum `{...}`, `ℝ`, `Int`, and collected-system Ref identifiers.
`inputSum` remains restricted to numeric input fields; accepting enum/Ref schema
fields does not add new expression operations. Output builders remain the
current count/sum numeric results. An empty system omits `where`. General
transitions use one `set` line per effect. Views support count and `sum|min|max`, optional
`where`, and mandatory `using` for non-count reductions. Summaries support
`sum|min|max|last|argmaxₜ` over `box.view`. Wires retain ASCII `->` because they
name dataflow endpoints rather than reactions.

Declarations may be interleaved where scope permits, but relative textual order
within each emitted IR list is stable. Collection remains multi-pass, so
forward references work. Every declaration retains its original syntax token
for diagnostics and infoview widget anchors.

## Diagnostics and tests

No accepted construct may be ignored or partially elaborated. Every feature PRD
must add:

- positive legacy/new twins under `frontend/Positive/` or a dedicated imported
  test module;
- complete failing files under `frontend/Negative/`;
- exact `file:line:column: error: ...` expectations in
  `frontend/scripts/test-negative.sh`; and
- tests that prove diagnostics point at the new source token, not at a generated
  kernel term.

Extend the shell harness with a helper for new syntax failures that extracts all
`file:line:column: error: ...` lines and compares the complete set exactly. A
required expected line plus unexpected additional errors must fail; the current
`grep -Fx`-only behavior is insufficient for parser-heavy command negatives.
Legacy checks may keep their existing helper if changing them would create
unrelated churn.

Generic parser errors are not substitutes for required unknown-name,
ambiguity, type, scope, duplicate, schema, or inference diagnostics.

## Required checks for every PRD

From the repository root, each implementation and review must run:

```bash
cd frontend && lake build
cd frontend && bash scripts/test-negative.sh
bash frontend/scripts/check-parity.sh
./scripts/check.sh
```

Also run `git diff --check`. Preserve widget tests, exact-scientific tests,
proof hygiene, exporter aliases, fixed-seed output/hash parity, and all Rust
tests transitively through these checks.

## Global non-goals

- IR/JSON/schema changes or Rust/runtime/backend work.
- Set comprehensions (`#{...}`), dotted row binders, arbitrary predicates, or
  generalized joins.
- Option E, a do-notation builder, generated-model APIs, or a pretty-printer.
- New priors, types, expression semantics, runtime flags, or dependencies.
- Widget visual redesign; only source anchors needed by the new syntax may
  change.
- Editing external Obsidian/Vault copies during the managed run.

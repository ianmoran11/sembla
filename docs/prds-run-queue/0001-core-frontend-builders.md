# PRD 0007: Add pure parameter, table and model builders

## Dependencies

PRDs 0001–0006 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Semantic construction currently occurs inside macros. This first builder slice
must move core declaration construction into pure APIs without changing the
current macro surface. The pure API is intentionally broader than the syntax
adapter: it covers the complete declaration fragment accepted by PRD 0005,
while current macros continue to expose only their existing subset until PRD
0009 delegates to these builders.

PRD 0005 already defines the authoritative declaration predicates and checker.
PRD 0006 proves whole-model checking and exact reconstructive erasure. This PRD
must consume those contracts rather than duplicate declaration rules or invent a
second schema representation.

## Goal

Provide pure, source-order-preserving builders for the complete PRD 0005 core
declaration fragment—priors, parameters, attributes, tables, declaration-only
box shells and model shells—with exact success/failure correspondence and
proofs that complete shells are accepted by PRDs 0005 and 0006.

## Owned fragment

The pure builders must cover all of the following accepted raw forms, even when
the current macro syntax exposes only a subset:

1. `IR.Prior` values for Uniform, Normal and LogNormal families. Every family
   has exactly two exact scientific arguments; Uniform additionally requires
   strict ordered bounds. No argument is evaluated or normalized.
2. Real parameters with exact Real defaults and an optional valid prior, and Int
   parameters with exact Int defaults and no prior.
3. Real, Int, nonempty duplicate-free Enum and table-reference attribute types,
   plus named attributes in exact source order.
4. Tables with exact names, `sizeHint`s and source-ordered attributes. Table and
   attribute names are unique in their PRD 0005 namespaces.
5. Declaration-only box shells containing a name and source-ordered tables.
   Their transitions, inputs, outputs, ordinary views and grouped views are
   empty because those constructors belong to PRDs 0008–0009.
6. Declaration-only model shells containing an exact name, exact positive `dt`,
   source-ordered parameters and source-ordered box shells. Their wires and
   summaries are empty. Empty models, zero-table boxes, empty tables and
   zero-sized tables remain valid where PRD 0005 permits them.

Attribute references resolve against the complete ordered table-name catalog of
the enclosing box. Builders must therefore accept forward, backward, self and
mutual references that PRD 0005 accepts; they may not reject references merely
because a target table appears later in source order.

## Required builder architecture

1. Define pure structured builder errors independent of `Lean.Syntax` and macro
   diagnostics. Errors must carry a stable category and source-index path
   sufficient to identify model metadata, parameter/prior, box, table,
   attribute and enum-variant positions.
2. Error categories must distinguish every owned rejection class: nonpositive
   `dt`, duplicate parameter/box/table/attribute names, parameter default/type
   mismatch, an Int prior, invalid prior arity, unordered Uniform bounds, empty
   enums, duplicate enum variants and unresolved table references. Exact prose
   and precedence among simultaneous independent defects are non-normative.
3. Expose public compositional specification/result types equivalent to a
   `CoreBoxShell` and `CoreModelShell`. PRDs 0008–0009 must be able to consume
   their ordered names, parameters, tables and attributes without editing
   `Core.lean`, reparsing syntax or reconstructing a parallel name map.
4. Use the accepted PRD 0005 predicates directly, including
   `PriorWellFormed`, `ParameterWellFormed`, `SchemaWellFormed`,
   `BoxDeclarationsWellFormed` and `DeclarationsWellFormed`. Do not copy their
   logic into a differently named validity hierarchy.
5. Contextual box construction must collect the complete table catalog before
   validating attribute references. It must preserve every accepted name,
   scientific encoding, prior family/argument, enum variant, size and list
   position exactly.
6. Builders may return proof-carrying raw fragments or checked fragments. For a
   raw result, expose its accepted PRD 0005 predicate; for a checked result,
   erasure must equal the intended raw fragment exactly. No successful path may
   normalize, sort, deduplicate or silently repair input.
7. Complete model-shell construction must establish both declaration-checker
   and whole-model-checker acceptance. The PRD 0006 result must erase exactly to
   the constructed shell, whose later-owned lists remain empty.
8. Do not refactor macros in this PRD. PRD 0009 performs coordinated delegation
   after the core, transition and observation builder APIs exist. Supporting all
   PRD 0005 prior families in the pure API does not add new macro syntax.

## Required theorem matrix

Named public theorem/lemma declarations must include statements equivalent to:

| Obligation | Required statement |
| --- | --- |
| Prior soundness | Successful prior construction yields `PriorWellFormed` and retains the exact family and two scientific arguments. |
| Parameter soundness | Successful parameter construction yields `ParameterWellFormed` and retains its exact name, type, default and optional prior. |
| Schema/table soundness | Successful contextual table construction yields `SchemaWellFormed` against the complete box table catalog and preserves ordered attributes. |
| Box-shell soundness | Successful box construction yields `BoxDeclarationsWellFormed`; only the owned name/tables are populated. |
| Model-shell soundness | Successful model construction yields `DeclarationsWellFormed`; only owned metadata, parameters and box/table declarations are populated. |
| Success completeness | Every input satisfying the documented corresponding PRD 0005 predicate is reproduced successfully without changing its raw structure. |
| Failure characterization | Builder failure is equivalent to failure of the corresponding documented validity predicate; no unique category is required for simultaneous defects. |
| Declaration acceptance | A successful model shell has some `ctx` with `checkDeclarations raw = .ok ctx`. |
| Model acceptance and erasure | A successful model shell has some checked model with `checkModel raw = .ok checked` and `checked.erase = raw`. |

The builder-domain predicates used for completeness and failure may package the
existing PRD 0005 predicates with the explicit shell shape above, but they may
not be aliases for builder success or successful-checker existence.

All named theorems enter the existing automated axiom and opaque-proposition
audit. Computed fixtures do not replace these statements.

## Required fixture matrix

Direct builder fixtures must cover:

- Real parameters without priors and with Uniform, Normal and LogNormal priors;
- Int parameters without priors;
- exact, deliberately non-normalized scientific encodings in defaults, prior
  arguments and positive `dt`;
- Real, Int, Enum and Ref attributes, exact enum order, source-ordered schemas,
  table sizes and multiple parameters/tables/boxes;
- forward, backward, self and mutual table references;
- empty models, zero-table boxes, empty tables and zero-sized tables;
- exact raw equality with representative current canonical frontend
  declarations, while exercising pure APIs directly rather than macros; and
- declaration-checker acceptance, whole-model-checker acceptance and exact
  checked erasure for a complete positive shell.

Single-defect rejected fixtures must cover every structured builder error
category above, including zero and negative `dt`, each duplicate namespace,
default/type mismatch, Int prior, wrong prior arity, unordered Uniform bounds,
empty/duplicate Enum declarations and unresolved references.

Update `docs/design/lean-ir-coverage.md` with a literal builder/category fixture
table linking every owned positive form and rejection category to executable
evidence.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Core.lean`
- `frontend/Sembla/Frontend/Builders/CoreTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Changing the accepted PRD 0005/0006 predicates, checkers, diagnostics,
  schemas, checked structures or erasers. If their public theorems prove
  insufficient, stop and amend this PRD before editing an earlier module.
- Adding or changing macro syntax, including exposing Uniform or Normal priors
  through macros. PRD 0009 owns macro delegation while preserving current
  syntax.
- Transition, effect, contest, input/output, ordinary/grouped-view or summary
  builders.
- Wires, composition sources or composition checking.
- Evaluation, normalization, sampling, `Float`/`f64` conversion or downstream
  runtime behavior.

## Test and proof guidance

Use exact integer/rational reasoning only. Compare successful results and
checked erasures with raw fixture values, not pretty-printed forms. Validate
references only after constructing the complete enclosing table-name catalog.
Use one-defect fixtures for stable category/path assertions and do not make
general error precedence a theorem.

Run at least:

```bash
cd frontend && lake build Sembla.Frontend.Builders.CoreTests
bash frontend/scripts/check-proofs.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
bash scripts/check.sh
git diff --check
```

Acceptance must cite the focused fixture table and automated axiom inventory.
No `sorry`, `admit`, `axiom`, `native_decide`, `unsafe`, `implemented_by` or
opaque semantic proposition is permitted.

## Acceptance criteria

1. Pure builders decide exactly the complete PRD 0005 core declaration fragment
   documented above, including all prior families and complete-catalog table
   reference resolution.
2. Public shell/result APIs preserve exact source order and are sufficient for
   PRDs 0008–0009 to consume without editing `Core.lean` or creating parallel
   schemas/name maps.
3. Builder soundness, completeness, failure, declaration acceptance,
   whole-model acceptance and exact-erasure theorem families pass the automated
   audit without circular definitions.
4. Every required positive and single-defect fixture passes and is traced in
   `docs/design/lean-ir-coverage.md`.
5. Existing macro syntax, positioned diagnostics, canonical model/export bytes
   and runtime behavior remain unchanged.
6. Focused build, proof hygiene, movable-path allowlist, full repository checks
   and `git diff --check` pass within the allowed file list.

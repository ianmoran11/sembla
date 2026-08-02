# PRD 0005: Check declarations, parameters and references

## Dependencies

PRDs 0001–0004 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

Raw models must first establish exact model metadata, global/local namespaces and
finite schemas before term checking can resolve expressions. PRD 0005 builds the
reusable declaration context consumed by PRD 0006; it does not check deferred
term, observation or output-builder payloads.

## Goal

Implement and prove the declaration/reference fragment of raw checking, with an
independent structural judgment and exact projection erasure.

## Requirements

1. Define an inductive `CheckErrorCategory`, a structured declaration path whose
   segments identify fields and source-list indices, and a `CheckError` value
   containing both. Diagnostic prose and general precedence between simultaneous
   defects are not theorem targets.
2. Preserve the single `IR.Model.name` exactly. Enforce these explicit namespace
   boundaries:
   - model-global, independently: parameters, boxes and summaries;
   - per box, independently: tables, transitions, input ports and output ports;
   - input and output port namespaces are separate, so the same spelling in one
     input and one output is accepted;
   - ordinary and grouped views share one per-box view namespace;
   - attributes are unique within each table, input schema and output schema;
   - enum variants are nonempty and duplicate-free within their owning enum.
3. Check exact mathematical `0 < dt`, parameter default/type agreement, integer-
   prior exclusion, and exactly two exact arguments for every current
   `IR.PriorFamily`. Require strict exact `lo < hi` for Uniform bounds. Priors
   remain raw-preserved structural metadata with no sampling denotation; do not
   add Normal/LogNormal distributional constraints.
4. Resolve declarations in phases:
   - establish all box/table headers before resolving attributes, so forward and
     mutual box-local table references are accepted;
   - construct the accepted `ModelSchema` and owner-indexed table schemas;
   - resolve input/output port schemas as box-owned schemas without assuming that
     a box has a table, and make them instantiable for any later current-table
     scope required by `InputSignature`;
   - resolve transition table targets;
   - retain shallow, source-ordered headers for views/grouped views, summaries and
     later-owned output payloads without checking their expressions, builders or
     target semantics.
5. Define a canonical `DeclarationContext` containing the exact source model
   name, the exact source `dt` with positivity evidence, the accepted
   `ModelSchema`, and the shallow catalogs/resolved declaration references needed
   by PRD 0006. Define an explicit raw `DeclarationProjection` and functions
   `projectDeclarations` and `eraseDeclarations`.
6. Define `DeclarationsWellFormed` structurally, independently of
   `checkDeclarations`; it must not be an alias for checker success or quantify
   over the executable checker's result.
7. Implement a terminating canonical checker
   `checkDeclarations : IR.Model → Except CheckError DeclarationContext`.
8. Preserve exact declaration projection under erasure: source spellings,
   scientific encodings, list order, sizes, prior payloads, enum order and raw
   reference-target spellings must not be normalized. Whole-model erasure remains
   PRD 0006.
9. Prove named checker soundness, completeness, failure characterization, exact
   erasure, exact scientific comparison and owner-indexed lookup/resolution
   theorems described below.
10. Add positive and single-defect negative fixtures for every owned declaration
    category and path family.

## Allowed files

- `frontend/Sembla/Semantics/CheckDeclarations.lean`
- `frontend/Sembla/Semantics/CheckDeclarationsTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Expression, effect, claim or observation checking.
- Output-builder, view/grouped-view body, summary-target or wire checking.
- Changing accepted prior syntax or defining probability distributions.
- Composition-source namespaces.
- Editing `Types.lean` or `Syntax.lean`; if an accepted prerequisite is
  insufficient, stop and amend this PRD's allowed-file scope.

## Required theorem matrix

The named theorem family must include statements equivalent to:

| Obligation | Required statement |
| --- | --- |
| Checker soundness | `checkDeclarations raw = .ok ctx` implies `DeclarationsWellFormed raw` and `eraseDeclarations ctx = projectDeclarations raw`. |
| Checker completeness | `DeclarationsWellFormed raw` implies some `ctx` with `checkDeclarations raw = .ok ctx`. |
| Failure characterization | Checker failure is equivalent to failure of the independent structural judgment; no unique category is required when several defects coexist. |
| Exact `dt` comparison | The executable scientific sign test is equivalent to `0 < scientificDenote dt`. |
| Exact Uniform order | The executable bound comparison is equivalent to strict denotation order. |
| Resolution fidelity | Successful parameter, box, table, attribute, enum-variant and transition-table lookups agree with their owner-indexed identifiers and raw spellings. |
| Projection erasure | Accepted declarations erase exactly to `projectDeclarations raw`, preserving every owned field and source order. |

Executable scientific comparisons must use exact sign/exponent/integer
arithmetic or an equivalent proved-exact procedure. They must not pass through
`Float`, `f64` or approximate canonicalization. Computed fixtures supplement,
but do not replace, the named theorem family.

## Required fixture matrix

Positive fixtures must cover every prior family, priorless real and integer
parameters, zero-sized tables, same-named input/output ports, distinct
ordinary/grouped views, empty boxes, and forward plus mutual box-local table
references.

Single-defect negative fixtures must discriminate category and structured path
for at least:

- zero and negative `dt`;
- every duplicate-name namespace listed above, including an ordinary/grouped
  view collision;
- parameter default/type mismatch, integer prior and prior arity;
- equal and reversed Uniform bounds;
- empty and duplicate enum variants;
- unresolved table references in table, input and output schemas; and
- unresolved transition table targets.

Update `docs/design/lean-ir-coverage.md` with one link for every owned invariant
and fixture family. Keep later-owned output/view/grouped-view/summary semantics
assigned to their later PRDs; this increment owns only their stated shallow
catalog obligations.

## Test and proof guidance

Negative fixtures assert structured categories and paths, not prose. Use
single-defect fixtures where a particular category/path is expected; general
error precedence remains intentionally non-normative.

Run at least:

- `cd frontend && lake build Sembla.Semantics.CheckDeclarationsTests`
- `bash frontend/scripts/check-proofs.sh`
- `python3 scripts/check-prd-allowlist.py <this PRD at its current path>`
- `bash scripts/check.sh`
- `git diff --check`

## Acceptance criteria

1. `DeclarationContext`, `DeclarationProjection`, their exact eraser, and all
   explicitly owned namespace/resolution boundaries are implemented without
   changing accepted earlier schema or syntax APIs.
2. The canonical `Except` checker decides exactly the independent structural
   `DeclarationsWellFormed` judgment.
3. Every theorem in the required matrix passes the automated axiom audit without
   a weakened or circular statement.
4. Every positive and negative family in the fixture matrix passes with complete
   coverage links and structured category/path assertions.
5. Exact source encodings and declaration order are preserved; no floating-point
   comparison, normalization or whole-model checking is introduced.
6. Focused build, proof hygiene, movable-path PRD allowlist, full repository
   checks and `git diff --check` pass within the allowed module map.

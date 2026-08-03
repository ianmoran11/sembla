# PRD 0008: Add pure transition, effect and contest builders

## Dependencies

PRDs 0001–0007 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

PRD 0007 exposes the authoritative declaration-only `CoreBoxShell` and
`CoreModelShell` APIs. Their raw projections deliberately leave transitions,
ports, observations, wires and summaries empty. This PRD must extend those
shells compositionally; it may not edit `Core.lean`, repeat its declarations or
construct a second parameter/box/table catalog.

PRD 0006 already provides the authoritative `DeclarationContext`, `TermContext`,
expression/transition checkers and whole-model checker. Transition construction
must remain pure and use those contracts so PRD 0009 can reuse the same term
surface when inputs and observations are attached.

## Goal

Provide pure, source-order-preserving builders for the complete PRD 0006-valid
transition fragment—typed/raw expressions, transitions, set-attribute effects
and race/key resource claims—and a public transition overlay on PRD 0007 core
shells, with exact checker acceptance and reconstructive erasure.

## Owned fragment

The pure API must cover every raw expression and aggregate form accepted by PRD
0006 in transition guard, hazard, effect-value, resource and ordering-key scopes,
including literals, parameters, current attributes, enum variants, arithmetic,
comparisons, Boolean operations, input aggregates and relational aggregates.
Current macro syntax may remain narrower until PRD 0009.

It must also cover:

1. transitions with an exact name, target table, Bool guard, Real hazard,
   source-ordered effects and source-ordered resource claims;
2. set-attribute effects with an exact destination and checked value;
3. race-time claims and raw key-order claims for every orderable domain accepted
   by PRD 0006: Real, Int and owner-indexed Enum;
4. empty and nonempty effect/claim lists, multiple effects and multiple claims;
5. the PRD 0006 duplicate-resource and Ref-write/claim coupling rules; and
6. source-ordered transition lists attached to each existing core box.

The pure raw/checker path must retain checker-valid key ordering. A distinct
surface-producing entry point must preserve the current DSL restriction to
race-time contests; it must not relabel key ordering as current surface syntax.

## Required integration architecture

1. `Transition.lean` must import and consume
   `Sembla.Frontend.Builders.Core`. `CoreModelShell` and its contained
   `CoreBoxShell`s remain the sole owners of the model name, `dt`, parameters,
   ordered boxes, tables, attributes and their ordered name projections.
2. Expose a public compositional transition specification/result. It must retain
   one accepted core shell and attach exactly one source-ordered transition list
   to each core box by that box's existing source ordinal. Structural alignment
   must be represented by construction, a dependent index, or an explicit
   proved invariant; silent `List.zip` truncation is forbidden.
3. The transition projection to raw IR must preserve every core field exactly,
   replace only each box's empty transition list with its attached list, and
   keep inputs, outputs, ordinary views, grouped views, wires and summaries
   empty in this slice.
4. It must not repeat box/table/parameter names in retained parallel state or
   build a replacement lookup catalog. Name resolution must use the accepted
   declaration context and `ctx.modelSchema.catalog`.
5. Expression/effect/claim/transition functions must accept the authoritative
   `DeclarationContext`/`TermContext`, or a thin package containing those exact
   values. They must not be specialized to `CoreModelShell.toRaw`'s empty input
   signature: PRD 0009 must be able to reuse them for input-bearing models.
6. A core preparation adapter may sequence `buildModelShell` and
   `checkDeclarations`, but it must preserve `CoreBuilderError` exactly and may
   not reproduce `modelIssue` or the PRD 0005 declaration rules.
7. Delegate expression synthesis/expected-sort checking to `synthExpr` and
   `checkExpr`, complete transition checking to `checkTransitionTerms`, and
   assembled-model checking to `checkModel`. Use the accepted public soundness
   and erasure theorems rather than defining a parallel typing relation.
8. Individual effect/claim builders may consume intrinsically typed inputs and
   prove their erasure equations. Independent raw `checkEffect`/`checkClaim`
   correspondence theorems are not required where PRD 0006 exposes soundness
   only through the complete transition checker.
9. No successful path may normalize, sort, deduplicate, repair or otherwise
   alter raw expressions, effects, claims or transition order.

## Structured failure contract

Define syntax-independent transition-builder failures with stable source-index
paths covering box, transition, target table, guard, hazard, effect,
destination, effect value, claim, resource and ordering key.

The public failure surface must preserve rather than reinterpret:

- a nested `CoreBuilderError` from core preparation;
- exact PRD 0005 declaration failures such as duplicate transition names and
  unresolved transition target tables;
- exact PRD 0006 `TermCheckError`/`ModelCheckError` categories and paths; and
- a frontend-only unsupported-surface-key-ordering category if a general claim
  request is passed through the race-only surface entry point.

Direct rejection coverage must enumerate every reachable transition term
category: unknown parameter, attribute, enum variant, input, table or join
attribute; nested input aggregate; enum-owner inference failure; expected Bool,
Real, numeric, reference or orderable values; sort mismatch; incompatible
equality or join targets; duplicate resource claims; and an unclaimed Ref
write. Exact prose and precedence among simultaneous independent defects remain
non-normative.

## Required theorem matrix

Named public theorem/lemma declarations must include statements equivalent to:

| Obligation | Required statement |
| --- | --- |
| Raw constructor fidelity | Each public expression/effect/claim/transition constructor erases or projects to the exact supplied raw fields. |
| Expression soundness | Successful synthesis or expected-sort checking yields the PRD 0004 typed term at the reported sort and erases exactly to the raw expression. |
| Transition soundness | Successful complete transition construction yields PRD 0006 well-typed guard, hazard, effects and claims in exact source order. |
| Core preservation | The overlay raw model retains the exact core name, `dt`, parameters, boxes, tables and attributes. |
| Ordinal attachment | Each transition list appears in the raw box with the same core source ordinal; no box or transition list is dropped or reassigned. |
| Slice boundary | Inputs, outputs, ordinary/grouped views, wires and summaries remain empty. |
| Model acceptance and erasure | A successful transition overlay has some `checked` with `checkModel raw = .ok checked` and `checked.erase = raw`. |
| Success completeness | Every documented core-plus-transition candidate satisfying the authoritative PRD 0005/0006 predicates is reproduced successfully without structural change. |
| Failure characterization | Builder failure corresponds to failure of core preparation, declaration checking, term checking or the documented surface-only restriction; it is not defined circularly as failure of the builder itself. |

`buildModelShell_model_acceptance_and_erasure` is only the declaration-only
bridge and must not be presented as the acceptance theorem for a transition
model.

All named theorems enter the automated axiom and opaque-proposition audit.
Computed fixtures do not replace these statements.

## Required fixture matrix

Direct builder fixtures must cover:

- every raw expression/aggregate constructor accepted in transition scopes,
  including exact non-normalized scientific encodings, enum anchoring, mixed
  numeric coercion, input aggregates and relational aggregates;
- Bool guards and exact Real hazards, including a negative literal retained for
  the later dynamic error contract;
- empty and multiple ordered effects and claims;
- race claims and Real, Int and Enum raw key claims;
- a matching claim for a Ref write;
- at least two boxes with different transition counts, including an empty list,
  proving ordinal attachment without truncation or reassignment;
- duplicate transition names, unresolved targets and every term rejection
  category listed above;
- exact core-error propagation without duplicating PRD 0007's full negative
  matrix;
- declaration/term/model checker acceptance and exact checked erasure; and
- exact raw parity with a representative current lowered frontend transition,
  including a multiple-claim contest.

Update `docs/design/lean-ir-coverage.md` with a literal fixture/category table
linking every owned positive form and rejection category to executable evidence.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Transition.lean`
- `frontend/Sembla/Frontend/Builders/TransitionTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Editing the accepted PRD 0007 core-builder module or any accepted semantic
  module. If a public theorem is insufficient, stop and amend this PRD rather
  than weakening encapsulation.
- Replacing the accepted declaration/term contexts, schemas, catalogs,
  predicates, checkers or erasers.
- Folding a current macro-only restriction into raw checker validity.
- Refactoring macros; PRD 0009 owns coordinated macro delegation.
- Input/output, ordinary/grouped-view or summary construction.
- Transition execution, contest winners, effect commit semantics or key-ordering
  surface syntax.
- Composition sources, wire checking or delivery semantics.

## Test and proof guidance

Use checker theorems from PRD 0006 rather than reproving typing independently.
Compare exact raw values and checked erasures, not pretty-printed forms. Use
one-defect fixtures for stable category/path assertions and do not make general
error precedence a theorem.

Run at least:

```bash
(cd frontend && lake build Sembla.Frontend.Builders.TransitionTests)
bash frontend/scripts/check-proofs.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
bash scripts/check.sh
git diff --check
```

No `sorry`, `admit`, `axiom`, `native_decide`, `unsafe`, `implemented_by` or
opaque semantic proposition is permitted.

## Acceptance criteria

1. Pure builders cover the complete PRD 0006-valid transition fragment, while a
   separate surface entry point preserves the current race-time-only syntax.
2. The public transition result consumes PRD 0007 shells without editing the
   accepted core-builder module or creating parallel schemas/name catalogs, and
   preserves exact source-ordinal attachment.
3. Soundness, completeness, failure, whole-model acceptance and exact-erasure
   theorem families pass the automated audit.
4. Every required positive and single-defect fixture passes and is traced in
   `docs/design/lean-ir-coverage.md`, including multi-claim transitions without
   choosing later winner semantics.
5. Existing macros, positioned diagnostics, canonical model/export bytes and
   runtime behavior remain unchanged.
6. Focused build, proof hygiene, movable-path allowlist, full repository checks
   and `git diff --check` pass within the allowed file list.

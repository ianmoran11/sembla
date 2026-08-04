# PRD 0009: Add observation builders and delegate thin macros

## Dependencies

PRDs 0001–0008 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

PRD 0007 provides declaration-only core shells and PRD 0008 provides
`TransitionOverlaySpec` plus context-parametric adapters such as
`buildSynthExpr`, `buildExpectedExpr`, `buildEffect`, `buildClaim` and
`buildTransition`. `DeclarationContext` and `TermContext` remain PRD 0005/0006
APIs. The intermediate core/transition raw projections intentionally leave
inputs and observations empty. The real frontend path, however, remains
macro-driven and currently performs semantic resolution, list assembly and raw
`IR.Box`/`IR.Model` construction itself.

This final builder slice must cover model-local inputs and observations, own one
pure final model assembly boundary, and make current macros parsing/diagnostic
adapters. The accepted PRD 0005/0006 contexts and checkers remain authoritative.

## Goal

Complete the pure model-local frontend for inputs, outputs, scalar views,
grouped views and summaries; provide one exact final model assembly API over the
PRD 0007–0008 results; and delegate all current model-local macro semantic
construction to these proved pure functions without changing syntax,
diagnostics or canonical exports.

## Owned fragment

The pure API must cover every PRD 0006-valid raw form in these families, even
where current syntax remains narrower:

1. input port declarations and their exact source-ordered schemas;
2. output declarations, output schemas, fields and field operations/filters;
3. ordinary scalar views and reducers;
4. grouped views, filters, grouping keys and optional numeric bands; and
5. model summaries and their resolved box/view targets.

The final model API must embed exactly one PRD 0008 `TransitionOverlaySpec`,
which is the sole owner of its `CoreModelShell` and source-ordered transition
lists, and add:

- source-ordered inputs, outputs, ordinary views and grouped views indexed by
  `Fin overlay.core.boxes.length`;
- source-ordered summaries; and
- an exact opaque raw wire list.

Wires are preserved for PRD 0006 reconstructive erasure but are not validated or
assigned delivery semantics here.

## Required builder and assembly architecture

1. `Observation.lean` must consume the public PRD 0007 core-shell and PRD 0008
   `TransitionOverlaySpec`/builder APIs. The complete specification embeds one
   `TransitionOverlaySpec`; it may not edit accepted modules, repeat their
   declarations, create parallel parameter/box/table catalogs, or separately
   re-own core or transition lists.
2. Expose public pure specifications/results for observation fragments and for
   the complete model. Per-box observation payloads must be indexed by
   `Fin overlay.core.boxes.length`, retaining the existing core box source
   ordinals by construction; silent truncation or name-based reassociation is
   forbidden.
3. Expose one pure final assembly function. It must produce the complete raw
   `IR.Model` rather than leaving `IR.Box.mk`, `IR.Model.mk` or semantic list
   assembly to macro code.
4. Final assembly must preserve every supplied raw field exactly and in its
   documented order. It may not normalize, sort, deduplicate or repair a
   checker-valid candidate.
5. The builder layer must assemble the complete raw candidate, including all
   inputs, before obtaining and reusing the authoritative
   `DeclarationContext`/`TermContext`. It must then invoke the PRD 0008
   context-parametric adapters for transition terms and the final public
   `checkModel` boundary. It is forbidden to derive the working context from
   `TransitionOverlaySpec.toRaw`, whose input and observation lists are empty,
   or to require `buildTransitionOverlay` success before checking an
   input-bearing complete candidate.
6. Output-field lowering for current macro syntax must retain the existing
   schema-order behavior. The general pure raw builder preserves the supplied
   checker-valid field order; a pure surface-lowering helper, not the macro,
   performs the current schema-order projection and is parity-tested.
7. Invoke `checkModel` for complete-model certification. A successful final
   result must retain either the returned checked model or sufficient evidence
   to expose the public acceptance and exact-erasure theorem.
8. `buildModelShell_model_acceptance_and_erasure` and
   `buildTransitionOverlay_model_acceptance_and_erasure` are intermediate
   bridges only. Neither is the final-assembly theorem because their later-owned
   lists are intentionally empty.
9. Composition-source/linker construction remains outside the proved builder
   boundary. Accepting and preserving an opaque raw wire list does not establish
   `WiresWellFormed` or any composition claim.

## Structured failure and diagnostic boundary

Define one syntax-independent final builder error sum with exact wrappers for:

- `CoreBuilderError` when the final assembly directly invokes a core operation;
- `TransitionBuilderError` when it invokes a PRD 0008 adapter;
- observation surface-lowering failures only; and
- the exact final `ModelCheckError` returned by `checkModel`.

Do not duplicate the declaration/term/model categories already nested in
`TransitionBuilderError` or `ModelCheckError`. Final checker failures retain the
authoritative `ModelCheckPathSegment` path unchanged. Observation-specific
surface paths may cover box, input, output, output schema/field, ordinary view,
grouped view/key/band and summary box/view syntax positions, but may be used only
for lowering failures and token mapping; they must not form a parallel semantic
checker path type.

The final checker wrapper must preserve every reachable PRD 0006 observation
rejection constructor, including unresolved output/view/summary targets,
duplicate or mismatched output fields, invalid view reducer shape,
grouped-key count/name/sort/band failures and aggregate use in grouped filters.
The PRD 0006 `ModelTermErrorCategory` constructor names must be listed in the
implementation traceability matrix so translation review is mechanical.

`frontend/Sembla/DSL.lean` remains a trusted adapter for parsing, retaining token
tables and rendering diagnostics. Its translation from final builder
category/path values to source positions must be exhaustive. It must not rerun
name, schema or term-validity logic to choose a diagnostic. Remaining macro
checks must be explicitly classified as parsing or current surface-shape
compatibility checks.

Exact positioned-diagnostic compatibility is governed by the existing negative
harness, including exact lines and columns. This PRD does not permit changing
that harness or its expected fixtures.

## Macro delegation requirements

Refactor current parameter/table/model, transition/effect/contest and
input/output/view/grouped-view/summary paths so that:

1. macros parse syntax and retain source-token/index information;
2. pure PRD 0007–0009 APIs perform semantic candidate construction,
   declaration/term resolution and complete model assembly;
3. macros translate structured failures to the existing diagnostic categories
   and positions; and
4. macros splice the successful exact raw result without independently
   reconstructing semantic IR structures.

The race-only current transition surface must delegate specifically through
`buildSurfaceTransition`.

Existing composition-source and wire macros may remain compatibility-tested
adapters. They may pass an exact opaque raw wire list to final assembly, but no
composition checking or proof claim may be introduced.

Parsing, macro expansion, token-to-path mapping and diagnostic rendering must be
documented as trusted/tested rather than verified.

## Required theorem matrix

Named public theorem/lemma declarations must include statements equivalent to:

| Obligation | Required statement |
| --- | --- |
| Observation constructor fidelity | Successful input/output/view/grouped-view/summary construction retains every supplied raw field exactly. |
| Contextual observation soundness | Fragment builders prove exact raw constructor/lowering fidelity. For the complete candidate, successful final `checkModel` certification establishes the corresponding PRD 0006 checked observation obligations and exact erasure; no duplicate standalone component checker is required. |
| Output ordering | The current surface-lowering helper emits output builder fields in the existing schema order; the general raw builder preserves its supplied order. |
| Core/transition preservation | Final assembly retains every PRD 0007 core field and every PRD 0008 transition at the same box/source ordinal. |
| Observation attachment | Inputs, outputs and views attach to the intended existing box ordinal; summaries retain exact model source order. |
| Wire preservation | Final assembly retains the supplied raw wire list exactly without claiming wire validity. |
| Model acceptance and erasure | Successful final assembly has some `checked` with `checkModel raw = .ok checked` and `checked.erase = raw`. |
| Success completeness | Every documented complete model candidate satisfying the authoritative PRD 0005/0006 predicates is reproduced without structural change. |
| Failure characterization | Final builder failure corresponds to an underlying core operation, PRD 0008 adapter, observation surface-lowering, or exact final model-check failure; it is not defined circularly through builder success. |

All named theorems enter the automated axiom and opaque-proposition audit.
Computed fixtures and macro parity tests do not replace these statements.

## Required fixture matrix

Direct builder fixtures must cover:

- every accepted input, output, ordinary-view, grouped-view and summary
  constructor, including every output operation, ordinary-view reducer and
  summary reducer;
- multiple inputs, outputs, output fields, views, grouped keys and summaries in
  exact order;
- current schema-order output lowering from syntax-independent raw field
  specifications, including interleaved source syntax;
- every observation-specific rejection category and nested term error family;
- multiple boxes with different transition/observation counts, proving ordinal
  attachment without truncation or reassignment;
- a complete input-bearing model whose transition or observation term uses an
  input aggregate;
- complete-model declaration/checker acceptance and exact checked erasure;
- an exact nonempty raw wire-list preservation fixture with no validity claim;
  and
- complete raw equality for representative current command-model shapes,
  including the feature-tour and interleaved-order fixtures.

Run the existing positive/negative elaboration, canonical-model and export
parity suites. No canonical fixture regeneration is permitted without an
explicit schema decision. Update `docs/design/lean-ir-coverage.md` with literal
builder/category fixtures and update `frontend/README.md` to state the exact
proved-builder versus trusted-macro boundary.

## Allowed files

- `frontend/Sembla/Frontend/Builders/Observation.lean`
- `frontend/Sembla/Frontend/Builders/ObservationTests.lean`
- `frontend/Sembla/Frontend/Builders.lean`
- `frontend/Sembla/DSL.lean`
- `frontend/Sembla/CommandFrontendTests.lean`
- `frontend/Sembla.lean`
- `frontend/README.md`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Editing accepted core, transition or `Sembla.Semantics` modules. If their
  public contracts are insufficient, stop and amend scope rather than weakening
  encapsulation.
- Verifying Lean metaprograms, parser expansion, token bookkeeping or diagnostic
  rendering.
- Changing public syntax, positioned diagnostic expectations, canonical raw
  models or export bytes.
- Composition-source/linker refactoring, wire validity, wire delivery or
  composition preservation proofs.
- Behavioral evaluation semantics.

## Test and proof guidance

Run direct builder tests and all existing elaboration/export suites. Compare
complete exact raw values and checked erasures, not pretty-printed fragments.
Use one-defect fixtures for structured failures and retain the existing
positioned negative harness as the compatibility oracle.

Run at least:

```bash
(cd frontend && lake build Sembla.Frontend.Builders.ObservationTests)
(cd frontend && lake build Sembla.CommandFrontendTests)
bash frontend/scripts/test-negative.sh
bash frontend/scripts/check-proofs.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
bash scripts/check.sh
git diff --check
```

No new verified builder/proof declaration may contain `sorry`, `admit`,
`axiom`, `native_decide`, `unsafe`, `implemented_by` or an opaque semantic
proposition. Existing trusted evaluator support in `DSL.lean` is outside this
verified declaration boundary and must not be presented as proved.

## Acceptance criteria

1. Every current model-local semantic construction path delegates to the pure
   PRD 0007–0009 APIs, culminating in one complete raw-model assembly function.
2. Final assembly preserves exact core, transition, observation, summary and raw
   wire structure, is accepted by `checkModel`, and has exact checked erasure.
3. Builder soundness, completeness, failure, attachment/order and erasure
   theorem families pass the automated audit.
4. Macros contain only parsing/current-surface compatibility, token bookkeeping,
   exhaustive error translation and result splicing; this trusted boundary is
   documented honestly.
5. Direct builder fixtures, positive/negative elaboration, positioned
   diagnostics, canonical raw models and export bytes are unchanged and pass.
6. Focused build, proof hygiene, movable-path allowlist, full repository checks
   and `git diff --check` pass within the allowed file list.

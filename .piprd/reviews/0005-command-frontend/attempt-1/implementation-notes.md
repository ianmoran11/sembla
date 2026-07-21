# PRD 0005 implementation notes

## Architecture

- Added the indentation-structured `sembla_model` parser, token-retaining command collectors, and ordinary declaration adapter in `frontend/Sembla/DSL.lean`.
- Command declarations are stable-partitioned directly into the existing `SurfaceModel` / `SurfaceBox` graph. Parameters and reaction arrows reuse their original syntax nodes and collectors; changed command forms construct only the matching surface records.
- The command adapter calls `elaborateSurfaceModel` once and installs its returned checked expression as one namespace-qualified safe `Model` definition. It does not quote `model%`, add another `Model.mk`, or duplicate semantic, schema, resolver, type, widget, or IR code.
- Model and system runtime names use the existing frozen derivation helper or the exact optional `name` override. Header, declaration, expression, endpoint, attribute, variant, and effect tokens remain source anchored.
- General-transition guard/hazard/effect cardinality and malformed command recovery are checked while collecting because malformed multiplicities cannot inhabit `SurfaceTransitionBody.general`; all semantic checks remain in the shared kernel.
- Decimal row literals with underscore grouping are normalized only into the existing natural-number surface term before shared row validation.

## Positive coverage

`frontend/Sembla/CommandFrontendTests.lean` is imported and covers:

- ordinary constants in a namespace, derived and explicit runtime model names, exact headers, and an empty command model without category boilerplate;
- a complete command/legacy twin with prior/priorless and forward-referenced parameters, two boxes, forward system and input Ref targets, all four system/input attribute types, frequency reaction hazard, multi-effect general transitions, typed outputs, two feedback wires, all view and summary reductions, and underscore row grouping;
- structural `Model` equality, literal serialized `Sembla.IR.toJson` string equality, input schema shape, and equal state/hazard widget data for a system, reaction arrow, and general transitions;
- all four frozen reaction-arrow restriction forms; and
- a separate deliberately interleaved command/legacy twin with structural and literal serialized-JSON equality, equal representative state/hazard widget props, and direct guards pinning model/box/table/transition/input/output/view/wire/summary, attribute, enum variant, schema/builder field, and effect order.

## Exact diagnostics

Added complete `check_failure_exact` fixtures for command headers; all duplicate declaration namespaces; indentation/misplacement; unsupported declarations; Ref/enum/row rules; transition system/attribute/parameter/input resolution; output/view/wire/summary resolution; deterministic arrow ambiguities; schema mismatch and duplicate delivery; numeric input restrictions and Ref effects; guard/hazard/effect/output/view typing; malformed count/non-count forms; and every general-transition cardinality failure. Expectations compare the full ordered positioned error set.

## Widget cursor check note

Automated data-level tests prove state and hazard props equal the legacy twin and retain separate source nodes for system and transition anchors. Manual VS Code checks were completed in `CommandFrontendTests.lean`:

1. Cursoring `system Person` on line 26 selected the `person` state-machine panel (`S`, `I`, `R`; `infect: S → I`).
2. Cursoring the `infect` arrow name on line 34 selected the distinct `infect` panel and its expanded frequency hazard.
3. Cursoring the general-transition name `adjust` on line 38 selected the distinct `adjust` panel with hazard `0.1`.

No widget visual redesign was made.

## Review revision

- Preserved the complete original grouped-row token span when normalizing underscore-separated row literals by attaching canonical `SourceInfo.fromRef stx.raw` metadata to the generated natural-number syntax.
- Kept u64 validation exclusively in the shared `validateSize` path; no parser-local semantic check, `SurfaceSystem` change, grammar change, or duplicate builder was introduced.
- Added `CommandGroupedOversizedRows.lean` alongside the existing ungrouped fixture. Its complete exact diagnostic now points to the original grouped literal at line 5, column 22.

## Validation

After the review revision, the focused command twin, both grouped and ungrouped exact row diagnostics, complete exact diagnostic harness, full Lean build, canonical/runtime parity, full repository suite, whitespace check, and frozen-path diff all passed. No commit was created.

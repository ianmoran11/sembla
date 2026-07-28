# PRD 0003 implementation notes

## Implementation

- Added all four frozen reaction-arrow forms to the existing `semblaTransition` category while retaining general transitions.
- Extended the collected `SurfaceTransition` representation with a tagged body that keeps the original name, optional system, optional state attribute, source, hazard, and destination tokens.
- Added deterministic full-box system and enum-state inference with stable source-order candidate diagnostics. Explicit system and attribute labels use the same collected declarations and resolve both ambiguity classes.
- Lowered reactions through the existing `Transition.mk`, `Expr.enumIs`, and `Effect.setAttr` nodes with one guard, one effect, and no contests.
- Kept hazard elaboration/type checking common to general and arrow transitions. Extracted shared identifier-effect validation so both paths use the same enum-membership, Ref-write, and value-type checks.
- Retained transition order and attached both state-diagram and hazard-panel widgets to the original transition-name token using the resolved system.
- Added a targeted trailing guard/effect recovery form with a clear arrow-limitation diagnostic.

## Positive coverage

`frontend/Sembla/ReactionArrowTests.lean` is imported by `frontend/Sembla.lean` and registered in the elaboration harness. It covers:

- all four frozen arrow forms;
- exact structural and `toJson` twins against expanded general transitions;
- exact transition/effect order and zero contests;
- inferred and explicit systems/attributes;
- self-loops;
- Greek bare-parameter hazards;
- legacy `countBy`/`sizeBy` hazards;
- complete candidate collection with the valid system declared second;
- explicit disambiguation of multiple systems and multiple enum attributes; and
- equal `StateDiagramProps` and `HazardPanelProps` for every representative twin.

## Negative coverage

Added exact positioned failures for unknown source/destination variants, split-attribute endpoints, unknown explicit system/attribute, explicit non-enum attributes, zero/multiple inferred systems, multiple enum attributes, non-Real hazards, and trailing arrow effects. Every PRD 0003 fixture uses `check_failure_exact`.

## Validation

All passed:

- `cd frontend && lake build`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- `./scripts/check.sh`
- `git diff --check`
- frozen-path diff over examples, IR/JSON, canonical models, manifests, Rust, CI, and docs

No canonical model/fixture, IR/JSON, Rust, dependency, CI, or public documentation file changed. No commit was created.

## Revision after review

- Corrected unlabelled system compatibility to require one enum attribute containing both source and destination variants. Split-column systems no longer create false multiple-system ambiguities.
- Strengthened the existing collected-candidate arrow/expanded twin: an earlier system now contains the endpoints on different enum columns, while the valid `Person.health` system remains declared second.
- Confirmed the explicit-system split-column negative retains its exact positioned diagnostic.
- Re-ran `lake build`, `scripts/test-negative.sh`, `scripts/check-parity.sh`, `./scripts/check.sh`, `git diff --check`, and the frozen-path diff; all passed.

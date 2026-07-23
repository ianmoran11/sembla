# PRD 0005 implementation notes — attempt 1

## Scope

- Added pure one-level composition props/builders from elaborated `ComponentDefinitionV1` and `CompositionSourceV1` values.
- Added a theme-aware HTML composition renderer and declaration-panel attachment for both `sembla_component` and `sembla_composition`.
- Added pure prop/rendering guards, documentation, and no runtime/simulation behavior.
- Existing state/hazard renderer bodies, schemas, plans, fixtures, demos, diagnostics, and dependencies remain unchanged.

## Definition catalog resolution

A bare `ComponentDefinitionV1` stores an instance's referenced definition id but not the referenced definition's display name or boundary ports. The required one-argument `diagramPropsOfDefinition` therefore remains available and is complete for primitives, with deterministic id-derived fallbacks for unresolved composite children. `diagramPropsOfDefinitionWithDefinitions` accepts the already-elaborated definition catalog and renders direct children accurately without recursive expansion. Component attachment passes its already-resolved direct child values; source attachment passes `CompositionSourceV1.definitions`. No JSON parsing, source-map lookup, or re-elaboration is used.

## Primitive-body panel investigation and decision

`Sembla.DSL.elaborateSurfaceModelCore` exposes only an all-or-none `attachWidgets : Bool` policy. Enabling it for the synthetic primitive-component model would attach both state and hazard panels. The synthetic wrapper intentionally supplies dummy `dt := 1.0` and requirement defaults of `0.0`; the hazard builder uses those values for defaults and firing-probability plots, so enabling the existing path would display invented behavioral data. A state-only hook would require changing `frontend/Sembla/DSL.lean`, outside this PRD's allowed files and beyond attachment-only scope.

Primitive body state/hazard reattachment is therefore **deferred**. Primitive declarations receive the required composition interface panel (one primitive box plus author-ordered boundary ports). This preserves the one-kernel rule and avoids misleading behavior panels.

## Composition prior-panel investigation

There is no standalone prior-marginal props builder or panel. Prior curves are private pieces of `HazardPanelProps` and require a concrete transition; the composition's parameter-only synthetic model has no boxes or transitions. Reuse is therefore not possible without changing the existing widget architecture, so no prior panel is fabricated.

## Fixture note

`SurfaceModels.lean` contains the required surface-authored `Population`, exposing `EpidemicPolicy`, and `TwoRegions` values, and their guards exercise the surface-to-props path. It does not define `RegionalResponse`, and that file is not in this PRD's allowed files. The corresponding existing canonical elaborated value `Sembla.Composition.Fixtures.regionalResponse` is used for the required exposure-plus-hidden guard; no fixture or demo golden was changed.

## Rendering discipline

- Nodes preserve instance order and distinguish primitive (solid card) from composite (dashed card).
- Delayed wires use solid accent rows and render the numeric `N-tick delay` marker.
- Exposures use dashed accent rows and render `zero-delay alias`.
- Hidden ports render in a separate struck-through row.
- Academic, editor, and notebook themes all pass through the existing `widgetShell` mechanism.
- All React `style` values remain JSON objects; no JavaScript component, asset, layout engine, or dependency was added.

## Validation

- `cd frontend && lake build`: passed; all widget guards and every composition/demo command elaborated with attachment enabled.
- `bash frontend/scripts/test-negative.sh`: passed unchanged.
- `bash frontend/scripts/check-parity.sh`: passed; canonical source/plan/export and showcase bytes remain identical.
- `./scripts/check.sh`: passed.
- Focused `lake build Sembla.WidgetTests Sembla.Composition.WidgetTests Sembla.Composition.SurfaceModels`: passed.
- `git diff --check` plus explicit checks for the new untracked Lean files: passed.
- No Rust, dependency manifest, frozen fixture, example, demo, or demo-golden diff is present.

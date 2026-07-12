# PRD 0011: Lean structure widgets

## Context

The two v0.1 widgets (`DESIGN.md` §3, §9) are **structure widgets**: rendered
from the elaborated model with zero runtime — this is the Lean-specific
capability the frontend was chosen for. Behavior widgets
(slider → simulate → plot) are explicitly out of scope until the runtime
latency budget exists (§10.6). Builds on the PRD-0010 DSL and model types.

## Goal

Two infoview widgets in `frontend/`: (1) a state-machine diagram for the
system under the cursor, (2) a hazard/parameter summary panel with a plotted
curve — both driven purely by elaborated data, plus tests on the *data* the
widgets render (pixels are verified by documented manual steps).

## Specification

- Add `ProofWidgets4` as the only new dependency (pin its rev in the Lake
  manifest).
- **Widget 1 — state diagram**: with the cursor on a `system` or
  `transition` declaration, the infoview shows a directed graph: nodes = the
  enum states of that system's state attributes; edges = transitions
  (labelled with name and hazard expression pretty-printed). Render via
  ProofWidgets' HTML/SVG components (a simple layered/circular layout is
  fine; do not add a JS graph library unless ProofWidgets already vendors
  one).
- **Widget 2 — parameter & hazard panel**: with the cursor on a
  `transition`, show its guard and hazard pretty-printed, and for each
  `param` it references: the default value and — when a prior is declared —
  an inline SVG plot of the **prior density** (Normal / LogNormal / Uniform
  closed-form curves; this is the "rendered priors" feature of `DESIGN.md`
  §3). Below that, an inline SVG plot of the implied per-tick firing
  probability `p(dt) = 1 − exp(−λ·dt)` as a function of dt over a sensible
  range, at the param defaults (evaluable only when the hazard is a closed
  expression over params — otherwise show the expression and skip that plot,
  stating why; the prior plots render regardless).
- Architecture requirement: each widget is a pure function
  `elaborated model data → widget props (JSON)`, with the RPC/display layer
  kept thin. The pure functions are what the automated tests target.
- `frontend/README.md` gains a "Widgets" section with screenshots-optional,
  step-by-step manual verification instructions (open file X, place cursor
  on line Y, expect Z).

## Non-goals

Behavior widgets, sliders, editing-source-from-widget round-trips, wiring
diagram rendering for multi-box models (add nodes-for-boxes only if trivial;
otherwise defer), any Rust changes.

## Acceptance criteria

1. `lake build` succeeds with ProofWidgets pinned; PRD-0010 parity tests
   still pass.
2. Automated: a test elaborates the SIR model and asserts widget 1's props
   JSON contains exactly nodes `{S, I, R}` and edges
   `infect: S→I`, `recover: I→R` with their hazard strings.
3. Automated: a test asserts widget 2's props for `recover` include γ's
   default value and a p(dt) polyline with > 10 points whose first point is
   ≈ (0, 0) and which is monotone increasing; that for `infect` (hazard
   depends on an aggregate) the props carry the no-plot explanation for the
   p(dt) curve while still carrying β's **prior density polyline** (checked
   against the closed-form LogNormal density at 3 sampled x-values); and
   that a prior-less param yields default-value-only props with no density
   curve.
4. The widget code path contains no simulation execution and no IO beyond
   the widget RPC layer (verified by inspection in review; the pure
   prop-builders are IO-free by type).
5. Manual verification instructions exist in `frontend/README.md`, precise
   enough to follow without reading the source.

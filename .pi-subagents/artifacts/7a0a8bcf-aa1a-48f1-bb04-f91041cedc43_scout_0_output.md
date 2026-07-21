# Code Context

## Files Retrieved

1. `docs/design/composition-options.md` (lines 1-42, 128-194, 528-584, 925-993) - authoritative composition discussion, current flat baseline, constraints, and explicitly illustrative future syntax.
2. `docs/design/surface-syntax-options.md` (lines 1-72, 108-207) - implemented-vs-deferred status and reusable public Lean fragments.
3. `frontend/Sembla/Models.lean` (lines 7-28, 46-102, 104-192) - executable canonical model syntax and candidate domains.
4. `frontend/Sembla/Demos/Modeling.lean` (lines 1-113) - executable full feature tour covering schemas, ports, transitions, outputs, wires, views, and summaries.
5. `frontend/Sembla/IR.lean` (lines 1-159) - actual deep IR types; proves the currently representable model/box/schema/wire shape.
6. `frontend/Sembla/Demos/CanonicalModels.lean` (lines 1-56) - catalog of eight existing domain models and structural checks.

## Key Code

### Safe, implemented public Lean fragments

The public authoring entry point is `sembla_model`, not `sembla_component` (`docs/design/surface-syntax-options.md:1-27,45-72`):

```lean
sembla_model Example (name := "runtime_name") (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231) 0.25

  box population where
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      visits : Int
      employer : Employer
```

These schema forms are executable today: enum `{...}`, `ℝ`, `Int`, and a reference written as the referenced system name. See `frontend/Sembla/Demos/Modeling.lean:27-42`. Runtime population initialization remains external; `rows` is only an IR size hint (`frontend/Sembla/Demos/Modeling.lean:16-21`).

Typed input/output ports and exact field schemas (`frontend/Sembla/Demos/Modeling.lean:43-64`):

```lean
input restriction where
  modifier : ℝ

output activity from Person where
  infected : Int := count where health = I
  total_risk : ℝ := sum (risk)
```

Reaction-arrow and general transitions are both current (`docs/design/surface-syntax-options.md:133-170`):

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I

transition restrict on Controller where
  guard mode = Open ∧ inputSum activity field infected > 100
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

Use arrows only for exactly one enum equality guard and one write to that enum attribute; compound guards, multiple effects, and input-dependent rules require the general form (`docs/design/surface-syntax-options.md:133-170`). `freq (...) over refKey` is implemented, but keyed comprehension `#{q ∈ ...}` is deferred (`docs/design/surface-syntax-options.md:172-193`).

Current wire syntax is space-qualified, not dotted (`frontend/Sembla/Models.lean:99-100`):

```lean
wire population infection_count -> policy infection_count
wire policy restriction_modifier -> population restriction_modifier
```

Current views/summaries (`frontend/Sembla/Models.lean:23-28`; `frontend/Sembla/Demos/Modeling.lean:90-102`):

```lean
view I := count Person where health = I
view total_risk := sum Person using risk
summary peak_I := max population.I
summary peak_tick := argmaxₜ population.I
```

### Actual representational boundary

`frontend/Sembla/IR.lean:95-159` defines `PortDecl`, `OutputDecl`, `ViewDecl`, flat `Box`, `WireEndpoint`, `Wire`, `SummaryDecl`, and `Model`. In particular:

```lean
structure Box where
  name : String
  tables : List Table
  transitions : List Transition
  inputs : List PortDecl
  outputs : List OutputDecl
  views : List ViewDecl

structure Model where
  name : String
  dt : Scientific
  params : List ParamDecl
  boxes : List Box
  wires : List Wire
  summaries : List SummaryDecl
```

There is no constraint/invariant type, reusable component, instance, product expression, child box, exposure, hiding, or synchronized family. The design note says this directly at `docs/design/composition-options.md:128-194`.

### Proposed-only fragments: label conspicuously

Everything below is illustrative and **not implemented/frozen**; the note explicitly says so at `docs/design/composition-options.md:925-927`:

```lean
sembla_component Independent := Population ⊗ Policy

sembla_component Coupled where
  instance population := Population
  instance policy := Policy
  wire population.infection_count -> policy.infection_count
  expose population.population_state as population_state

sembla_component PublicPolicyModel :=
  hide Coupled.policy_debug
  expose Coupled.population_state

sembla_component BalancedLedger :=
  restrict (DebitLedger ⊗ CreditLedger) where
    invariant balanced :=
      DebitLedger.exposed_total = CreditLedger.exposed_total
```

Exact source: `docs/design/composition-options.md:929-978`. Note the proposed dotted wire endpoints conflict with the current executable space-qualified wire grammar; do not present them as current Lean.

The synchronized `family ... match ... leg ... claims ...` fragment at `docs/design/composition-options.md:980-993` is even more tentative: row correlation, event identity, clock ownership, and scheduler coordination are unresolved. The earlier constraint spelling `component BalancedLedger := ...` at `docs/design/composition-options.md:528-550` also differs from later `sembla_component`; neither is frozen.

## Architecture

The implemented command frontend elaborates indentation-structured `sembla_model` declarations into the flat deep IR. A model owns ordered parameters, boxes, model-level wires, and summaries; a box owns systems/tables, transitions, typed ports, outputs, and views. Wires require compatible ordered schemas and carry tables with one-tick delay; they are not same-tick transition synchronization (`frontend/Sembla/Demos/Modeling.lean:16-21`; `docs/design/composition-options.md:128-180`).

The proposed composition layer would sit above this representation: reusable definitions/instances/product/wiring/hiding/restriction would be linked deterministically back to the existing flat plan. No such source representation or linker exists yet (`docs/design/composition-options.md:31-42,896-923`). Therefore composition examples may safely embed current box internals verbatim, but every surrounding `sembla_component`, `instance`, `⊗`, dotted wire, `expose`, `hide`, `restrict`, and `invariant` form must be marked proposed.

## Existing domains for concrete composed examples

1. **SIR population + policy feedback** — best direct example. `frontend/Sembla/Models.lean:46-102` already has two boxes, reciprocal typed ports, general controller transitions, two delayed wires, and summaries. It can be refactored illustratively into reusable `Population` and `Policy` components without inventing domain behavior.
2. **Reversible two-state CTMC + radioactive decay chain** — compact independent-product example. Both are self-contained single-box reaction-arrow models (`frontend/Sembla/Models.lean:104-130`), making noninterference/product syntax easy to show without wiring.
3. **SIS importation + SEIRS waning (or noisy voter)** — parameterized frequency-dependent components with declared reference keys (`frontend/Sembla/Models.lean:132-192`). Suitable for multiple-instance/tagged-interface examples, but they currently expose no ports, so any wiring or cross-component constraint would be invented and must be labeled hypothetical.

## Warnings

- Do not call model-level `box` declarations reusable components; they cannot currently be independently defined or instantiated.
- Do not reuse `expose on Person` from `frontend/Sembla/Models.lean:167` as composition exposure: there it is merely a transition named `expose`.
- Product means machine/component parallel product, not factored table attributes or relational Cartesian product (`docs/design/composition-options.md:68-113`).
- Constraints/invariants have no current Lean or IR syntax. The proposed form is genuine component-level restriction over explicitly exposed projections, not arbitrary hidden-table or row-level relational inspection (`docs/design/composition-options.md:528-584`).
- Current wire endpoints use `wire box port -> box port`; proposed component examples use dotted paths. Keep that distinction visible.
- Current stochastic IDs depend on flat declaration order, so strong product/noninterference claims are not yet true for draw coordinates (`docs/design/composition-options.md:182-200`).

## Start Here

Open `frontend/Sembla/Models.lean:46-102` first. It is the smallest authoritative, executable bridge from today’s flat two-box syntax to a concrete proposed composition example, including schemas, transitions, ports, reciprocal wires, and observations.

## Acceptance

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Only the requested scout artifact was written; project/source files were not modified by this task."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "Mapping cites exact file/line ranges, separates implemented from proposed syntax, and identifies three concrete domain choices."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/7a0a8bcf-aa1a-48f1-bb04-f91041cedc43/composition-examples/current-syntax.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "targeted find/grep/read inspection of Lean and design documentation",
      "result": "passed",
      "summary": "Located current DSL, deep IR, canonical models, and proposed composition fragments."
    },
    {
      "command": "git status --short",
      "result": "passed",
      "summary": "Observed pre-existing modified/untracked workspace files; no staged-file indicators were shown."
    }
  ],
  "validationOutput": [
    "Current syntax cross-checked against executable frontend/Sembla/Models.lean and Demos/Modeling.lean.",
    "Proposed-only status cross-checked against composition-options.md explicit non-freezing warnings and current IR shape."
  ],
  "residualRisks": [
    "Workspace already contained unrelated modified/untracked files, including docs/design/composition-options.md; findings describe the inspected working tree, not necessarily HEAD.",
    "No compilation was run because this was a read-only syntax mapping task."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added one requested Markdown scout artifact; no project/source edits.",
  "reviewFindings": [
    "no blockers",
    "warning: proposed dotted component wire syntax must not be confused with current space-qualified model wire syntax"
  ],
  "manualNotes": "Review gate remains for the parent/reviewer."
}
```

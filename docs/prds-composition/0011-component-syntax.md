# PRD 0011: `sembla_component` surface syntax and twin conformance

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. The
whole pipeline now runs from hand/Lean-value sources. This PRD adds the human
authoring surface: Lean commands that elaborate to `ComponentDefinitionV1` /
`CompositionSourceV1` values, reusing the existing surface kernel
(`frontend/Sembla/DSL.lean`, extracted in the surface-syntax track's
PRD 0001) for primitive bodies. The acceptance keystone is the **twin test**:
the Lean-authored `epidemic_policy` composition must serialize to canonical
source bytes byte-identical to the checked-in fixture from PRD 0006, and link
to the identical plan.

Frozen design constraints:

- Components become ordinary Lean constants: `sembla_component Population
  where …` elaborates to `def Population : Sembla.Composition.
  ComponentDefinitionV1 := …`. Reuse across compositions is Lean name
  reference — no new registry or environment extension beyond what constant
  elaboration gives for free.
- Stable-id slugs derive from Lean identifiers via the **existing frozen
  runtime-name derivation** (`docs/prds-surface-syntax/README.md`,
  "Frozen runtime-name derivation"): `Population → def:population`,
  `EpidemicPolicy → def:epidemic_policy`. Instance/wire/port slugs derive
  the same way from their surface names. Non-derivable identifiers are
  positioned errors, same rules as the existing track.
- No inert syntax (DESIGN.md §5.5): every accepted form fully elaborates;
  everything else is a positioned diagnostic.

## Goal

`sembla_component` (primitive and composite) and `sembla_composition`
commands elaborate to source values; `sembla-export --source` accepts
Lean-authored compositions; the Lean-authored twins byte-match the PRD 0006
fixtures; negative diagnostics are pinned in the negative suite.

## Specification

### 1. Primitive components

```lean
sembla_component Population where
  requires beta
  requires gamma
  requires restriction_scale
  input restriction_modifier where
    restriction : ℝ
  output infection_count from Person where
    infected : Int := count where health = I
  system Person (rows := 1000) where
    health : {S, I, R}
    employer : Employer
  system Employer (rows := 50)
  infect on Person : health: S →[ … ] I
  recover on Person : health: I →[ … ] R
  view S := count Person where health = S
  …
```

- The body between the port/`requires` declarations and the end is the
  **existing box-body surface** (systems, transitions, reaction arrows,
  views, inputs, outputs) — feed it through the existing surface kernel's
  collection/validation path exactly as `sembla_model` box bodies do. Do not
  build a second elaborator for bodies; the PRD fails review if a duplicate
  body pipeline appears.
- `requires <ident>` declares a parameter requirement (slug-derived name).
  Inside the body, requirement names resolve like model parameters do in
  `sembla_model` (same ambiguity/unknown diagnostics), but elaborate to
  `Param` references by requirement name.
- Ports: each `input`/`output` in the body induces the definition's
  `PortDeclV1` (id `port:` + declaration name, direction, schema) —
  generated, not separately declared, so the PRD 0006 port/body 1:1 rule
  holds by construction.
- Result: `def Population : ComponentDefinitionV1` with a `#guard`-able
  value. Elaboration must be deterministic (no `Name`-hash or `HashMap`
  iteration order in emitted lists — use author order everywhere).

### 2. Composite components

```lean
sembla_component EpidemicPolicy where
  instance population := Population
  instance policy := Policy (response_threshold := response_threshold)
  wire population.infection_count -> policy.infection_count
  wire policy.restriction_modifier -> population.restriction_modifier
  expose population.infection_count as infection_count
  hide policy.some_port
```

- `instance <name> := <Component>` — `<Component>` is a term elaborating to
  a `ComponentDefinitionV1` constant. Optional named-argument list
  `(<req> := <modelParam>)` sets parameter bindings; omitted requirements
  bind to the **same-named** model parameter by default (elaboration records
  the explicit full binding list in the value — defaulting is surface sugar
  only, so serialized bytes always carry complete bindings).
- Wire slugs: derived as `wire:` + source port slug + `_to_` + target
  instance slug (e.g. `wire:infection_count_to_policy`)… **No.** The PRD
  0006 fixtures froze `wire:count_to_policy` and
  `wire:restriction_to_population`; auto-derivation cannot reproduce those.
  Therefore wires take an explicit label: the frozen surface form is

  ```lean
  wire count_to_policy : population.infection_count -> policy.infection_count
  ```

  with the label deriving the wire slug. Exposures likewise:
  `expose <label> : population.infection_count as infection_count` deriving
  `expose:<label>` and outer `port:infection_count`. `hide` needs no label
  (`hide policy.restriction_modifier`).
- Composite components do not accept `requires`, primitive body forms,
  `param`, or `dt` — positioned errors naming the restriction.

### 3. Root composition

```lean
sembla_composition epidemicPolicyModel
    (name := "epidemic_policy") (dt := 0.25) where
  param beta : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
  param gamma : ℝ := 0.1
  …
  root EpidemicPolicy
  summary peak_i := max north.population.I   -- instance-path dotted form
```

Elaborates to `def epidemicPolicyModel : CompositionSourceV1` with
`model_id` from `(name := …)` (mandatory here, unlike `sembla_model` — the
model id must be an exact slug), `outer_dt` from `dt`, `param` declarations
reusing the existing frozen param/prior surface, one mandatory `root`, and
summaries whose dotted path resolves at *serialization-time* only
syntactically (instance-path slugs; semantic resolution stays the linker's
job — surface must not duplicate linker validation, only produce the
`SourceSummaryV1` record).

### 4. Export and twin conformance

- Extend `frontend/Main.lean`: `sembla-export --source` gains registry
  entries for Lean-authored compositions. Author in a new
  `frontend/Sembla/Composition/SurfaceModels.lean`, using the surface
  syntax, twins of: `epidemic_policy`, `independent_epidemic_policy`, and
  `two_regions` (with the exposing `EpidemicPolicy` variant, per the
  PRD 0006 fixture family). The primitive bodies must reproduce the fixture
  bodies (scaled `sir_policy` boxes) exactly — same attrs, rates, views,
  order.
- **Twin tests** (the keystone; in `frontend/Sembla/Composition/
  SurfaceTests.lean`): for each authored twin, `#guard` that its rendered
  canonical source bytes equal the checked-in
  `fixtures/composition-source/<name>.source.json` bytes (read at
  elaboration time is not available — instead `#guard` value equality
  against the corresponding `Fixtures.lean` value **and** add a parity-script
  section exporting the surface twin and `cmp`-ing against the same fixture
  file the PRD 0006 section uses). Value equality + byte equality together
  prove surface, fixture values, and serialized contract agree.
- Since linking is deterministic, plan-level twin equality follows from
  source-byte equality — assert it anyway for `epidemic_policy` with one
  `#guard` (link both, compare rendered plans) as a cheap end-to-end seal.

### 5. Diagnostics and negative suite

Every new command must reject with positioned errors (extend
`frontend/Negative/` + `frontend/scripts/test-negative.sh` with exact
`file:line:column: error: …` expectations, using the complete-set helper
from the surface-syntax track):

- duplicate instance names; unknown component constant (Lean's own unknown
  identifier is acceptable if positioned at the term); unknown port in
  `wire`/`expose`/`hide`; unlabeled `wire`; non-derivable identifier
  (e.g. `instance North' := …`); `requires` in a composite; `dt` on a
  component; missing `root`; missing `(name := …)`; duplicate wire labels;
  binding an unknown requirement; summary path with a non-identifier
  segment.
- Diagnostics must anchor to the offending token, not a generated term —
  same standard as the surface-syntax track.

### 6. Registration and checks

Import new modules from `frontend/Sembla.lean`. Run the full check set
including `bash frontend/scripts/test-negative.sh` (this PRD touches surface
syntax, so the negative suite is mandatory per the folder README).

## Allowed files

- `frontend/Sembla/Composition/Surface.lean` (the commands),
  `SurfaceModels.lean`, `SurfaceTests.lean` (new)
- `frontend/Sembla/DSL.lean` (only if a kernel entry point needs a
  non-behavior-changing export; the existing `sembla_model` path must remain
  byte-identical — parity proves it)
- `frontend/Main.lean`, `frontend/Sembla.lean`
- `frontend/Negative/**`, `frontend/scripts/test-negative.sh` (additions
  only), `frontend/scripts/check-parity.sh` (append only)
- implementation notes/artifacts created by the managed run

## Non-goals

- `⊗` tensor notation, `restrict`, `family`, renames, scheduler
  annotations, or any §J12 construct — reject, don't reserve.
- New body-level expression syntax, priors, or types; the body surface is
  exactly the existing frozen one.
- Changing fixture bytes: if a twin can't reproduce the fixture, the twin
  (or its authoring) is wrong, not the fixture.
- Widgets for composition (defer; do not add anchors speculatively).

## Acceptance criteria

1. `lake build`, full parity (including the new surface-twin section), and
   `bash frontend/scripts/test-negative.sh` pass; `sembla_model` models and
   all legacy exports remain byte-identical.
2. Twin keystone: surface-authored `epidemic_policy`,
   `independent_epidemic_policy`, and `two_regions` produce canonical source
   bytes `cmp`-identical to the PRD 0006 fixtures, value-equal to
   `Fixtures.lean`, and (for `epidemic_policy`) link to the byte-identical
   plan.
3. Code inspection: primitive bodies elaborate through the existing surface
   kernel; no second body elaborator, expression checker, or IR builder
   exists.
4. Every listed negative case fails with an exact positioned diagnostic in
   the extended negative suite; no accepted construct is inert.
5. Parameter bindings serialize complete (defaulting leaves no trace in
   bytes), pinned by the twin byte-equality.
6. `./scripts/check.sh` and `git diff --check` pass.

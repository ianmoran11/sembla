# Surface syntax: implemented command-style mathematics

**Status:** Implemented, 2026-07-20.
**Scope:** Lean surface syntax only. The IR, JSON encoding, validator, Rust
runtime, proofs, and checked fixtures were unchanged. The public reference is
the [frontend README](../../frontend/README.md).

---

## 1. Decision and implemented status

The human authoring surface is the indentation-structured `sembla_model`
command. It layers mathematical notation over the same semantic kernel as the
supported legacy `model%` form and emits byte-identical IR.

| Option | Status | Result |
|---|---|---|
| B — binders, tilde priors, derived names, aliases | **Implemented** | Parameters are bindings; bare names resolve symbolically; runtime names derive predictably with explicit overrides. |
| A — reaction arrows | **Implemented** | One enum guard/effect is written as a reaction arrow with deterministic system/attribute inference. |
| C(ii) — `freq … over …` | **Implemented** | The exact keyed-frequency idiom has mathematical notation with the existing relational restrictions. |
| D — command-style declarations | **Implemented** | Models use declaration-shaped, indentation-structured blocks with distinct source anchors. |
| C(i) — keyed comprehensions | **Deferred** | Add only when a real model needs a non-frequency keyed count and can justify row-binder syntax. |
| E — do-notation builder | **Rejected/deferred for humans** | Direct IR constructors remain the machine-writer path; an imperative builder is not the public mathematical surface. |

`model%` remains the compatibility kernel. It is not deprecated or removed:
focused legacy positives and legacy/new twins retain it as a regression surface.
Both frontends collect the same surface records and call the same uniqueness,
scope, expression, type, schema, reference, ordering, widget, and IR-emission
path.

## 2. From compatibility syntax to the public surface

The original compatibility form is a single nested expression with explicit
list blocks and wire-format names:

```lean
private def legacy : Model := model% "sir_workplace_frequency_dependent" step(0.25) where
  params [param beta : Real := 0.8 prior LogNormal(-0.2231, 0.25)]
  boxes [box sir where
    systems [system Person as "person" rows(1000000) where [
      state health : {S, I, R}, ref employer : Employer]]
    inputs []
    transitions [transition infect on Person where
      guard health = S
      hazard parameter beta * (countBy employer (health = I) / sizeBy employer)
      set [health := I]]
    outputs []]
  wires []
```

It remains accepted, but new human-authored models use:

```lean
sembla_model SirWorkplace
    (name := "sir_workplace_frequency_dependent")
    (dt := 0.25) where
  param β : ℝ := 0.8 ~ LogNormal (-0.2231) 0.25

  box sir where
    system Person (rows := 1_000_000) where
      health : {S, I, R}
      employer : Employer
    system Employer (rows := 50_000)

    infect on Person : health: S →[
      β · freq (health = I) over employer
    ] I
```

The new form removes required empty categories, list brackets, separator commas,
keyword-marked parameter references, and quoted names that can be derived. Each
declaration retains its own source token for diagnostics and infoview cursor
targets.

## 3. Binding constraints retained

### Byte-stable IR

Every new form is sugar over the existing deep IR. Canonical migration was
accepted only after literal JSON `cmp`, normalized comparison, Rust validation,
and fixed-seed CSV/summary/hash parity. Stable source order remains observable:
parameters, boxes, systems, transitions, effects, ports, views, wires, and
summaries are never name-sorted.

### No inert syntax

Every accepted node either elaborates completely or emits a deliberate
positioned diagnostic. Complete ordered error sets are pinned by
[`frontend/scripts/test-negative.sh`](../../frontend/scripts/test-negative.sh)
and the focused `frontend/Negative/` command fixtures.

### Closed relational fragment

The surface still expresses only declared-key joins and commutative-monoid
reductions. Pretty notation does not expand the semantic fragment. There are no
new dependencies.

### Layered authoring paths

- `sembla_model`: public human authoring.
- `model%`: supported compatibility/kernel form and regression surface.
- direct `Sembla.IR` constructors: machine-writer and lower-level test path.

The IR remains frontend-agnostic (`DECISIONS.md` §A5).

## 4. Implemented option B: names and mathematical bindings

Parameters bind identifiers:

```lean
param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25
param γ : ℝ := 0.1
```

Hazards use `β` and `γ` directly while the emitted IR retains symbolic
`Expr.param "beta"` and `Expr.param "gamma"`. Defaults and priors remain
metadata, not substitutions (`DECISIONS.md` §G3).

Model and table runtime names derive through a frozen snake-case/transliteration
algorithm. Explicit `(name := "...")` is required when derivation would not
match the frozen wire name; for example, the observation fixture preserves the
uppercase table name `"Person"`. Derivation collisions are diagnosed.

Implemented aliases include `ℝ`, `·`, `∧`, `≠`, and `≤`, alongside supported
ASCII spellings. Legacy `parameter beta` and `system ... as "..."` forms remain
available only through the compatibility syntax; they did not “die” at the
kernel boundary.

## 5. Implemented option A: reaction arrows

A transition with exactly one enum equality guard and one write to that enum
attribute uses:

```lean
infect on Person : health: S →[β · freq (health = I) over employer] I
recover on Person : health: I →[γ] R
```

System inference is permitted only when one collected system is compatible.
State-attribute inference is permitted only when the selected system has
exactly one enum attribute; a multi-enum system requires explicit `health:`
syntax even when only one attribute contains both endpoints. Endpoint
compatibility is then validated against that selected attribute. Candidate
selection is deterministic and never depends on map iteration order.

Arrows intentionally do **not** cover every transition. The SIR-policy
controller has input-dependent compound guards and multiple ordered effects, so
its rules remain general command transitions:

```lean
transition restrict on Controller where
  guard mode = Open ∧ inputSum infection_count field infected > 500
  hazard 1e300
  set mode := Restricted
  set modifier := 0.4
```

The general form is also required for additional guards, multiple effects, or
other rules that cannot be represented as one enum reaction.

## 6. Implemented option C(ii): keyed frequency

The epidemiological idiom is:

```lean
freq (health = I) over employer
```

It emits the exact existing `countBy employer (health = I) / sizeBy employer`
IR plan. It requires a selected source system, a Ref key on that system, and a
Boolean row-local predicate. Nested frequency, input/relational aggregates,
non-Ref keys, and predicates outside the closed fragment are rejected.

### Deferred option C(i)

The proposed keyed comprehension:

```lean
#{q ∈ Person by employer | q.health = I}
```

remains deferred. It introduces row-variable and dotted-field scoping that the
canonical models do not need. It will be reconsidered only when a real model
requires a keyed count that is not exactly a frequency.

## 7. Implemented option D: command declarations

`sembla_model` defines one ordinary namespace-qualified `Model` constant.
Inside it, repeated/interleaved command declarations replace mandatory category
lists. Collection remains multi-pass, enabling supported forward references;
emission is a stable partition preserving relative source order within every IR
category.

The command supports all current model features: parameters and priors; systems
and enum/real/integer/Ref attributes; reaction and general transitions; typed
inputs and outputs; count/sum/min/max views; wires; and all five summary
reductions. It does not generate a giant quoted `model%` tree or maintain a
second semantic builder.

Distinct system, arrow, and general-transition syntax nodes are also distinct
infoview anchors, realizing the cursor/source-granular workflow described in
`DECISIONS.md` §A1.

## 8. Option E and machine generation

A do-notation builder remains inappropriate for human authoring:

```lean
-- Deliberately not the public style:
-- build do
--   let β ← param 0.8
```

It reads as imperative construction rather than a mathematical model. Generated
models should construct the deep IR directly. A typed builder can be revisited
only if future programmatic tooling demonstrates a need beyond raw constructors.

## 9. Evidence and references

- Public syntax and workflow: [`frontend/README.md`](../../frontend/README.md)
- Canonical command models: [`frontend/Sembla/Models.lean`](../../frontend/Sembla/Models.lean)
- Full command/legacy and ordering twins: [`frontend/Sembla/CommandFrontendTests.lean`](../../frontend/Sembla/CommandFrontendTests.lean)
- Binder/name twins: [`frontend/Sembla/SurfaceKernelTests.lean`](../../frontend/Sembla/SurfaceKernelTests.lean)
- Arrow twins: [`frontend/Sembla/ReactionArrowTests.lean`](../../frontend/Sembla/ReactionArrowTests.lean)
- Frequency twins: [`frontend/Sembla/FrequencyTests.lean`](../../frontend/Sembla/FrequencyTests.lean)
- Canonical names/order: [`frontend/Sembla/CanonicalModelsTests.lean`](../../frontend/Sembla/CanonicalModelsTests.lean)
- Positioned negatives: [`frontend/scripts/test-negative.sh`](../../frontend/scripts/test-negative.sh)
- Literal canonical/runtime parity: [`frontend/scripts/check-parity.sh`](../../frontend/scripts/check-parity.sh)

The complete implementation order and frozen contracts are recorded in
[`docs/prds-surface-syntax/README.md`](../prds-surface-syntax/README.md).

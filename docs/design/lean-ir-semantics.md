# Lean IR semantics charter

Status: **architecture and proof policy frozen; semantics not yet implemented**.

This charter is the maintained design entry point for the
[Lean IR foundational-formalization track](../prds-lean-ir-formalization/README.md).
It fixes the intended layering, semantic choices, module ownership, and proof
gate before semantic definitions land. Normative authority remains
[`DESIGN.md`](../../DESIGN.md) plus later explicit entries in
[`DECISIONS.md`](../../DECISIONS.md).

## Scope and assurance boundary

The track formalizes the current serialization-friendly V1 Lean IR through the
structural plan/export-data boundary. When the complete track is accepted, its
claim is a checked typed core, proved checking/building, and proof-complete
**deterministic pathwise tau-leap semantics** over mathematical values and an
explicit draw oracle.

It does not claim ideal CTMC/probability-law semantics, verified Lean macros,
byte-level JSON or hash-primitive proofs, Rust refinement, CPU/CUDA refinement,
binary64 refinement, or concrete Philox refinement. In particular, this PRD
adds only module umbrellas and an audit command: there is not yet a raw-to-
checked checker, evaluator, tick meaning, or trace meaning.

The architecture is:

```text
Lean commands/macros (trusted parser and diagnostic adapter)
  → pure frontend builders
  → raw V1 IR
  → decidable checker/resolver
  → checked typed core
  → pathwise semantics
      State × Inputs × DrawOracle → TraceOutcome
```

The existing serialization structures remain in
[`Sembla.IR`](../../frontend/Sembla/IR.lean). Pure builders will sit beneath the
existing macro surface in [`Sembla.DSL`](../../frontend/Sembla/DSL.lean).
Mathlib supplies mathematical reals, finite structures, and proof
infrastructure in the new checked/semantic layers; it does not replace the raw
wire-oriented structures.

The exhaustive raw declaration inventory, its independent role/checking-status
classifications, compile-time constructor/field guards and computed fixtures
are recorded in [`lean-ir-coverage.md`](lean-ir-coverage.md). That PRD 0002
evidence establishes coverage only; the checked and pathwise definitions named
below remain later deliverables.

## Distinct layers and failures

These categories must not be collapsed:

| Layer | Meaning and responsibility |
| --- | --- |
| Raw validity | Whether a serialization-friendly `Sembla.IR` model or plan satisfies the current V1 structural contract. Raw checking preserves the original semantic raw structure and does not canonicalize it. |
| Checked values | Resolved declarations and intrinsically typed terms for which names, schemas, and static sorts are total. Checked values are future track deliverables, not existing PRD 0001 definitions. |
| Dynamic semantic errors | Explicit failures that require supplied state, inputs, draws, or evaluated values, such as division by zero in the mathematical semantics, malformed supplied references, negative hazards, conflicting accepted writes, and invalid empty reductions. They are values in the future semantic result, not macro diagnostics. |
| Macro diagnostics | Positioned parsing, name-resolution, and authoring diagnostics produced by trusted syntax adapters. Tests establish their accepted/rejected boundary, but this track does not prove Lean macro expansion. Pure builders will own semantic construction beneath them. |
| Pathwise tau-leap meaning | Deterministic execution relative to `State`, tick-indexed `Inputs`, and a coordinate-addressed `DrawOracle`. Rates are frozen at tick start, effects read old state, accepted writes commit simultaneously, and observations inspect committed state. This is the executable semantic layer formalized by PRDs 0010–0017. |
| Ideal CTMC meaning | The continuous-time probability-law interpretation in [`DESIGN.md` §4.3](../../DESIGN.md#43-time-and-stochastics-hazard-rates-and-racing-clocks) and [`DECISIONS.md` §C3](../../DECISIONS.md#c3-racing-clocks-ctmc-as-ground-truth-semantics). It is deliberately deferred and is not replaced by the pathwise tau-leap meaning. |

A mathematical semantic choice is not automatically a claim about the current
Rust executor. For example, this charter makes division by zero an explicit
semantic error, while the current binary64 evaluator deliberately retains IEEE
infinity/NaN behavior in
[`eval.rs`](../../crates/sembla-runtime/src/eval.rs). Rust/binary64 refinement is
outside this track, so that known divergence is not silently presented as a
proved correspondence.

## Accepted architecture decisions

The following decisions are frozen for this track:

- Implement only the current V1 IR. Extension seams may be designed, but this
  track adds no overlay, stratification, lens, or conflict-completion
  constructors.
- Establish the checked semantic foundation before composition. No executable
  composition semantics or linker-preservation theorem belongs in this track.
- Use Mathlib deliberately for reals, finite structures, and proof support.
- Define deterministic pathwise behavior from a coordinate-addressed draw
  oracle. Ideal CTMC semantics remains a named later layer.
- Elaborate raw serialization-friendly structures into a checked/resolved typed
  core rather than assigning partial meaning directly to unchecked syntax.
- Put semantic construction in proved pure builders beneath thin trusted macro
  adapters.
- Represent dynamically invalid operations as explicit semantic errors.
- Land proof-complete increments. Deferred theorem families are future work,
  not opaque propositions in the trusted base.
- Prove structural plan/export-data correctness only through Lean-produced data;
  byte encoders, hash primitives, and downstream implementation refinement are
  excluded.

These choices reconcile the raw deep embedding described in
[`DESIGN.md` §4.5](../../DESIGN.md#45-meaning-the-lean-layer), the CTMC/tau-leap
split in [`DECISIONS.md` §§C3–C5](../../DECISIONS.md#c3-racing-clocks-ctmc-as-ground-truth-semantics),
and the accepted flat-plan boundary in
[`DECISIONS.md` §J](../../DECISIONS.md#j-composition-and-the-option-d-architecture-accepted-2026-07-21).
Changing one requires an explicit maintained decision and track amendment before
the affected PRD is enqueued.

## Frozen V1 semantic decisions

The foundational-contract column is copied from the binding track README.
Evidence identifies the present authority or implementation boundary; it does
not claim the future Lean semantics already exists.

| Area | Foundational contract | Current authority or evidence |
| --- | --- | --- |
| Scientific values | Interpret `coefficient × 10^exponent` exactly in `ℝ`. Numerically equal encodings have equal meaning; plan canonicalization remains a separate structural operation. | Raw `Scientific` syntax is in [`Sembla.IR`](../../frontend/Sembla/IR.lean); exact mathematical meaning begins in PRDs 0003/0010, while structural canonicalization belongs to PRD 0020. |
| Coercions | Checked numeric operations carry explicit `Int → Real` coercions. Division returns Real. Assignment requires exact destination type unless the checked term contains an explicit coercion. | Current raw scalar constructors and attribute types are in [`Sembla.IR`](../../frontend/Sembla/IR.lean). The checked coercion nodes and proofs are owned by PRDs 0004–0006. |
| `dt` | Raw checking requires exact `0 < dt`. Zero or negative `dt` is rejected statically. | `dt` is a raw model field in [`Sembla.IR`](../../frontend/Sembla/IR.lean); its semantic role is fixed by [`DESIGN.md` §4.3](../../DESIGN.md#43-time-and-stochastics-hazard-rates-and-racing-clocks) and [`DECISIONS.md` §C4](../../DECISIONS.md#c4-tau-leaping-as-the-executed-approximation). |
| References | Current V1 references are table/schema-matched in-range row ordinals in a supplied finite state. There is no generic active/retired-row feature. Out-of-range state data is an explicit semantic error. | Raw reference types are in [`Sembla.IR`](../../frontend/Sembla/IR.lean); finite checked rows and dynamic state validation belong to PRDs 0003, 0005, and 0011. |
| State cardinality | In validated V1 state, each table has exactly its declared `sizeHint` rows. `sizeHint` fixes finite execution shape but does not initialize row values. Unvalidated supplied state may carry a wrong row count; PRD 0011 owns validation and explicit state/reference errors. | The Lean frontend preserves authored `rows :=` as `Table.sizeHint`, while concrete value initialization remains a runtime concern. V1 state artifacts require matching row counts; PRD 0003 owns the valid dependent domain and PRD 0011 owns supplied-state validation. |
| Evaluation errors | Evaluate subterms left-to-right and return the first explicit error. Checked names/types are total; dynamic division by zero and invalid supplied state/reference data are errors. | The order and mathematical error behavior are a new Lean contract. The current Rust evaluator is syntax-tree ordered but intentionally uses binary64 division behavior in [`eval.rs`](../../crates/sembla-runtime/src/eval.rs); no refinement theorem is claimed. |
| Priors | Defaults must match parameter type. Integer parameters have no prior. Current prior metadata is checked structurally for family, exactly two exact arguments, and ordered Uniform bounds; it has no sampling denotation in this pathwise execution track. | Raw parameter/prior metadata is in [`Sembla.IR`](../../frontend/Sembla/IR.lean). Declaration checking and builder preservation belong to PRDs 0005 and 0007. |
| Empty reductions | `count = 0`, numeric `sum = 0`, and grouping an empty input yields no groups. Empty `min`, `max`, `last` and `argmaxTick` produce `emptyReduction` semantic errors. | Raw observation forms are in [`Sembla.IR`](../../frontend/Sembla/IR.lean). Aggregate, observation, summary, and trace behavior belongs to PRDs 0012, 0013, and 0017. |
| Hazards | Guards are evaluated before hazards. Zero hazard does not fire; negative dynamic hazard is an explicit error; positive hazard uses mathematical race time `-log(u)/h` for `0 < u < 1`; firing requires strict `raceTime < dt`. | Hazard-rate and frozen-rate semantics are normative in [`DESIGN.md` §4.3](../../DESIGN.md#43-time-and-stochastics-hazard-rates-and-racing-clocks) and [`DECISIONS.md` §§C2–C5](../../DECISIONS.md#c2-hazard-rates-not-per-tick-probabilities). PRD 0014 owns the Lean definition. |
| Draw identity | Coordinates are semantic identities `(tick, transition identity, row identity, draw index)`, not mutable stream positions or dense runtime ordinals. | Coordinate-addressed randomness is required by [`DESIGN.md` §4.2](../../DESIGN.md#42-state-transitions-and-aggregates) and the accepted Philox identity decision in [`DECISIONS.md` §J4](../../DECISIONS.md#j4-rng-strategy-doc-open-question-2-resolved). PRD 0014 owns the abstract oracle. |
| Contests | Lower race/key value wins; stable transition/row identity breaks ties. Claim-ordering domains must agree for a resource. Each resource chooses one winner independently; a candidate fires iff it wins every claim. Crossed multi-resource contests may defer all candidates. Losers commit no effects. | Resource conflict semantics are fixed by [`DESIGN.md` §5.1](../../DESIGN.md#51-resource-conflicts) and represented in the current IR/runtime boundary. PRD 0015 owns the pathwise winner relation. |
| Writes | Effects read the pre-tick snapshot. Accepted writes commit simultaneously. A conflicting accepted write set is an explicit semantic error rather than list-order resolution. | Uniform read-old/write-new is normative in [`DECISIONS.md` §C5](../../DECISIONS.md#c5-no-within-tick-cascades-uniform-one-tick-delay); the current staging order is visible in [`executor.rs`](../../crates/sembla-runtime/src/executor.rs). PRD 0016 owns the Lean tick. |
| Execution inputs | A finite run receives a tick-indexed provider `Nat → InputSnapshot`; non-composed callers may supply a constant provider. Each atomic tick consumes exactly its indexed snapshot. | This is the external semantic input boundary. It must not be confused with composition delivery, whose ordinary wires materialize the next tick's input under [`DESIGN.md` §4.4](../../DESIGN.md#44-composition-an-operad-with-tables-on-the-wires). |
| Observations | Outputs/views/grouped views are evaluated from the successfully committed state and are sinks: they do not mutate state or consult draws. If post-commit observation fails, the trace retains that committed state and all prior events, records the observation-phase error, emits no value for the failed observation, and terminates. Summaries fold completed observation values; an invalid empty summary retains the initial/last committed state under the same rule. | [`DESIGN.md` §4.6](../../DESIGN.md#46-observation-a-sink-never-a-feedback-path) fixes prefix noninterference: the transition/draw kernel and every common completed prefix are unchanged, while an explicit post-commit observation error may terminate trace production without rollback. The current executor also observes only after commit in [`executor.rs`](../../crates/sembla-runtime/src/executor.rs). |
| Raw checking | Raw-to-checked elaboration does not canonicalize. Erasing a successful checked model returns the original semantic raw structure exactly. Plan normalization is owned by PRDs 0019–0020. | The raw source is [`Sembla.IR`](../../frontend/Sembla/IR.lean). Exact erasure is proved in PRD 0006; plan validity and identity normalization remain separate in PRDs 0019–0020. |
| Composition handoff | Outputs materialize from post-commit state, wires deliver at the next tick, one output may fan out, each input has at most one source, an unwired input is an empty table, and feedback is allowed only through the one-tick delay. Later source/flat preservation uses the existing full pathwise observation fields and states boundary refactoring modulo an explicit identity bijection where literal identities change. | Uniform delay is normative in [`DESIGN.md` §4.4](../../DESIGN.md#44-composition-an-operad-with-tables-on-the-wires), and the full observation quotient is fixed by [`DECISIONS.md` §J13](../../DECISIONS.md#j13-observation-quotient-and-proof-obligations-2026-07). PRD 0021 records only the future handoff. |

## Exact module map for PRDs 0002–0021

This reviewed map is authoritative for the track's file ownership. It was
checked against every later PRD's allowed-file list; no mismatch was found, so
no numbered PRD requires amendment before 0002 is enqueued. “Umbrellas” means
`Sembla.Semantics`, `Sembla.Frontend.Builders` where applicable, and
`Sembla` exactly as listed by that PRD.

| PRD | Exact Lean module work |
| --- | --- |
| 0002 | Add `Sembla.Semantics.Raw`, `Sembla.Semantics.RawTests`; update semantic/root umbrellas. |
| 0003 | Add `Sembla.Semantics.Types`, `Sembla.Semantics.State`, `Sembla.Semantics.TypesTests`; update semantic/root umbrellas. |
| 0004 | Add `Sembla.Semantics.Syntax`, `Sembla.Semantics.SyntaxTests`; update semantic/root umbrellas. |
| 0005 | Add `Sembla.Semantics.CheckDeclarations`, `Sembla.Semantics.CheckDeclarationsTests`; update semantic/root umbrellas. |
| 0006 | Add `Sembla.Semantics.CheckTerms`, `Sembla.Semantics.CheckModel`, `Sembla.Semantics.CheckModelTests`; update semantic/root umbrellas. |
| 0007 | Add `Sembla.Frontend.Builders.Core`, `Sembla.Frontend.Builders.CoreTests`; update builder/root umbrellas. |
| 0008 | Add `Sembla.Frontend.Builders.Transition`, `Sembla.Frontend.Builders.TransitionTests`; update builder/root umbrellas. |
| 0009 | Add `Sembla.Frontend.Builders.Observation`, `Sembla.Frontend.Builders.ObservationTests`; update builder/root umbrellas and existing `Sembla.DSL`, `Sembla.CommandFrontendTests`. |
| 0010 | Add `Sembla.Semantics.Eval`, `Sembla.Semantics.EvalTests`; update semantic/root umbrellas. |
| 0011 | Add `Sembla.Semantics.StateEval`, `Sembla.Semantics.StateEvalTests`; update semantic/root umbrellas. |
| 0012 | Add `Sembla.Semantics.Aggregate`, `Sembla.Semantics.Output`, `Sembla.Semantics.AggregateTests`; update semantic/root umbrellas. |
| 0013 | Add `Sembla.Semantics.Observation`, `Sembla.Semantics.Summary`, `Sembla.Semantics.ObservationTests`; update semantic/root umbrellas. |
| 0014 | Add `Sembla.Semantics.Random`, `Sembla.Semantics.Candidate`, `Sembla.Semantics.CandidateTests`; update semantic/root umbrellas. |
| 0015 | Add `Sembla.Semantics.Contest`, `Sembla.Semantics.ContestTests`; update semantic/root umbrellas. |
| 0016 | Add `Sembla.Semantics.Effect`, `Sembla.Semantics.Tick`, `Sembla.Semantics.TickTests`; update semantic/root umbrellas. |
| 0017 | Add `Sembla.Semantics.Trace`, `Sembla.Semantics.Noninterference`, `Sembla.Semantics.TraceTests`; update semantic/root umbrellas. |
| 0018 | Add `Sembla.Semantics.Lumping`, `Sembla.Semantics.LumpingTests`; update existing `Sembla.LumpingProof` and semantic/root umbrellas. |
| 0019 | Add `Sembla.Semantics.PlanValidity`, `Sembla.Semantics.PlanValidityTests`; update existing `Sembla.PlanExport`, `Sembla.PlanTests`, and semantic/root umbrellas. |
| 0020 | Add `Sembla.Semantics.PlanIdentity`, `Sembla.Semantics.PlanIdentityTests`; update existing `Sembla.PlanExport`, `Sembla.PlanJson`, `Sembla.PlanTests`, and semantic/root umbrellas. |
| 0021 | Add `Sembla.Semantics.CoverageAudit`; update `Sembla.Semantics.ProofAudit` and semantic/root umbrellas. The composition handoff is documentation-only. |

The exact filesystem paths are the module names above under `frontend/` with
`.lean`, plus the existing files named in each row. Tests are deliberately in
the same covered module roots so the source and environment audits cannot omit
their named proofs.

## Proof and axiom policy

The covered roots are exactly:

- `Sembla.Semantics`;
- `Sembla.Frontend.Builders`.

Covered sources may contain no `sorry`, `admit`, explicit `axiom`,
`native_decide`, `unsafe`, `implemented_by`, or proposition hidden behind an
`opaque` declaration. Ordinary opaque data implementations are allowed only
when the environment audit classifies them as data-returning declarations.

Every named theorem or lemma in a covered namespace or covered imported module
is enumerated in deterministic name order. Its transitive `Lean.collectAxioms`
set must be a subset of exactly:

```text
{propext, Classical.choice, Quot.sound}
```

The deterministic repository command is:

```sh
bash frontend/scripts/check-proofs.sh
```

The script imports every source module under the frontend's `Sembla` module
tree. It then invokes
`#audit_proofs Sembla.Semantics Sembla.Frontend.Builders` from
[`Sembla.Semantics.ProofAudit`](../../frontend/Sembla/Semantics/ProofAudit.lean).
The command filters that complete environment by namespace ancestry and
imported-module ownership, so private generated names and covered namespaces
declared outside their canonical directories cannot escape. The script also
preserves the existing lumping/composition specification name checks.

Each run performs cleaned-up negative self-tests: all forbidden source forms
must be rejected; a theorem depending on a fresh unapproved axiom plus an opaque
proposition must fail with their names; and a temporary module outside the
canonical semantic directory must not hide a covered-namespace theorem.
Temporary sources and generated artifacts are removed by a shell trap.

The frontend pins Lean 4.13.0 in `lean-toolchain`, Mathlib `v4.13.0`, and the
resolved dependency revisions in `lake-manifest.json`. Later acceptance records
must cite the automated inventory rather than selected `#print axioms` output.
Theorem statements cannot be weakened without amending the owning PRD; computed
fixtures and canonical bytes remain evidence, not proofs.

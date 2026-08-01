# Lean IR foundational formalization PRDs

Status: **approved; PRD 0001 enqueued**. This track makes Sembla's current V1 Lean frontend and IR reason-able through proof-complete, pathwise tau-leap semantics. It ends at the Lean-produced structural plan/export-data boundary.

This README is binding on every numbered PRD.

## Queue policy

Do not run this directory directly. The only executable pending-work source is [`../prds-run-queue/README.md`](../prds-run-queue/README.md). After explicit approval, **move, do not copy**, only the next eligible numbered PRD into the run queue. Return an accepted PRD to this track as its implementation record. Never enqueue more than the dependency chain permits.

## Assurance claim

When all numbered PRDs are accepted, Lean will contain:

1. a complete inventory of the current serialization-friendly raw V1 IR and plan contract;
2. a resolved, intrinsically typed semantic core;
3. proved-sound and complete pure checking from raw models to that core;
4. proved-safe pure frontend builders beneath thin syntax elaborators;
5. proof-complete **pathwise tau-leap V1 semantics** for parameters, expressions, finite state, aggregates, observations, transitions, contests, effects, ticks, traces and explicit errors;
6. an IR-connected form of the existing grouped-count/lumping theorem (target 1b);
7. structural correctness theorems for Lean plan construction, stable identities, canonicalization and export data; and
8. a constructor-by-constructor proof/coverage audit plus a precise handoff charter for a later composition/model-algebra track.

The claim does **not** include ideal CTMC/probability-law semantics, Lean macro verification, byte-level JSON encoder proofs, hash-primitive proofs, Rust, CPU, CUDA, binary64 or concrete Philox behavior.

## Accepted architecture decisions

- Scope the implemented current V1 IR; design extension seams but do not add overlay, stratification, lens or conflict-completion constructors.
- Establish the foundation before composition. No executable composition PRD belongs in this track.
- Use Mathlib for mathematical reals, finite structures and proof infrastructure.
- Define deterministic pathwise behavior from an explicit coordinate-addressed draw oracle. Ideal CTMC semantics remains a named later layer and is not replaced by tau-leap semantics.
- Keep raw serialization-friendly structures and elaborate them into a checked/resolved typed core.
- Move semantic construction into pure functions beneath thin macros. Parsing, macro expansion and diagnostic presentation remain tested trusted adapters.
- Represent dynamically invalid operations as explicit semantic errors.
- Land proof-complete increments; later theorem families remain future work rather than opaque propositions in the trusted base.
- Prove structural plan/export-data correctness but not byte-level encoding or downstream implementation refinement.

Changing these decisions requires an explicit README/decision amendment before an affected PRD is enqueued.

## Frozen V1 semantic decision table

PRD 0001 must copy this table into the maintained semantic charter and reconcile citations, not choose different behavior silently.

| Area | Foundational contract |
| --- | --- |
| Scientific values | Interpret `coefficient × 10^exponent` exactly in `ℝ`. Numerically equal encodings have equal meaning; plan canonicalization remains a separate structural operation. |
| Coercions | Checked numeric operations carry explicit `Int → Real` coercions. Division returns Real. Assignment requires exact destination type unless the checked term contains an explicit coercion. |
| `dt` | Raw checking requires exact `0 < dt`. Zero or negative `dt` is rejected statically. |
| References | Current V1 references are table/schema-matched in-range row ordinals in a supplied finite state. There is no generic active/retired-row feature. Out-of-range state data is an explicit semantic error. |
| Evaluation errors | Evaluate subterms left-to-right and return the first explicit error. Checked names/types are total; dynamic division by zero and invalid supplied state/reference data are errors. |
| Priors | Defaults must match parameter type. Integer parameters have no prior. Current prior metadata is checked structurally for family, exactly two exact arguments, and ordered Uniform bounds; it has no sampling denotation in this pathwise execution track. |
| Empty reductions | `count = 0`, numeric `sum = 0`, and grouping an empty input yields no groups. Empty `min`, `max`, `last` and `argmaxTick` produce `emptyReduction` semantic errors. |
| Hazards | Guards are evaluated before hazards. Zero hazard does not fire; negative dynamic hazard is an explicit error; positive hazard uses mathematical race time `-log(u)/h` for `0 < u < 1`; firing requires strict `raceTime < dt`. |
| Draw identity | Coordinates are semantic identities `(tick, transition identity, row identity, draw index)`, not mutable stream positions or dense runtime ordinals. |
| Contests | Lower race/key value wins; stable transition/row identity breaks ties. Claim-ordering domains must agree for a resource. Each resource chooses one winner independently; a candidate fires iff it wins every claim. Crossed multi-resource contests may defer all candidates. Losers commit no effects. |
| Writes | Effects read the pre-tick snapshot. Accepted writes commit simultaneously. A conflicting accepted write set is an explicit semantic error rather than list-order resolution. |
| Execution inputs | A finite run receives a tick-indexed provider `Nat → InputSnapshot`; non-composed callers may supply a constant provider. Each atomic tick consumes exactly its indexed snapshot. |
| Observations | Outputs/views/grouped views are evaluated from the successfully committed state and are sinks: they do not mutate state or consult draws. If post-commit observation fails, the trace retains that committed state and all prior events, records the observation-phase error, emits no value for the failed observation, and terminates. Summaries fold completed observation values; an invalid empty summary retains the initial/last committed state under the same rule. |
| Raw checking | Raw-to-checked elaboration does not canonicalize. Erasing a successful checked model returns the original semantic raw structure exactly. Plan normalization is owned by PRDs 0019–0020. |
| Composition handoff | Outputs materialize from post-commit state, wires deliver at the next tick, one output may fan out, each input has at most one source, an unwired input is an empty table, and feedback is allowed only through the one-tick delay. Later source/flat preservation uses the existing full pathwise observation fields and states boundary refactoring modulo an explicit identity bijection where literal identities change. |

A conflict between this table and an accepted maintained decision is a stop condition for PRD 0001, not permission to guess.

## Representation architecture

```text
Lean commands/macros (trusted parser/diagnostic adapter)
  → pure frontend builders
  → raw V1 IR
  → decidable checker/resolver
  → checked typed core
  → pathwise semantics
      State × Inputs × DrawOracle → TraceOutcome
```

Mathlib belongs in the semantic layers. Existing raw structures, exact scientific literals and exporters remain separate.

## Proof policy

The following contract is mechanical and binding:

1. New track modules may contain no `sorry`, `admit`, explicit `axiom`, `native_decide`, `unsafe`, `implemented_by` or semantic proposition hidden behind `opaque`.
2. `#print axioms`/`Lean.collectAxioms` for every named theorem or lemma introduced under `Sembla.Semantics` or the verified frontend-builder namespaces may contain only `propext`, `Classical.choice` and `Quot.sound`. Fewer is preferred.
3. PRD 0001 must add an automated declaration inventory that enumerates the covered namespaces, checks the allowed transitive-axiom set, and fails if a theorem/lemma is omitted. Later acceptance records cite this command rather than hand-selecting favorable theorems.
4. Theorem statements may not be weakened during implementation without stopping for a PRD amendment.
5. Computed fixtures and canonical bytes are evidence, not proofs.

Ordinary opaque implementation definitions are allowed only when they hide no deferred proposition and the audit classifies them explicitly.

## Run order

All PRDs are serial and require every numerically earlier PRD to be accepted.

| # | Increment |
| --- | --- |
| [0001](../prds-run-queue/0001-semantic-charter-and-proof-policy.md) | semantic charter, Mathlib, module map and proof audit — enqueued |
| [0002](0002-raw-ir-and-plan-coverage.md) | raw IR/plan constructor coverage |
| [0003](0003-scalar-schema-and-state-domains.md) | scalar values, schemas and finite state domains |
| [0004](0004-typed-term-syntax.md) | typed expressions, aggregates, effects and claims |
| [0005](0005-declaration-and-reference-checking.md) | declaration/reference checking |
| [0006](0006-term-and-model-checking.md) | term/model checking, soundness, completeness and exact erasure |
| [0007](0007-core-frontend-builders.md) | parameter/table/model pure builders |
| [0008](0008-transition-frontend-builders.md) | transition/effect/contest pure builders |
| [0009](0009-observation-builders-and-macro-delegation.md) | observation builders and macro delegation |
| [0010](0010-scalar-expression-semantics.md) | scalar expression semantics |
| [0011](0011-finite-table-and-reference-semantics.md) | finite table and reference semantics |
| [0012](0012-aggregate-and-output-semantics.md) | aggregate and output-materialization semantics over abstract inputs |
| [0013](0013-observation-and-summary-semantics.md) | views, grouped views and summaries |
| [0014](0014-draw-oracle-and-candidate-semantics.md) | draw oracle and transition-candidate semantics |
| [0015](0015-claim-and-contest-semantics.md) | claims and deterministic contest winners |
| [0016](0016-effects-and-atomic-tick-semantics.md) | effects, simultaneous commit and atomic tick |
| [0017](0017-traces-errors-and-noninterference.md) | traces, errors and observation noninterference |
| [0018](0018-bind-lumping-proof-to-ir.md) | bind the grouped-count/lumping theorem to the IR evaluator |
| [0019](0019-plan-validity-and-construction.md) | plan well-formedness and constructor soundness |
| [0020](0020-identities-canonicalization-and-export-data.md) | stable identities, canonicalization and structural export data |
| [0021](0021-final-audit-and-composition-handoff.md) | final coverage/proof audit and composition follow-on charter |

## Global implementation rules

- PRD 0001 freezes exact module paths. Each later PRD already lists exact intended files; changing that list requires amending the PRD before implementation edits the extra path.
- Preserve existing public syntax and V1 export fixtures unless an explicit compatibility decision is approved.
- Diagnostic categories and accepted/rejected boundaries matter; exact wording and precedence do not, except existing positioned tests must remain compatible.
- A later PRD may use definitions and theorems from earlier modules but may edit only its allowed files. If an earlier theorem is insufficient, stop and amend scope rather than weakening encapsulation opportunistically.
- No numbered PRD may introduce composition source semantics or linker-preservation theorems. PRD 0021 only records their prerequisites and exact future questions.

## Required checks

Every implementation PRD must pass its focused checks plus:

```bash
cd frontend && lake build
bash frontend/scripts/check-proofs.sh
./scripts/check.sh
```

Acceptance records include focused fixture results and the automated axiom inventory. Environmental inability to run the complete checks prevents acceptance.

## Stop conditions

Stop for a decision if work reveals a current V1 constructor without an assigned meaning/invariant, a conflict with an accepted decision, a public schema/syntax change, a need for another dependency, a theorem requiring runtime behavior as normative semantics, or pressure to add composition/model-algebra behavior before PRD 0021.

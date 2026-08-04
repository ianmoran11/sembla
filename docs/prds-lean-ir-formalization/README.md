# Lean IR foundational formalization PRDs

Status: **approved; PRDs 0001–0008 accepted; PRD 0009 enqueued**. This track makes Sembla's current V1 Lean frontend and IR reason-able through proof-complete, pathwise tau-leap semantics. It ends at the Lean-produced structural plan/export-data boundary.

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
- PRD 0006 owns static resolved checked declarations for outputs, ordinary views,
  grouped views and summaries. PRDs 0012–0013 retain their values, traversal,
  materialization, fold behavior and executable denotation.
- PRD 0006 preserves raw wires exactly without validating them. PRD 0019 owns
  endpoint, direction, schema and fan-in structural validity; executable delivery
  remains outside this foundational checker.
- PRD 0007's pure core builders cover every PRD 0005-valid form in their owned
  prior/parameter/attribute/table/model-shell fragment, including all three prior
  families. Existing macros retain their current narrower syntax until PRD 0009
  delegates to the builders; pure API coverage does not itself add syntax.
- PRD 0008 consumes `CoreBoxShell`/`CoreModelShell` as the sole ordered owner of
  core declarations and attaches transitions by existing box source ordinal. Its
  pure raw/checker path covers the complete PRD 0006-valid transition fragment,
  including key ordering, while the current surface-producing path remains
  race-time-only.
- PRD 0009 owns one pure complete model-local assembly boundary by embedding
  one `TransitionOverlaySpec` and adding inputs and observations by the existing
  core-box ordinals. It assembles the complete input-bearing raw candidate before
  obtaining authoritative declaration/term contexts, and certifies observation
  semantics only through the public final `checkModel` boundary. It preserves raw
  wires without proving their validity; macros remain trusted parsing,
  token-position and diagnostic adapters rather than independent semantic
  constructors.
- Semantic failures are composed, not extended after acceptance: PRD 0010 owns
  scalar `EvalError`, each later semantic phase owns its local closed error type,
  and combining APIs preserve those exact errors through explicit wrapper sums.
- Source-order traversal fixes cross-box, transition, row, claim and effect
  evaluation/error precedence. Stable semantic identities, rather than source
  ordinals, govern draw coordinates and contest ties. Permutation theorems compare
  identity-indexed successful outcomes; they do not silently equate differently
  ordered provenance/error traces.

Changing these decisions requires an explicit README/decision amendment before an affected PRD is enqueued.

## Frozen V1 semantic decision table

PRD 0001 must copy this table into the maintained semantic charter and reconcile citations, not choose different behavior silently.

| Area | Foundational contract |
| --- | --- |
| Scientific values | Interpret `coefficient × 10^exponent` exactly in `ℝ`. Numerically equal encodings have equal meaning; plan canonicalization remains a separate structural operation. |
| Coercions | Checked numeric operations carry explicit `Int → Real` coercions. Division returns Real. Assignment requires exact destination type unless the checked term contains an explicit coercion. |
| `dt` | Raw checking requires exact `0 < dt`. Zero or negative `dt` is rejected statically. |
| References | Current V1 references are table/schema-matched in-range row ordinals in a supplied finite state. There is no generic active/retired-row feature. Out-of-range state data is an explicit semantic error. |
| State cardinality | In validated V1 state, each table has exactly its declared `sizeHint` rows. `sizeHint` fixes finite execution shape but does not initialize row values. Unvalidated supplied state may carry a wrong row count; PRD 0011 owns validation and explicit state/reference errors. |
| Evaluation errors | Evaluate subterms left-to-right and return the first explicit error. Across boxes, transitions, rows, claims and effects, traverse preserved source order and return the first explicit error. Checked names/types are total; dynamic division by zero and invalid supplied state/reference data are errors. Semantic phases use closed local error types composed through exact wrapper sums rather than extending an accepted error inductive. |
| Priors | Defaults must match parameter type. Integer parameters have no prior. Current prior metadata is checked structurally for family, exactly two exact arguments, and ordered Uniform bounds; it has no sampling denotation in this pathwise execution track. |
| Empty reductions | `count = 0`, numeric `sum = 0`, and grouping an empty input yields no groups. Empty `min`, `max`, `last` and `argmaxTick` produce `emptyReduction` semantic errors. Summary `sum` therefore returns the correctly typed zero even though the current Rust executor rejects all empty summary inputs; this track makes no Rust-refinement claim. |
| Hazards | Guards are evaluated before hazards. Static checking requires a Real hazard but does not inspect its sign. Zero hazard does not fire; negative dynamic hazard, including a negative literal, is an explicit error; positive hazard uses mathematical race time `-log(u)/h` for `0 < u < 1`; firing requires strict `raceTime < dt`. |
| Draw identity | Coordinates are semantic identities `(tick, transition identity, row identity, draw index)`, not mutable stream positions, source ordinals or dense runtime ordinals. Model-local transition identity is the declaration-stable pair `(box name, transition name)`, corresponding to the existing encoded plan identity `occurrence(box.name) ++ "#" ++ transition.name`; row identity retains its table owner and typed `RowId`. PRD 0020 proves the structural export correspondence. |
| Contests | Lower race/key value wins; stable transition/row identity breaks ties. Static checking validates each claim and retains its Real/Int/owner-indexed-enum ordering domain but does not require all possible claims to agree. PRD 0015 requires exact domain equality only among actual claimants for one evaluated resource: race-time and Real-key claims are compatible, Int is distinct from Real, and enum keys are compatible only in the same retained owner-indexed enum domain. Each resource chooses one winner independently; a candidate fires iff it wins every claim. Crossed multi-resource contests may defer all candidates. Losers commit no effects. |
| Writes | Effects read the pre-tick snapshot. Raw checking requires every Ref-destination effect to have a claim whose resource expression is structurally equal to that effect's raw RHS; duplicate claim resources within one transition are rejected. Accepted writes commit simultaneously. A conflicting accepted write set is an explicit semantic error rather than list-order resolution. |
| Execution inputs | An `InputSnapshot` supplies exactly one finite table for each checked `InputId`, with exactly its declared schema and arbitrary finite row cardinality. A finite run receives a tick-indexed provider `Nat → InputSnapshot`; non-composed callers may supply a constant provider. Each atomic tick consumes exactly its indexed snapshot. |
| Observations | PRD 0006 statically resolves and types output/view/grouped-view/summary declarations; PRDs 0012–0013 own their values and meaning. Each checked output materializes exactly one row in checked/source field order with its exact declared schema. Grouped views are count-only: key tuples follow declaration order, Enum/Ref keys use their retained ordinal identities, negative integer bands use Euclidean division, emitted groups are lexicographically ordered, and zero-count groups are omitted. Summaries target ordinary scalar views only and consume strictly increasing absolute-tick-labeled histories; `argmaxTick` returns the least maximizing absolute tick as Int, while other reducers retain the view's numeric sort. Outputs/views/grouped views are evaluated from the successfully committed state and are sinks: they do not mutate state or accept an oracle/draw argument. If post-commit observation fails, the trace retains that committed state and all prior events, records the observation-phase error, emits no value for the failed observation, and terminates. Observation noninterference is therefore equality through a horizon only when both observation sets succeed; a failing observer has the same identity-indexed behavioral/committed-state prefix through its failure tick, not unavailable future events. Summaries fold completed observation values; an invalid empty summary retains the initial/last committed state under the same rule. |
| Raw checking | Raw-to-checked elaboration does not canonicalize. Erasing a successful checked model returns the original semantic raw structure exactly. PRD 0006 excludes wire validity and preserves the raw wire list unchanged; PRD 0019 owns structural wire validation. Plan normalization is owned by PRDs 0019–0020. |
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
| [0001](0001-semantic-charter-and-proof-policy.md) | semantic charter, Mathlib, module map and proof audit — accepted in `e95570f` |
| [0002](0002-raw-ir-and-plan-coverage.md) | raw IR/plan constructor and field coverage — accepted in `837360e` |
| [0003](0003-scalar-schema-and-state-domains.md) | scalar values, schemas and finite state domains — accepted in `38a1ef1` |
| [0004](0004-typed-term-syntax.md) | typed expressions, aggregates, effects and claims — accepted in `503957b` |
| [0005](0005-declaration-and-reference-checking.md) | declaration/reference checking — accepted in `8f22eb1` |
| [0006](0006-term-and-model-checking.md) | term/model checking, soundness, completeness and exact erasure — accepted in `1ec3210` |
| [0007](0007-core-frontend-builders.md) | parameter/table/model pure builders — accepted in `16d7d93` |
| [0008](0008-transition-frontend-builders.md) | transition/effect/contest pure builders — accepted in `462ef2c` |
| [0009](../prds-run-queue/0001-observation-builders-and-macro-delegation.md) | observation builders and macro delegation — enqueued |
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

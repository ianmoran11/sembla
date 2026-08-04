# PRD 0017: Define finite traces, errors and observation noninterference

## Dependencies

PRDs 0001–0016 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

A tick has meaning. Models need finite execution traces, preserved failure prefixes and precise observation projections before IR-connected rewrite theorems can be stated.

## Goal

Lift tick semantics to deterministic finite pathwise traces and prove observation sinks cannot affect behavior.

## Requirements

1. Define finite execution requests with initial checked state, parameters, a
   tick-indexed input provider `Nat → InputSnapshot`, draw oracle, absolute start
   tick and tick count. A constant provider is the ordinary non-composed case.
2. Define trace events for consulted coordinates; candidates with stable identity
   and source provenance; complete evaluated contest results including resource,
   retained ordering domain/value, winner and accepted/deferred disposition;
   accepted firings; source-provenance writes; committed states; post-commit
   observations; and exact phase-local errors.
3. Apply the README's phase rule exactly: absolute tick `n` consumes input
   `inputs n`; observations run only after successful commit; an
   observation-phase error retains the newly committed state and prior events but
   emits no failed value. Every pre-commit failure—including scalar guard/hazard,
   negative-hazard, `invalidDraw`, claim/domain, effect-evaluation and write
   conflict errors—retains only the last committed prefix. Zero-tick invalid
   summary reduction retains the initial state.
4. Define named full-pathwise, identity-indexed behavioral, state-trace and
   user-observation projections. Ordered full traces retain source provenance;
   permutation/noninterference claims compare the identity-indexed projection so
   representation order is not mistaken for behavioral change.
5. Prove fixed-provider replay determinism, successful prefix/append laws,
   phase-sensitive error-prefix stability and state-step locality. Prefix/append
   must preserve an absolute tick offset: appending after a prefix of length `k`
   consumes `inputs (start + k + n)` and the same absolute draw coordinates,
   never a suffix restarted at tick zero.
6. Prove qualified observation noninterference. When both observation sets
   evaluate successfully through the compared horizon, adding/removing
   output/view/grouped-view/summary declarations cannot change consulted
   coordinates, candidates, evaluated contests/winners, accepted/deferred
   dispositions, commits or states. If an added/changed observer fails, prove the
   same identity-indexed behavioral and committed-state prefix through that
   tick, after which only the failing observation run terminates; do not claim
   equality of unavailable future behavior.
7. Add success, no-fire, repeated-fire and observation-heavy fixtures, plus
   mid-run scalar guard/hazard, negative-hazard, invalid-draw, claim-key
   evaluation, incompatible-domain, effect-evaluation and conflicting-write
   failures proving the exact pre-commit prefix behavior, and an
   observation-failure fixture proving only common-prefix noninterference.

## Allowed files

- `frontend/Sembla/Semantics/Trace.lean`
- `frontend/Sembla/Semantics/Noninterference.lean`
- `frontend/Sembla/Semantics/TraceTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-semantics.md`
- `docs/design/lean-ir-coverage.md`

## Non-goals

- Infinite/probabilistic traces.
- Wiring/composition behavior.

## Test and proof guidance

Every equivalence theorem must name its projection. Noninterference includes consulted coordinate sets/traces, not only final state.

## Acceptance criteria

1. Success/error traces and all four named projections are executable
   definitions.
2. Replay, prefix, error, locality and noninterference theorems pass the audit.
3. Fixtures exercise each outcome/projection.
4. Build, proof hygiene and full checks pass.

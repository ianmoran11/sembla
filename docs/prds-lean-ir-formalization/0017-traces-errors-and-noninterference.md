# PRD 0017: Define finite traces, errors and observation noninterference

## Dependencies

PRDs 0001–0016 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

A tick has meaning. Models need finite execution traces, preserved failure prefixes and precise observation projections before IR-connected rewrite theorems can be stated.

## Goal

Lift tick semantics to deterministic finite pathwise traces and prove observation sinks cannot affect behavior.

## Requirements

1. Define finite execution requests with initial checked state, parameters, a tick-indexed input provider `Nat → InputSnapshot`, draw oracle and tick count. A constant provider is the ordinary non-composed case.
2. Define trace events for consulted coordinates, candidates, per-resource winners, accepted firings, writes, committed states, post-commit observations and errors.
3. Apply the README's phase rule exactly: tick `n` consumes input `inputs n`; observations run only after successful commit; an observation-phase error retains the newly committed state and prior events but emits no failed value; pre-commit errors retain only the last committed prefix. Zero-tick invalid summary reduction retains the initial state.
4. Define named full-pathwise, state-trace and user-observation projections.
5. Prove fixed-provider replay determinism, successful prefix/append laws, phase-sensitive error-prefix stability and state-step locality.
6. Prove adding/removing output/view/grouped-view/summary declarations cannot change consulted coordinates, candidates, winners, commits or states.
7. Add success, no-fire, repeated-fire, observation-heavy and mid-run-error fixtures.

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

1. Success/error traces and all three projections are executable definitions.
2. Replay, prefix, error, locality and noninterference theorems pass the audit.
3. Fixtures exercise each outcome/projection.
4. Build, proof hygiene and full checks pass.

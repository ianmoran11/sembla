# PRD 0005 implementation notes — attempt 1

## Rust validator division of labor

The surface checks only the facts available from the selected table declaration:
the contest attribute exists, is `Ref`-typed, and is not duplicated in the same
transition. It emits only `ClaimOrdering.raceTime`; keyed orderings remain hidden
under DECISIONS §K7.

The existing Rust validator remains authoritative for deeper IR-wide contest
rules. In `crates/sembla-ir/src/validate.rs:515-563`, `validate_transition`:

- infers every resource expression against the transition table and requires a
  `Ref` result;
- canonicalizes resource expressions to detect duplicate claims; and
- typechecks an IR-authored keyed ordering and requires an orderable key.

In `crates/sembla-ir/src/validate.rs:565-584`, it additionally requires a Ref
write's value expression to have a matching claim. PRD 0005 does not reproduce
or weaken those checks in Lean and does not lift the surface Ref-write rejection.

## Runtime conflict and diagnostic evidence

`crates/sembla-runtime/src/executor.rs:673-724` evaluates each claim's Ref column
from the old snapshot and maps `RaceTime` to the candidate's exponential race
time. `resolve_claims` (`executor.rs:953-1042`) groups candidates by resource
row, selects the existing deterministic argmin winner, requires a multi-claim
candidate to win every resource, and counts losers by resource table. The
runtime then exposes those counts through `TickReport.deferred_per_resource_table`;
`run` applies the existing strict saturation threshold and produces structured
warnings (`executor.rs:255-299`). Finally, `detect_double_writes`
(`executor.rs:1129-1173`) rejects unresolved same-cell writes before commit and
names both transitions.

The end-to-end fixture uses a canonical Lean export and creates its identity-Ref
initializer through the `sembla.state/v1` writer, then exercises the state-artifact
loader through the CLI. Its claimless counterexample clears only the exported
IR claims and reaches the unchanged runtime `DoubleWrite` defense.

## Validation

- `./scripts/check.sh`
- `cd frontend && bash scripts/test-negative.sh`
- `bash frontend/scripts/check-parity.sh`
- `cargo test --locked -p sembla-cli --test contest`
- explicit ignored canonical Lean fixture-regeneration check
- temporary-index `git diff --cached --check`

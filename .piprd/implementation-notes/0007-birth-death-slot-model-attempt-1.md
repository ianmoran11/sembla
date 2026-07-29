# PRD 0007 implementation notes — attempt 1

## Model and interpretation

`frontend/Sembla/Models/DemographicSlots.lean` authors the complete
`demographic_slots` model through the surface DSL. The fixed `PersonSlot`
rows implement DECISIONS §K1 identity as `(slot, generation)`; birth activation
is the only generation increment, and the golden run reaches generation 2 from
an initial maximum of 1. All entries/exits require a clear marker, marker
clearing requires a non-clear marker, and the three death age bands are
disjoint. Deaths claim each slot's one-to-one `slot_resource` by `race_time`.

Empty table schemas are supported, so `SlotResource` has no placeholder
attribute. `Area.area_key` remains the requested explicit Int column.

The default engineering rates are monthly hazards 0.025 per eligible vacant
birth slot and 0.001/0.003/0.012 for young/adult/old mortality. They are
non-scientific test values with LogNormal priors. Seed 7007 produces enough
births, deaths, and slot reuse to pin the accounting behavior in 24 ticks.
Per DECISIONS §K10, the birth hazard is not interpreted as fertility.

## Deterministic fixtures and goldens

The Rust integration test synthesizes the state artifact with plain arithmetic:
4,000 present rows, 1,000 vacant rows, alternating sex, round-robin area,
arithmetic ages in 0–1080, one-to-one slot resources, and generations 1/0.
The normal test regenerates temporary bytes and compares them; the ignored
`regenerate_demographic_state_fixture` test is the only explicit writer.

Lean exports canonical model and direct-stable plan fixtures under
`fixtures/demographic/`. The appended parity section re-exports both, compares
exact bytes, and Rust-validates the plan. The 24-tick run records scalar CSV,
summary CSV, both grouped CSVs, a normalized manifest, and stdout hash records.
The ignored golden-regeneration test and normal run-twice test use the same
fixed contract.

At the golden seed the final population is 4,105, with 548 births, 443 deaths,
maximum invalid-age count 0, final maximum generation 2, and 548 locked-out
row-ticks. Thus the measured lockout equals births both per tick and in total.

## Acceptance coverage

`crates/sembla-cli/tests/demographic_slots.rs` contains eight explicit golden
invariant tests: stock-flow accounting, marker-summary accounting, invalid
ages, single-exit bounds, grouped/scalar totals, strict generation reuse,
lockout measurement, and bitwise reproduction twice. It also checks canonical
model/plan/state fixtures and chains two deterministic 12-tick windows with
changed mortality theta, including manifest state-link hash equality.

Documentation in `docs/demographic-model.md` records the state machine, fixed
slot identity, exact one-tick lockout, measured golden magnitude, verbatim §K10
caveats, run command, and grouped artifacts. `docs/state-format.md` links it.

## Validation

Passed:

- `cargo test --locked -p sembla-cli --test demographic_slots` (10 passed,
  2 explicit regeneration tests ignored)
- both explicit ignored regeneration tests; SHA-256 comparison confirmed the
  state artifact and all six golden files remained byte-identical
- `frontend/scripts/check-parity.sh`, including exact demographic model/plan
  export comparison and Rust plan validation
- `./scripts/check.sh`, including Rust formatting/Clippy/workspace tests, Lean
  build and proof hygiene, negative suite, parity, documentation, dependency,
  and lock checks
- `git diff --check`

No dependency or framework changes were made. `Cargo.lock`, `DESIGN.md`,
`examples/**`, pre-existing state goldens, and pre-existing plan fixtures are
unchanged.

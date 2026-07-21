# PRD 0008: Linker wires — delayed channels, mailbox identity, twin plans

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. The
linker (PRD 0007) handles product; this PRD adds ordinary wires: exact-schema
directional channels with exactly one tick of delay, one mailbox per wire.
The runtime already implements delayed wire delivery for flat models
(`examples/two_box.json`, `sir_policy.json`) — **no runtime semantics change
is needed or permitted**; the linker lowers source wires to the existing flat
`Model.wires`, and the identity map names their mailboxes.

Semantics restated (architecture doc §8/§13.3, DECISIONS §J3):

- A wire connects one instance's output port to one instance's input port
  within the same composite. Direction and exact ordered schema equality are
  validated. An input has at most one driver. Unwired inputs remain legal
  (they read empty tables, matching current runtime behavior).
- Wire occurrence identity: `<owner-occurrence>#wire:<wire-slug>` — a wire
  inside a composite instantiated twice gets two occurrences.
- Mailbox identity:
  `mbox:<wire-occurrence>|<source-occurrence>.port:<port>|<target-occurrence>.port:<port>`.
- In V1 (until PRD 0009) wires may reference only **direct child primitive
  instances** of the owning composite; wiring to a composite child's
  boundary requires exposures (PRD 0009).

## Goal

`epidemic_policy` links to a two-leaf, two-mailbox plan that validates, runs,
and exhibits the exact one-tick/two-tick delay discipline; a flat
`direct_stable` twin produces the same plan semantic hash as the linked plan;
deterministic wire validation errors are pinned.

## Specification

### 1. Relax the construct gate for wires only

In `Link.lean`, stage 2 (PRD 0007) stops rejecting `wires`; `exposures` and
`hiddenPorts` still yield `unsupportedConstruct` naming PRD 0009.

### 2. Wire resolution and validation (new linker stage)

For each composite under expansion, for each `WireDeclV1`:

1. Resolve `sourceInstance`/`targetInstance` to direct children (parser
   already guarantees the ids exist in the composite; here resolve to the
   expanded child). If a referenced child is a composite → `missingPort`
   with a message stating boundary wiring requires exposures (PRD 0009).
2. Resolve `sourcePort` on the source child's definition ports: must exist
   (`missingPort`) and have direction `output` (`directionMismatch`).
   Symmetrically `targetPort` must be an input.
3. Schema check: source port schema and target port schema must be equal as
   ordered field lists — name and type both (`schemaMismatch`; message lists
   both schemas).
4. Single driver: at most one wire (across the whole expanded plan) may
   target a given `(leaf occurrence, input port)` (`multipleDrivers`, both
   wire ids in `related`). Fan-out from one output is legal.
5. Lower to a flat `IR.Wire`: endpoints `(box := <leaf chain>,
   port := <port name>)` using the leaf box names from expansion. Wires are
   emitted for every expansion of the owning composite — `two_regions`-style
   fixtures produce one wire set per region with distinct identities.

### 3. Mailbox identities in the identity map

For every lowered wire, add a `MailboxIdentityV1` entry with wire occurrence
`occ:<owner-chain>#wire:<slug>` (owner chain of the composite expansion that
owns the wire declaration; root-owned wires use `occ:`), and the endpoint
occurrences/ports per the frozen format. Sort by identity. The Rust plan
validator (PRD 0003 §4.8) checks mailboxes↔wires bijection; extend its check
to accept linked wire occurrences (`occ:<chain>#wire:<slug>` format) as well
as the `direct_stable` synthesized form — both formats are now legal, each
tied to its origin: `direct_stable` plans must use the synthesized form,
`linked` plans the declared form.

### 4. Fixtures, goldens, delay tests

- **Linked goldens.** Link `epidemic_policy` → check in
  `fixtures/plans/linked/epidemic_policy.plan.json` (two leaves
  `population`, `policy`; exactly two mailboxes). Extend the parity section.
- **Ping/pong delay fixtures.** Add to `Fixtures.lean` (and export/check in
  as source + linked plan):
  - `def:ping` — primitive; single-row table `sender` with enum attr
    `phase : {Idle, Fire}` (initial variant first, matching the controller
    pattern in `examples/sir_policy.json`); transitions
    `arm : phase Idle → Fire` and `disarm : phase Fire → Idle`, both with
    hazard `1e300` (deterministic firing; guards disjoint so no conflict);
    output port `port:pulse` schema `[value : Int]` counting
    `phase = Fire`.
  - `def:pong` — primitive; single-row table `receiver` with
    `seen : {No, Yes}`; input `port:pulse` `[value : Int]`; transition
    `notice` guarded on `inputSum pulse field value > 0 ∧ seen = No`,
    hazard `1e300`, effect `seen := Yes`; view `seen_yes :=
    count receiver where seen = Yes`.
  - `def:ping_pong` — composite wiring `ping.pulse -> pong.pulse`
    (`wire:pulse`).
- **Delay trace test (mechanical).** Run the linked `ping_pong` plan for 4
  ticks at any fixed seed; assert from the CSV that `seen_yes` is `0` at
  tick 0 and `1` from tick 1 onward — the output built from tick-0 committed
  state arrives at tick 1; this pins "exactly one tick of delay". For
  `epidemic_policy`, assert the two mailbox identities in the plan are
  distinct and that a run at fixed seed produces a nonzero
  `fired:policy.restrict` no earlier than tick 1 (count reaches policy after
  one tick; the restriction returns to population one tick later — assert
  restriction-dependent behavior cannot appear at tick 0/1 by checking the
  policy controller's mode view flips no earlier than tick 1).
- **Twin plans (architecture doc §15.3).** Build a flat Lean model twin:
  an `IR.Model` value (in a test module, not `Models.lean`) whose boxes are
  named `population`/`policy` with bodies identical to the linked leaves
  (post-parameter-rewrite) and the two wires. Export it with
  `directStablePlan` (PRD 0005) and `#guard` that its **plan semantic hash**
  equals the linked plan's semantic hash… noting one caveat: mailbox wire
  occurrences differ between `direct_stable` (synthesized) and `linked`
  (declared) forms, and mailboxes are inside the semantic hash. Resolve as
  follows, and record it in the implementation notes: the twin test compares
  the canonical bytes of `{model}` plus the `transitions`/`leaves`/
  `scheduler_domains` sections of the identity map (i.e. everything except
  `mailboxes`) — define a helper hash for exactly that comparison in the
  test. The full semantic hashes are *expected to differ only in the mailbox
  wire-occurrence text*; assert that too (mailbox entries agree on all
  endpoint fields).
- **Error tests** (extend `LinkTests.lean`), each pinning sorted codes:
  wire to missing port; output→output (`directionMismatch`);
  input→input; schema field-name mismatch; schema type mismatch; two wires
  into one input (`multipleDrivers`); wire naming a composite child
  (`missingPort` with the exposure message).

### 5. Rust-side runs

Extend the walking validate test to the new linked plans. Add a CLI run
golden for `linked/epidemic_policy.plan.json` (fixed seed/ticks, CSV +
hashes) and for the ping/pong delay assertion (the delay test can live in
`crates/sembla-cli/tests/` reading the CSV it produces, or as a Lean-free
Rust test — either way it must assert the tick-1 property mechanically).

## Allowed files

- `frontend/Sembla/Composition/Link.lean`, `Errors.lean` (messages),
  `Fixtures.lean`, `LinkTests.lean`, `SourceTests.lean` (new fixtures'
  round-trips)
- `frontend/Sembla.lean`, `frontend/scripts/check-parity.sh` (append only)
- `fixtures/composition-source/**`, `fixtures/plans/linked/**`,
  `fixtures/plans/goldens/**`
- `crates/sembla-ir/src/plan.rs`/`identity.rs` (mailbox-format acceptance
  per origin), `crates/sembla-ir/tests/**`, `crates/sembla-cli/tests/**`
- implementation notes/artifacts created by the managed run

## Non-goals

- Exposure, hiding, boundary wiring, nested-composite wiring (PRD 0009).
- Merge components, fan-in reduction, adapters, zero-delay anything
  (deferred; `delay_ticks != 1` is already rejected at parse).
- Runtime/mailbox execution changes — the flat runtime is already correct.
- Changing the plan schema or hash payload definitions.

## Acceptance criteria

1. `lake build` + extended parity pass; `epidemic_policy` and `ping_pong`
   linked goldens byte-reproduce from checked-in sources.
2. All linked plans validate in Rust including canonicality; the
   per-origin mailbox-format rule is enforced both ways (negative fixture
   for each mismatch).
3. The ping/pong CSV proves the one-tick delay (`seen_yes`: 0 at tick 0,
   1 at tick 1) via a mechanical test.
4. The twin test passes: flat `direct_stable` twin and linked plan agree
   bitwise on model + non-mailbox identity sections, and mailbox entries
   agree on all endpoint fields.
5. Every wire error case produces its pinned deterministic code list;
   exposures still reject with `unsupportedConstruct` naming PRD 0009.
6. `two_regions` is **not** linked in this PRD (its `epidemic_policy_exposed`
   definition uses exposures) — confirm it still rejects deterministically.
7. `./scripts/check.sh` and `git diff --check` pass; no legacy artifact
   changed.

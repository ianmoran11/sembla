# PRD 0005: Contest declarations in the surface DSL

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K7 binds:
`race_time` ordering only; keyed orderings (queue disciplines) remain v0.5
scope and must not be exposed.

Verified facts:

- The conflict machinery is v0.1 core (DECISIONS §E3): `ResourceClaim
  { resource: Expr, ordering: ClaimOrdering }` with
  `ClaimOrdering::{RaceTime, Key{expr}}` exists in both IRs
  (`crates/sembla-ir/src/model.rs:326-336`, `frontend/Sembla/IR.lean:90-96`
  `raceTime`), and the runtime resolves claims by argmin with the
  lexicographic tie-break.
- The surface deliberately parses and rejects `contest`:
  `syntax "contest" ident` exists at box- and model-item level with
  unsupported-diagnostics (`frontend/Sembla/DSL.lean:331,351,945,976`).
- Ref-attribute writes are rejected in the surface *because* they require
  claims (`DSL.lean:1397`) — this PRD does not lift that; it only adds
  explicit claims on Ref-valued resources read from the row.

The demographic model's competing exits (death vs overseas departure vs
internal departure racing for one `slot_resource`) are the driving use:
all three transitions claim the same one-to-one resource so at most one
wins per slot per tick, losers defer, and the saturation diagnostic
applies. This resolves DESIGN.md §10.1 for the race-time case.

## Goal

`contest <ref-attr> by race_time` is accepted in transition bodies (list
kernel and command frontend), lowers to the existing
`ResourceClaim`/`RaceTime` IR through the single semantic kernel, is
proven equivalent to hand-written IR by an exact twin, exercised
end-to-end by a competing-exits runtime test, and fully covered by
negative diagnostics — with the box/model-level stubs and all legacy
bytes unchanged in meaning.

## Specification

### 1. Syntax and elaboration

- **Grammar.** Inside a `transition … where` body (alongside `guard`,
  `hazard`, `set`):

  ```lean
  contest <ident> by race_time
  ```

  `<ident>` resolves against the transition's selected table and must be a
  declared `Ref`-typed attribute (the one-to-one resource pattern:
  `slot_resource : SlotResource`). `by race_time` is mandatory — `contest
  x` without it is a positioned error naming the required ordering, and
  any other ordering identifier is a positioned error stating that keyed
  orderings are not yet supported (name §K7 in the message text or the
  error explanation, matching house diagnostic style).
- **Lowering.** Exactly:
  `ResourceClaim.mk (Expr.selfAttr "<runtime-attr-name>")
  ClaimOrdering.raceTime` appended to the transition's `contests` list in
  declaration order. Route through the existing kernel structures — no
  second transition builder.
- **Multiplicity.** Multiple `contest` lines per transition are allowed
  (the IR takes a list); an exact duplicate (same attribute) is a
  positioned error. Reaction-arrow transitions do not accept `contest`
  (they encode single-guard/single-effect sugar only — the general form
  exists for this); a `contest` attached to an arrow form is a positioned
  error telling the author to use the general transition form.
- **Existing stubs.** The box- and model-level `contest` stubs keep
  rejecting, but update their message to point at the transition-level
  form (`contest is declared inside a transition body`). Their negative
  expectations update accordingly (this is a permitted diagnostic-text
  improvement, not a position change).
- **Validation placement.** Surface-level checks: attribute exists, is
  Ref-typed, no duplicates. Deeper coverage rules (claims covering
  writes, overlap analysis) remain where they live today — the Rust
  validator and runtime double-write defense; do not duplicate them in
  Lean and do not weaken them. Read the Rust validator's existing
  contest-related checks first and record in the implementation notes
  what it enforces, so the division of labor is explicit.

### 2. Exact-IR twin

An imported Lean test with a `#guard`: a surface transition carrying
`contest slot_resource by race_time` elaborates to IR byte-identical to a
hand-constructed `Sembla.IR` model whose transition has the
`ResourceClaim` — the same twin pattern every surface construct uses.

### 3. Competing-exits runtime test (end to end)

Author a minimal competing-exit model in the new surface (test scale, in a
Lean test/fixture module — this is a framework test, not yet the
demographic canonical model):

- `Slot` table (e.g. 100 rows): `occupancy : {present, vacant}`,
  `cause : {none, a, b}`, `slot_resource : SlotResource`;
  `SlotResource` table 100 rows; slot_resource[i] = i in the initial
  state (use PRD 0002's state-artifact loader or the numeric initializer
  if it can express identity refs — read `initialize_population` first;
  if it cannot, the state-artifact fixture path is the way).
- Two transitions `exit_a`, `exit_b`, both guarded `occupancy = present`,
  both claiming `contest slot_resource by race_time`, with different
  hazards, setting `occupancy := vacant` and `cause := a|b` respectively.
- Export, run at fixed seed, and assert from views: (1)
  `count(cause = a) + count(cause = b) + count(occupancy = present) =
  rows` at every tick (no double-exit — a slot exits exactly once by
  exactly one cause); (2) both causes occur at the chosen seed
  (the race is real); (3) the run is bitwise reproducible; (4) with the
  claims removed (hand-IR variant), the same model would double-write —
  assert the runtime's double-write error fires for the claimless variant
  (this pins *why* the construct matters).
- Also assert the deferred-loser counter surfaces where the runtime
  already reports it (read the saturation-diagnostic plumbing; assert
  its value rather than adding new reporting).

### 4. Negative suite

Exact-position cases: unknown attribute; non-Ref attribute
(`contest age_months by race_time`); missing `by race_time`; unknown
ordering (`by priority`); duplicate contest; contest on a reaction arrow;
contest at box level and model level (updated messages). Use the
complete-set helper.

### 5. Documentation

Update the surface authority documentation with the construct, its
race-time-only status, the §K7 pointer, and one sentence on the
division of validation labor (surface checks names/types; runtime owns
conflict resolution and double-write defense).

## Allowed files

- `frontend/Sembla/DSL.lean`
- Lean test modules under `frontend/Sembla/`, `frontend/Sembla.lean`
- `frontend/Negative/**`, `frontend/scripts/test-negative.sh` (additions
  plus the two updated stub expectations only)
- `fixtures/state/**` (the competing-exits initial state, if §3 needs it)
- `crates/sembla-cli/tests/**` or `crates/sembla-runtime/tests/**` (the
  end-to-end and double-write assertions)
- surface authority documentation file(s)
- implementation notes/artifacts created by the managed run

## Non-goals

- Keyed orderings, top-k capacity, queue disciplines (v0.5).
- Lifting the surface Ref-write rejection (`DSL.lean:1397` behavior
  unchanged; §K9's Ref-reassignment deferral stands).
- New runtime conflict semantics, diagnostics, or validator rules.
- The demographic canonical model itself (PRD 0007).

## Acceptance criteria

1. Full check battery + negative suite + parity pass; all canonical
   exports byte-identical.
2. The exact-IR twin `#guard` holds; review confirms lowering goes
   through the single kernel.
3. The competing-exits runtime test passes all four assertions, including
   the claimless double-write counterexample.
4. Every negative case fails at the exact position with the expected
   message; box/model stub messages updated and covered.
5. The implementation notes record the Rust validator's existing contest
   enforcement (division-of-labor evidence).
6. `git diff --check` passes; no IR, runtime-semantics, or dependency
   changes.

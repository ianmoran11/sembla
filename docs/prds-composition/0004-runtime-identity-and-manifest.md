# PRD 0004: Plan-supplied rule words drive the runtime; manifest identity tuple

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. PRD 0003
landed the validated `ExecutablePlanV1` envelope. Today the runtime's Philox
coordinate and conflict tie-breaks use the dense declaration-order `rule_id`
that `validate()` assigns, and `crates/sembla-runtime/src/executor.rs` also
uses that same number as an array index (e.g. `fired[*rule_id as usize]`).
That conflation — one number serving as compact ordinal, RNG word, and
tie-break key — is exactly what DECISIONS §J4 separates:

- the **ordinal** stays dense and internal (array indexing, report ordering);
- the **rule word** is the `u32` fed to Philox and used in tie-breaks. For
  legacy models it equals the ordinal (bit-identical behavior, proven by the
  frozen goldens); for versioned plans it comes from the plan's validated
  identity map.

This is the PRD that makes the headline property real: because versioned rule
words are content-addressed from occurrence identities, inserting an
unrelated sibling box never changes another box's draws.

## Goal

The executor takes its RNG word and tie-break key from a per-rule `rule_word`
distinct from the dense ordinal; `sembla run` executes plan envelopes; run
manifests record the frozen plan-identity tuple; legacy behavior is
byte-identical; a sibling-insertion invariance test locks the property in.

## Specification

### 1. Split ordinal from rule word in `sembla-ir`

In `crates/sembla-ir/src/validate.rs`, extend the validated rule record with
`pub rule_word: u32` alongside the existing dense id. For the legacy path set
`rule_word = rule_id` (the dense value) — nothing else changes. Give
`ValidatedPlan` (from PRD 0003) a method producing a `ValidatedModel` whose
rules carry the plan's words: match plan `identity.transitions` entries to
dense rules by `(box, name)` (both sides are already validated bijections).

### 2. Executor uses the word for RNG and tie-breaks only

In `crates/sembla-runtime`, audit **every** use of the dense rule id and
classify it:

- Philox key material (`rng.rs` call sites building
  `[tick, rule_id, entity_id, draw_idx]`) → use `rule_word`.
- Conflict/argmin tie-break keys `(time, rule_id, entity_id)` → use
  `rule_word`.
- Array indexing (`fired` accumulation, per-rule buffers), diagnostics, and
  error messages (`EntityIdOverflow { rule_id, … }`, double-write reports) →
  keep the dense ordinal.

Record the complete classified list of call sites in the implementation
notes; the review must be able to check it against `grep -n rule_id`. Because
legacy words equal ordinals, every legacy fixture, CSV golden, state hash,
output hash, and the CPU/CUDA differential corpus must pass **unchanged** —
that is the no-drift proof for this refactor. Any golden diff means the split
leaked into semantics; fix the code, never the golden.

Reserved namespaces: `SWEEP_REPLICA_RULE_ID`/prior-draw words remain reserved;
plan validation (PRD 0003) already rejects colliding words, and the runtime
must not re-map anything.

### 3. `sembla run` accepts plan envelopes

In `crates/sembla-cli/src/main.rs`, route `run` through `parse_input`:

- Legacy model file → exactly today's path.
- Plan envelope → `validate_plan` + canonicality check (as in `validate`),
  then execute the derived `ValidatedModel`-with-words on the CPU backend.
  `--backend cuda` with a plan file: allowed only if it needs no code beyond
  passing the words through the existing differential-tested path; otherwise
  reject with `plan envelopes run on the cpu backend only for now` and note
  it in the implementation notes. `sweep`, `verify-run`, `compare`,
  `diff-backends` keep their PRD 0003 deterministic rejection.

### 4. Run-manifest identity tuple

In `crates/sembla-cli/src/manifest.rs`, add optional append-only fields to
`RunManifest` (serde `skip_serializing_if = "Option::is_none"`; readers reject
partial tuples per DESIGN.md §5.4 rule 3):

```rust
pub struct PlanIdentityTuple {           // all-present-or-all-absent
    pub plan_schema: String,             // "sembla.executable-plan/v1"
    pub identity_scheme: String,         // "sembla.identity/stable-v1"
    pub origin: String,                  // "linked" | "direct_stable"
    pub plan_semantic_hash: HashRecordV1,
    pub enabled_features: Vec<String>,   // sorted, [] in V1
}
pub struct LinkedSourceTuple {           // all-present-or-all-absent
    pub source_hash: HashRecordV1,
    pub linker_semantics: String,        // "sembla.linker/v1"
}
```

Plan runs always write `plan: Some(PlanIdentityTuple)`; `linked_source` is
`Some` iff the plan's origin is `linked` (copy `source_hash` and
`linker.semantics` out of the plan's `linked_provenance` — never fabricate).
Legacy runs write neither field, and a byte-comparison test proves a legacy
run's manifest is identical to pre-PRD output. Manifest reading
(`verify-run`'s parser) must reject a manifest where a tuple is partially
present.

### 5. Golden plan run and sibling-insertion invariance

Create `fixtures/plans/goldens/`:

1. **Plan run golden.** Run `fixtures/plans/two_box.plan.json` at a fixed
   seed/ticks (pick seed 55, ticks 40) and check in the CSV, final state
   hash, output hash, and manifest (minus environment-dependent fields —
   follow the normalization the existing manifest tests use). A CLI test
   re-runs and compares. Note: these numbers intentionally differ from a
   legacy run of `examples/two_box.json` — the identity scheme differs; that
   is the design, record it in the test's comment.
2. **Sibling-insertion invariance.** Build (with the PRD 0003 regeneration
   helper) `fixtures/plans/two_box_plus_sibling.plan.json`: same model plus
   one additional unwired box (copy a simple box, e.g. a one-row
   controller-style table with one transition, renamed `bystander`) inserted
   so that it sorts **before** the existing boxes. A runtime/CLI test runs
   both plans at the same seed and asserts, for every shared box: identical
   per-tick view columns, identical per-tick `fired:` columns, and identical
   deferred counts. This is the test that fails on declaration-order identity
   and passes on content-addressed identity.
3. **Determinism.** Run the plan golden twice in-process and assert bitwise
   identical CSV bytes and hashes (Level A on the same binary).

### 6. Documentation touch

Update the CLI usage/help text for `validate`, `run`, `plan-hash` to mention
plan envelopes. No other doc changes in this PRD.

## Allowed files

- `crates/sembla-ir/src/validate.rs`, `plan.rs`, `lib.rs`
- `crates/sembla-runtime/src/**` (word/ordinal split only — no semantic
  change to sampling, conflict resolution, effects, or state)
- `crates/sembla-cli/src/main.rs`, `manifest.rs`
- `crates/sembla-cli/tests/**`, `crates/sembla-runtime/tests/**`,
  `crates/sembla-ir/tests/**`
- `fixtures/plans/**` (new fixtures/goldens), `Cargo.lock`
- implementation notes/artifacts created by the managed run

## Non-goals

- Any Lean change (PRD 0005).
- Linked plans, mailbox-identity runtime use, or source maps (PRDs 0007–0009).
- CUDA feature work beyond §3's conditional pass-through.
- Changing sweep seeds, prior draws, Philox internals, `exp_f64`, or any
  numeric path.
- Regenerating any existing golden. If one fails, the implementation is
  wrong.

## Acceptance criteria

1. Full `./scripts/check.sh` passes with **zero** changes to existing goldens,
   `examples/**`, CSV fixtures, or hash expectations — the legacy
   byte-identity proof.
2. The implementation notes contain the classified list of every former
   `rule_id` use site (word vs ordinal), and code review confirms Philox and
   tie-breaks use `rule_word` while indexing/diagnostics use the ordinal.
3. `sembla run fixtures/plans/two_box.plan.json --seed 55 --ticks 40 …`
   reproduces the checked-in golden CSV and hashes bitwise, twice.
4. The sibling-insertion test passes: shared-box view and fired traces are
   bitwise identical between `two_box.plan.json` and
   `two_box_plus_sibling.plan.json`.
5. A plan-run manifest contains the complete `plan` tuple (frozen strings,
   semantic hash record, sorted empty `enabled_features`) and no
   `linked_source`; a legacy-run manifest is byte-identical to pre-PRD
   output; partial-tuple manifests are rejected by the reader with a test.
6. `git diff --check` passes; no new dependencies.

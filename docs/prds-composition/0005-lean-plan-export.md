# PRD 0005: Lean plan envelope emission and cross-language canonical parity

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. Rust now
validates and runs `ExecutablePlanV1` envelopes (PRDs 0003–0004). The Lean
frontend still exports only legacy unversioned model JSON via `sembla-export`
(`frontend/Main.lean`, `frontend/Sembla/Json.lean`). This PRD teaches Lean to
emit `direct_stable` plan envelopes for the existing canonical models, in
canonical bytes that Rust accepts **byte-for-byte** — establishing the
cross-language canonical-encoding contract that the linker (PRDs 0007–0009)
will rely on. Legacy exports remain byte-frozen.

The hard part is canonical JSON parity. Rust canonicalizes through
`serde_json::Value` (sorted keys, compact, ryu float printing). Lean must emit
the same bytes directly. The existing `frontend/Sembla/Json.lean` writer emits
fixed declaration-order keys and must not change; this PRD adds a **separate**
canonical writer.

## Goal

`sembla-export --plan <model> <out.json>` writes a canonical `direct_stable`
plan envelope for every registered canonical model; checked-in plan fixtures
are byte-identical between Lean export and Rust canonicalization; number and
hash parity are pinned by tests on both sides.

## Specification

### 1. Plan types — `frontend/Sembla/Plan.lean`

Mirror the Rust types from PRD 0003 §1 as Lean structures (`ExecutablePlanV1`,
`PlanOrigin`, `IdentityMapV1`, `SchedulerDomainV1`, `LeafIdentityV1`,
`TransitionIdentityV1`, `MailboxIdentityV1`, `LinkedProvenanceV1`,
`HashRecordV1`, `LinkerDescriptorV1`), with the frozen version strings as
named constants:

```lean
namespace Sembla.Plan
def planSchema : String := "sembla.executable-plan/v1"
def stableIdentityScheme : String := "sembla.identity/stable-v1"
-- etc. for every string in the README table
```

`LinkedProvenanceV1.sourceMap` may be a placeholder type for now (PRD 0007
defines it); `direct_stable` envelopes never populate it.

### 2. Canonical JSON writer — `frontend/Sembla/PlanJson.lean`

Introduce a tiny canonical-value layer rather than string-concatenating each
structure ad hoc:

```lean
inductive CJson where
  | str (s : String) | num (n : IR.Scientific) | int (n : Int)
  | bool (b : Bool) | arr (xs : Array CJson) | obj (fields : Array (String × CJson))

def CJson.render : CJson → String
```

`render` must implement `sembla.canonical-json/v1` exactly (README): compact
(no whitespace), object keys emitted in byte-wise sorted order (**sort in
`render`**, do not trust construction order), strings escaped exactly as
serde_json does (`"` `\` `\b \t \n \f \r`, other control chars as `\u00xx`
lowercase hex, everything else literal UTF-8), no trailing newline. Numbers:
integers in plain decimal; `Scientific` values must render to the same text
serde_json/ryu produces for the equivalent f64 — reuse/adapt the existing
Scientific printing from `frontend/Sembla/Json.lean`, and prove parity with
the vector test in §5 rather than assuming it.

Then write encoders `ExecutablePlanV1 → CJson` and — the bulk of the work —
`IR.Model → CJson` with **canonical array order** (this is a new ordering,
different from the legacy writer): boxes sorted by name; within each box,
tables/transitions/inputs/outputs/views sorted by name; params and summaries
sorted by name; wires sorted by their mailbox identity string. Field names
must match the Rust serde names exactly (`box`, `from`, `to`, `dt`, etc. —
read `crates/sembla-ir/src/model.rs` and the checked-in
`fixtures/plans/two_box.plan.json` to confirm every field spelling; the
byte-parity test will catch any mismatch).

### 3. Direct-stable identity derivation — `frontend/Sembla/PlanExport.lean`

```lean
def directStablePlan (model : IR.Model) : Except String Plan.ExecutablePlanV1
```

- Reject (with a clear message) any model whose name, box names, transition
  names, port names are not slugs (`[a-z][a-z0-9_]*`) — all current canonical
  models pass.
- `model_id := "model:" ++ model.name`; leaves `occ:<box>`; transition
  identities `occ:<box>#<name>`; rule words via `Sembla.Hash.ruleWord`
  (PRD 0002); reject reserved or duplicate words (deterministic error, no
  reassignment).
- One scheduler domain `domain:global` / `tau_leap` over all boxes, sorted.
- Mailboxes from `model.wires` with the synthesized wire occurrence
  `occ:#wire:to_<target_box>_<target_port>` — byte-compatible with Rust's
  `mailbox_identity` (PRD 0003 §2).
- Sort every identity array exactly as PRD 0003 §4 requires.

### 4. Export mode and registry

Extend `frontend/Main.lean`: `sembla-export --plan <name> <out.json>` accepts
every model name/alias the legacy exporter accepts, emits
`(directStablePlan model)` rendered canonically. Legacy invocation
(`sembla-export <name> <out.json>`) must remain byte-identical — the parity
harness proves it.

### 5. Cross-language parity fixtures and tests

1. **Number vectors.** Create `fixtures/hash/number-vectors.json` listing the
   numeric literals used across the canonical models and composition fixtures
   (at minimum: `0.25`, `0.1`, `0.8`, `1.0`, `2.0`, `0.4`, `1e300`, `1e-9`,
   `500.0`, `1000000.0` — extend with every distinct numeric that actually
   appears in the exported plans) with their expected canonical text. A Rust
   test asserts `serde_json::to_string` of the parsed f64 equals the expected
   text; a Lean `#guard` asserts the Scientific renderer produces the same
   text. If any value disagrees, either fix the Lean renderer or (if the
   value is only in a fixture you control) substitute a safe value and record
   why — never special-case the Rust side.
2. **Plan golden twins.** Export plans for `sir`, `sir_policy`, and
   `observations` into `fixtures/plans/` (e.g. `sir.plan.json`). These are
   the checked-in goldens. A new appended section in
   `frontend/scripts/check-parity.sh` re-exports each with `--plan` to a temp
   dir and `cmp`s against the checked-in fixture (append only — do not touch
   any existing line of the script). On the Rust side, extend
   `crates/sembla-cli/tests/validate.rs` with a test that walks every
   `fixtures/plans/*.plan.json`, validates it, and checks canonicality
   (parse → re-encode → byte equality). This pair of tests is the
   cross-language contract: Lean writes, Rust re-derives the same bytes.
3. **Hash parity.** For `sir.plan.json`, pin the semantic-hash digest as a
   literal in both a Lean `#guard` (hash the rendered bytes of the §3 value
   via `Sembla.Hash`) and the existing Rust `plan_golden.rs` style test.
   One constant, two languages.
4. **Runs.** Add a CLI test running `fixtures/plans/sir.plan.json` for a few
   ticks at a fixed seed with a checked-in CSV golden (same conventions as
   PRD 0004 §5).

### 6. Registration

Import new modules from `frontend/Sembla.lean`. Keep `two_box.plan.json` (the
PRD 0003 Rust-generated fixture) as-is if Lean has no registered two_box
model; the Lean goldens are the three models named above. If the Lean export
of any of them differs from what Rust's regeneration would produce, the Rust
canonicality test in §5.2 settles who is wrong: bytes must round-trip through
`serde_json` unchanged.

## Allowed files

- `frontend/Sembla/Plan.lean`, `PlanJson.lean`, `PlanExport.lean` (new),
  plus a `PlanTests.lean` test module
- `frontend/Main.lean`, `frontend/Sembla.lean`
- `frontend/scripts/check-parity.sh` (append a clearly-delimited plan
  section only)
- `fixtures/plans/**`, `fixtures/hash/number-vectors.json`
- `crates/sembla-cli/tests/validate.rs`, `crates/sembla-ir/tests/**`
  (parity/canonicality tests only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Changing `frontend/Sembla/Json.lean`, legacy export bytes, or
  `examples/**`.
- Composition source, linker, or surface syntax (PRDs 0006+).
- Emitting `linked` plans or source maps.
- A general-purpose Lean JSON pretty-printer or parser (parsing arrives in
  PRD 0006 via `Lean.Json`).

## Acceptance criteria

1. `cd frontend && lake build` and the extended
   `bash frontend/scripts/check-parity.sh` pass: legacy exports byte-frozen
   **and** the three plan exports `cmp`-identical to `fixtures/plans/`.
2. `cargo test --workspace` passes, including: canonicality round-trip over
   every `fixtures/plans/*.plan.json`; number-vector parity; the shared
   semantic-hash constant for `sir.plan.json`.
3. The same digest literal for `sir.plan.json`'s semantic hash appears in
   both a Lean `#guard` and a Rust test (grep-verifiable).
4. `sembla run fixtures/plans/sir.plan.json …` reproduces its checked-in CSV
   golden bitwise.
5. `directStablePlan` rejects non-slug names, reserved words, and duplicate
   words with deterministic errors, each covered by a Lean test.
6. `./scripts/check.sh` and `git diff --check` pass; no existing script line,
   golden, or example changed.

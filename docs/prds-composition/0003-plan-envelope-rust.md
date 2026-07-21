# PRD 0003: Versioned `ExecutablePlanV1` envelope and validation in Rust

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. Today
`crates/sembla-ir` parses one unversioned flat `Model` JSON shape and
`validate()` assigns dense declaration-order `u32` rule IDs
(`crates/sembla-ir/src/validate.rs`). DECISIONS §J (PRD 0001) introduces a
versioned plan envelope whose identity map carries content-addressed rule
words. This PRD adds the envelope, its canonical encoding, and its validation
to Rust, plus an explicit legacy branch, **without changing any runtime
behavior** — the executor starts consuming plan identities in PRD 0004.

Restated envelope design (normative here):

- A **versioned plan file** is a JSON object whose top level has key
  `schema_version`. An **unversioned legacy model file** (everything in
  `examples/`) has no such key and keeps today's parse/validate/run path
  byte-for-byte.
- The envelope wraps the existing `Model` unchanged under `model`, plus an
  `identity` block and optional `linked_provenance`.
- Canonical bytes follow `sembla.canonical-json/v1` (README): sorted keys, no
  whitespace, omitted optionals, canonical array order.

## Goal

`sembla-ir` parses, validates, and canonically re-encodes `ExecutablePlanV1`
envelopes; `sembla-cli validate` auto-detects envelope versus legacy input; a
new `plan-hash` command prints frozen hash records; golden and negative
fixtures pin all of it.

## Specification

### 1. Envelope types — `crates/sembla-ir/src/plan.rs`

Define serde types (all `#[serde(deny_unknown_fields)]`):

```rust
pub struct ExecutablePlanV1 {
    pub schema_version: String,      // must be "sembla.executable-plan/v1"
    pub identity_scheme: String,     // must be "sembla.identity/stable-v1"
    pub origin: PlanOrigin,          // "linked" | "direct_stable"
    pub model: Model,                // the existing flat model, unchanged shape
    pub identity: IdentityMapV1,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_provenance: Option<LinkedProvenanceV1>,
}

pub enum PlanOrigin { Linked, DirectStable }   // serde rename lowercase snake

pub struct IdentityMapV1 {
    pub model_id: String,                       // "model:" + model.name
    pub enabled_features: Vec<String>,          // must be []
    pub scheduler_domains: Vec<SchedulerDomainV1>,
    pub leaves: Vec<LeafIdentityV1>,            // sorted by box
    pub transitions: Vec<TransitionIdentityV1>, // sorted by identity
    pub mailboxes: Vec<MailboxIdentityV1>,      // sorted by identity
}

pub struct SchedulerDomainV1 {   // exactly one in V1
    pub id: String,              // "domain:global"
    pub algorithm: String,       // "tau_leap"
    pub leaves: Vec<String>,     // all box names, sorted
}

pub struct LeafIdentityV1 { pub r#box: String, pub occurrence: String }

pub struct TransitionIdentityV1 {
    pub r#box: String,
    pub name: String,        // transition name within the box
    pub identity: String,    // occurrence ++ "#" ++ name
    pub rule_word: u32,
}

pub struct MailboxIdentityV1 {
    pub identity: String,
    pub source_box: String, pub source_port: String,
    pub target_box: String, pub target_port: String,
}

pub struct LinkedProvenanceV1 {
    pub source_hash: HashRecordV1,   // {algorithm, domain, digest}
    pub linker: LinkerDescriptorV1,
    pub source_map: serde_json::Value, // opaque here; schema owned by Lean (PRD 0007+)
}

pub struct HashRecordV1 { pub algorithm: String, pub domain: String, pub digest: String }

pub struct LinkerDescriptorV1 {
    pub semantics: String,          // "sembla.linker/v1"
    pub source_schema: String,      // "sembla.composition-source/v1"
    pub plan_schema: String,        // "sembla.executable-plan/v1"
    pub identity_scheme: String,    // "sembla.identity/stable-v1"
    pub canonical_encoding: String, // "sembla.canonical-json/v1"
    pub source_map_schema: String,  // "sembla.source-map/v1"
}
```

JSON field names are snake_case exactly as written (`box` via `r#box` →
`"box"`). Every "must be" above is a validation rule, not a parse rule: parse
accepts any string, validation rejects wrong values deterministically so the
error names the offending version string (README: unknown versions are
rejected, never best-effort).

### 2. Identity derivation — `crates/sembla-ir/src/identity.rs`

Add `sha2 = "0.10"` to `crates/sembla-ir/Cargo.toml`. Implement:

```rust
pub const RULE_WORD_DOMAIN: &str = "sembla.rule-word/v1";
pub const RESERVED_RULE_WORDS: [u32; 2] = [u32::MAX - 1, u32::MAX];

/// SHA-256(domain ++ 0x00 ++ payload)
pub fn domain_digest(domain: &str, payload: &[u8]) -> [u8; 32];
pub fn rule_word(identity: &str) -> u32;    // first 4 bytes big-endian
pub fn is_reserved_rule_word(w: u32) -> bool;
/// "occ:" ++ leaf, "#" join, "mbox:" formats — helpers used by validation
pub fn occurrence_of_leaf(box_name: &str) -> String;
pub fn transition_identity(occurrence: &str, name: &str) -> String;
pub fn mailbox_identity(wire_occ: &str, sb: &str, sp: &str, tb: &str, tp: &str) -> String;
```

Mailbox identity format (frozen in README/J3):
`mbox:<wire-occurrence>|<source-occurrence>.port:<source_port>|<target-occurrence>.port:<target_port>`.
For `direct_stable` plans the synthesized wire occurrence is
`occ:#wire:to_<target_box>_<target_port>` (unique because each input has at
most one driver). Slug validation helper: `pub fn is_slug(s: &str) -> bool`
for `[a-z][a-z0-9_]*`.

Add a test that reads `fixtures/hash/vectors.json` (created in PRD 0002) and
asserts every `rule_words` entry matches `rule_word(identity)` — this is the
cross-language freeze.

### 3. Canonical JSON — `crates/sembla-ir/src/canonical_json.rs`

```rust
/// Serialize any Serialize value through serde_json::Value (whose object map
/// is a BTreeMap, i.e. keys already sort) into compact canonical bytes.
pub fn to_canonical_string<T: serde::Serialize>(value: &T) -> Result<String, String>;
```

Confirm in a unit test that `serde_json` was **not** compiled with
`preserve_order` (serialize a struct with unsorted field names and assert
sorted output). Canonical bytes are `to_canonical_string` output with no
trailing newline. Plan **semantic** payload is the canonical bytes of the JSON
object `{"identity": …, "identity_scheme": …, "model": …, "schema_version": …}`
(origin and provenance excluded — DECISIONS §J10); plan **envelope** payload
is canonical bytes of the whole envelope. Expose:

```rust
pub fn plan_semantic_hash(plan: &ExecutablePlanV1) -> Result<HashRecordV1, String>; // domain "sembla.plan-core/v1"
pub fn plan_envelope_hash(plan: &ExecutablePlanV1) -> Result<HashRecordV1, String>; // domain "sembla.plan-envelope/v1"
```

Both use `domain_digest` and lowercase hex.

### 4. Plan validation — extend `crates/sembla-ir/src/validate.rs` or a new `plan.rs` fn

`pub fn validate_plan(plan: &ExecutablePlanV1) -> Result<ValidatedPlan, ValidationError>`
must check, in a deterministic order with deterministic messages:

1. `schema_version`, `identity_scheme` equal the frozen strings; `origin` is a
   known value (parse enforces).
2. `identity.enabled_features == []`; any entry → error naming the feature.
3. The embedded `model` passes the existing `validate()` unchanged.
4. `identity.model_id == format!("model:{}", model.name)` and `model.name` is
   a slug.
5. `leaves` is a bijection with `model.boxes`: same set of box names, sorted
   by `box`, each `occurrence` equal to `"occ:" ++ box` where every
   `/`-separated segment of `box` is a slug.
6. `transitions` is a bijection with the model's (box, transition-name) pairs,
   sorted by `identity`; each `identity` equals
   `transition_identity(occurrence_of_leaf(box), name)`; each `rule_word`
   equals `rule_word(identity)` (recompute and compare — the map is
   defense-in-depth, not trusted input); no word is reserved; all words are
   pairwise distinct.
7. `scheduler_domains` is exactly one entry: id `domain:global`, algorithm
   `tau_leap`, `leaves` equal to the sorted box-name list.
8. `mailboxes` is a bijection with `model.wires`, sorted by `identity`; each
   entry's endpoints match one wire and its `identity` equals the frozen
   format from §2.
9. `linked_provenance` is present iff `origin == linked` (absent for
   `direct_stable`); when present, every descriptor string equals its frozen
   value and `source_hash.algorithm == "sha256"`,
   `source_hash.domain == "sembla.source-artifact/v1"`.
10. Canonical-order checks on the file's arrays: `model.boxes` sorted by name,
    each box's `transitions`/`views`/`inputs`/`outputs`/`tables` sorted by
    name, `model.params` and `model.summaries` sorted by name, `model.wires`
    sorted by their mailbox identity. (Legacy models are exempt — these rules
    bind only inside envelopes.)
11. Duplicate-word defense: implement the pairwise-distinct check as its own
    unit-testable function so a test can feed it a synthetic colliding map
    directly (a real SHA-256 prefix collision is not constructible).

`ValidatedPlan` wraps the existing `ValidatedModel` plus the identity map in
whatever minimal form PRD 0004 will need (e.g.
`pub words_by_dense_rule_id: Vec<u32>` aligned with the existing dense
ordering). Do **not** change `ValidatedModel` itself or any executor code in
this PRD.

### 5. Entry-point dispatch and CLI

In `sembla-ir`'s public API add:

```rust
pub enum ParsedInput { LegacyModel(Model), Plan(ExecutablePlanV1) }
pub fn parse_input(bytes: &str) -> Result<ParsedInput, …>
```

Dispatch rule: parse to `serde_json::Value`; object with `schema_version` key
→ envelope path (unknown value → error listing the one supported version);
otherwise legacy `Model` path, byte-identical behavior to today.

In `crates/sembla-cli/src/main.rs`:

- `sembla validate <file>` uses `parse_input`; for plans it runs
  `validate_plan` **plus** a canonicality check: re-encode the parsed
  `serde_json::Value` with `to_canonical_string` and require byte equality
  with the input file (error: `plan file is not canonical`).
- New command `sembla plan-hash <file>` validates, then prints exactly two
  lines:

  ```text
  semantic sha256 sembla.plan-core/v1 <digest>
  envelope sha256 sembla.plan-envelope/v1 <digest>
  ```

- `run`, `sweep`, `verify-run`, `compare`, `diff-backends` given an envelope
  file must fail with a clear deterministic error (`plan envelopes are not
  yet runnable; see PRD 0004`) — no silent legacy fallback.

### 6. Fixtures and tests

Create `fixtures/plans/two_box.plan.json`: a `direct_stable` envelope wrapping
the model from `examples/two_box.json` (canonicalized per §4.10 — boxes,
transitions, views sorted; the legacy example file itself is untouched).
Generate it with an `#[ignore]`d regeneration test
(`cargo test -p sembla-ir --test plan_golden regenerate -- --ignored`)
that parses the legacy example, builds the identity map with the §2 helpers,
sorts, and writes canonical bytes; the normal (non-ignored) golden test reads
the checked-in fixture, validates it, checks canonicality, and asserts the
semantic and envelope hash records equal literal digests pinned in the test.
Record the one-time regeneration run in the implementation notes; reviewers
treat any later regeneration as a contract change.

Negative fixtures under `fixtures/plans/invalid/` (each a small edit of the
golden, each with a test asserting the specific error): unknown
`schema_version`; unknown `identity_scheme`; non-empty `enabled_features`;
missing leaf entry; extra transition entry; wrong `rule_word` value; wrong
occurrence string; unsorted `model.boxes`; `linked_provenance` present with
`origin: direct_stable`; unknown top-level field; non-canonical bytes
(re-indented golden).

Unit tests in `crates/sembla-ir/tests/plan_validation.rs` for: dispatch on
presence/absence of `schema_version`; reserved-word rejection and synthetic
duplicate-word rejection (call the §4.11 function directly); slug validation;
mailbox identity formatting; the vectors-fixture cross-check (§2). CLI tests
in `crates/sembla-cli/tests/validate.rs` for `validate` and `plan-hash` on
the golden, and the not-yet-runnable error for `run`.

## Allowed files

- `crates/sembla-ir/src/plan.rs`, `identity.rs`, `canonical_json.rs` (new),
  `lib.rs`, `error.rs`, `validate.rs` (additions only; existing legacy checks
  and messages unchanged), `Cargo.toml` (+`sha2`)
- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/tests/validate.rs`
- `crates/sembla-ir/tests/plan_validation.rs`, `plan_golden.rs` (new)
- `fixtures/plans/**` (new), `Cargo.lock`
- implementation notes/artifacts created by the managed run

## Non-goals

- Executing plans, changing `ValidatedModel`, executor, RNG, or manifests
  (PRD 0004).
- Lean changes of any kind (PRD 0005).
- Linked-origin construction or source maps (PRD 0007) — `linked` is parsed
  and validated but no fixture exercises a full linked plan yet beyond §4.9's
  presence rule (a minimal hand-edited linked negative fixture is enough).
- Any change to `examples/**` or existing goldens/tests.

## Acceptance criteria

1. `cargo test --workspace` passes; existing `sembla-ir` golden and
   validation tests are unchanged and green.
2. `sembla validate fixtures/plans/two_box.plan.json` succeeds;
   `sembla plan-hash` prints the two frozen-format lines whose digests match
   the literals pinned in `plan_golden.rs`.
3. Every negative fixture fails validation with the specific expected error,
   each asserted by a test.
4. The Rust vectors test proves `rule_word` agrees with
   `fixtures/hash/vectors.json` for all five frozen identities.
5. `sembla validate examples/two_box.json` (legacy) behaves exactly as before
   this PRD; `run examples/…` is untouched; `run fixtures/plans/…` fails with
   the not-yet-runnable error.
6. `sembla-runtime` and `sembla-cuda` crates have no diff;
   `./scripts/check.sh` (fmt, clippy `-D warnings`, dependency policy — sha2
   in sembla-ir is the one allowed addition) and `git diff --check` pass.

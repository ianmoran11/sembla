# PRD 0002: Define the additive `sembla.parameters/v1` core contract

max_review_cycles: 3

## Dependencies

Contract-governance PRD 0001 is accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md) and
`DECISIONS.md` §P first.

## Context

Legacy `--params` files are anonymous JSON objects. They carry values but no
model binding, producer, target hashes or diagnostic evidence. This PRD defines
the missing artifact in the pure contracts crate. It does not change CLI
admission or runtime parameter resolution; PRD 0003 owns that boundary.

## Goal

Add a strict, canonical, estimator-neutral parameter artifact and exhaustive
positive/negative fixtures while leaving all legacy inputs and existing
serialized contracts byte-identical.

## Frozen JSON shape

The exact v1 shape is:

```json
{
  "schema_version": "sembla.parameters/v1",
  "model": {
    "name": "model_name",
    "ir_hash_algorithm": "sha256",
    "ir_hash": "64 lowercase hex characters"
  },
  "values": {
    "parameter_name": 1.25
  },
  "provenance": {
    "kind": "estimated",
    "estimator": {
      "name": "implementation name",
      "version": "implementation version",
      "implementation_sha256": "64 lowercase hex characters"
    },
    "inputs": [
      {
        "role": "targets",
        "path": "repository/or/artifact-relative/path.json",
        "sha256": "64 lowercase hex characters"
      }
    ],
    "diagnostics": [
      {
        "role": "fit-report",
        "path": "repository/or/artifact-relative/report.json",
        "sha256": "64 lowercase hex characters"
      }
    ]
  }
}
```

`provenance` is a tagged union with exactly two variants:

- `manual`: `{ "kind": "manual", "authority": <non-empty string> }`;
- `estimated`: the exact shape above.

For estimated provenance, `inputs` and `diagnostics` are non-empty, sorted by
`(role, path)` and contain no duplicate pair. Roles use the existing frozen ASCII
slug grammar. A producer records role `targets` when it actually consumed a
target ledger; directly estimated/profile artifacts instead name their truthful
source-data roles. The generic schema never requires a false target reference.
Paths are non-empty UTF-8 relative paths using `/`, contain no empty, `.` or
`..` segment, and are never absolute or URL-like. The contract hashes referenced
bytes; it does not require those paths to exist when the artifact is parsed
outside its original bundle.

Every JSON object—including `values`—rejects duplicate keys before typed serde
decoding; last-key-wins behavior is forbidden. Artifact input bytes must already
equal their canonical serialization exactly, so pretty-printed, reordered,
alternate-number-spelling, trailing-newline and otherwise non-canonical inputs
are rejected rather than silently normalized. A separate explicit writer may
produce canonical bytes from typed values.

`model.name`, manual `authority`, estimator `name` and estimator `version` must
be non-empty after trimming, contain no control character and equal their trimmed
form. Standalone shape validation imposes no extra slug rule on model names;
model-bound validation requires exact equality with the existing
`ValidatedModel` name (a plan-bound model already satisfies the plan slug rule).

Numeric rules are exact: Int parameters require a JSON integer token with no
fraction/exponent and a value in signed 64-bit range; Real parameters accept any
canonical JSON number token that converts to finite binary64, including integral
spelling and signed zero. NaN/Infinity, binary64 overflow, or a lexical number
that cannot round-trip to the same canonical numeric value fails. Canonical
reserialization defines the accepted exponent, decimal and signed-zero spelling.

The artifact is complete: `values` must contain exactly every parameter declared
by the bound validated model, with matching Int/Real types and finite values.
Partial overrides remain a legacy plain-object capability; a versioned artifact
never uses defaults silently.

Canonical bytes use the existing recursively key-sorted compact canonical JSON
encoding with **no trailing newline**. Array order is semantic and validated as
specified above. No timestamp, arbitrary metadata map or absolute command/path
is permitted.

## Requirements

### 1. Contract types and validation

Add `crates/sembla-ir/src/parameters.rs` and export its public contract from
`lib.rs`. Use a duplicate-aware raw JSON pass before strict serde decoding with
`deny_unknown_fields` on every object, then require exact canonical input bytes.
Expose:

- `PARAMETERS_SCHEMA = "sembla.parameters/v1"`;
- typed model binding, artifact-reference, estimator and provenance records;
- `ParameterArtifactV1`;
- shape validation independent of a model; and
- validation against `ValidatedModel`, returning path-aware errors and an exact
  ordered parameter assignment suitable for a caller to translate into its own
  runtime environment.

Do not depend on `sembla-runtime` or CLI types.

### 2. Canonical model hash helper

Add one `sembla-ir` helper that computes the SHA-256 of the existing
`to_canonical_json(model)` bytes used by run/pairs manifests. It introduces no
new hash domain and must match `sembla-cli::manifest::canonical_ir_hash` on all
fixtures. PRD 0003 may make the CLI delegate to it; this PRD must not change
manifest bytes.

### 3. Fixtures

Add:

```text
fixtures/parameters/manual.parameters.json
fixtures/parameters/estimated.parameters.json
fixtures/parameters/invalid/*.json
```

The positive artifacts bind an existing small canonical model. Invalid fixtures
cover every independent refusal class:

- duplicate key at every object level and in `values`;
- non-canonical whitespace/key order/number spelling/trailing newline;
- unknown schema and unknown field at every object level;
- malformed model name/hash/algorithm;
- empty, partial, extra or type-mismatched values;
- non-finite or out-of-range numeric spellings;
- malformed tagged provenance and fields from the other variant;
- empty estimated input/diagnostic arrays;
- duplicate or non-canonical reference order;
- whitespace-only/untrimmed authority, estimator or model strings;
- integer fraction/exponent/overflow, real overflow and signed-zero canonicality;
- malformed role/path/hash; and
- model-name or canonical-IR-hash mismatch.

Do not regenerate or edit existing model/plan/state/bundle/hash fixtures.

### 4. Registry and documentation

Register `sembla.parameters/v1` with owner
`crates/sembla-ir/src/parameters.rs`, CLI and maintained data producers as
future producers/consumers, kind `artifact-schema` and stability `versioned`.
Update `docs/architecture/contracts.md` to describe the now-defined contract but
state that CLI admission and producer integration arrive in PRDs 0003–0004.

## Allowed files

- `crates/sembla-ir/src/lib.rs`
- `crates/sembla-ir/src/parameters.rs` (new)
- `crates/sembla-ir/tests/parameters_contract.rs` (new)
- `crates/sembla-cli/src/manifest.rs` (test-only comparison or delegation with
  byte-identical output)
- `fixtures/parameters/**` (new)
- `docs/architecture/artifact-registry.json`
- `docs/architecture/contracts.md`

## Non-goals

- No CLI `--params` behavior change.
- No runtime, CUDA, Lean, estimator or data-pipeline change.
- No partial versioned assignment, migration command or automatic wrapping.
- No new hash domain, dependency, JSON Schema or arbitrary extension map.
- No edit to existing serialized fixtures or retained evidence.
- `DECISIONS.md` and the binding compatibility policy are read-only context in
  this PRD.

## Test guidance

Run:

```bash
cargo test --locked -p sembla-ir --test parameters_contract
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
python3 -B scripts/check-artifact-registry.py
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

Compare hashes of all pre-existing fixture trees before and after this PRD.

## Acceptance criteria

1. The exact JSON/provenance shapes and canonical-byte rule above are represented
   by strict public contract types in `sembla-ir`.
2. Duplicate-aware shape/model validation rejects every specified invalid
   class, including non-canonical bytes and numeric/string edge rules, with
   stable path-aware errors shared by independent fixtures.
3. A valid artifact is complete, finite, type-correct and bound to the exact
   model name/canonical IR hash; no default or unknown parameter survives.
4. Manual and estimated positive fixtures round-trip to identical canonical
   bytes across two processes/runs.
5. The core hash helper matches the existing CLI canonical IR hash without
   changing any current manifest byte.
6. The registry owns `sembla.parameters/v1` and all existing artifact fixtures
   remain byte-identical.
7. Full Rust, determinism, architecture and repository checks pass.

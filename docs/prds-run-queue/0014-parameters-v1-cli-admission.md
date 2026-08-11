# PRD 0003: Admit `sembla.parameters/v1` at CLI boundaries

max_review_cycles: 3

## Dependencies

Contract-governance PRDs 0001–0002 are accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md). The
positive and negative parameter fixtures from PRD 0002 are frozen inputs.

## Context

The runtime accepts only resolved `ParamEnv`; it must remain unaware of files,
provenance and estimation. The CLI already owns plain-object parsing. This PRD
extends that deep boundary to recognize a strict parameter envelope, validate
its model binding and unwrap complete values before execution.

## Goal

Accept `sembla.parameters/v1` everywhere a CLI parameter file is accepted,
preserve legacy behavior byte-for-byte, and record envelope identity in new run
manifests without leaking the artifact into runtime, plans or backends.

## Requirements

### 1. One shared parameter-input parser

Refactor `crates/sembla-cli/src/shared.rs` so every workflow delegates to one
parser returning:

- validated parameter overrides/assignment; and
- optional parameter-artifact identity (`schema_version` plus SHA-256 of the
  exact input bytes).

Dispatch rules are frozen:

- an object whose `schema_version` is the string `sembla.parameters/v1` is a v1
  envelope;
- an object with another string `schema_version` is an unsupported version and
  fails; and
- every other JSON object remains the legacy name→numeric override form,
  including a model that legitimately has a numeric parameter named
  `schema_version`.

A v1 envelope must pass PRD 0002 shape and model-bound validation before any
population/state load or backend construction. Legacy parsing and diagnostics
remain unchanged.

### 2. Workflow coverage

Use the shared parser for every current file-based parameter path, including:

- `run --params`;
- `sweep --params` base/pinned values;
- compare arms using parameter files; and
- backend-differential paths using parameter files.

Theta-file rows remain their existing separate draw-assignment contract. A
versioned complete envelope used as base parameters does not permit a theta row
to override the same name where current duplicate-source rules already reject
that combination.

The validated envelope is converted into the existing `ParamOverride`/`ParamEnv`
flow. No runtime or CUDA type mentions `ParameterArtifactV1`.

### 3. Additive manifest provenance

Add an optional, default-empty `parameter_sources` map to `RunManifest`. Values
contain exactly:

```json
{"schema":"sembla.parameters/v1","sha256":"<exact input bytes>"}
```

The exact per-kind matrix is:

- `manifest_kind = run`: map absent for legacy input or exactly key `run`;
- `manifest_kind = sweep`: map absent or exactly key `sweep_base`;
- `manifest_kind = compare`: map absent or the non-empty subset of `compare_a`
  and `compare_b` corresponding exactly to envelope-backed arms; and
- `diff-backends` has no `RunManifest`, so it records no entry here; its future
  machine-readable conformance report owns its own input provenance.

An empty map serializes as absent. All tuples are complete, algorithms are fixed
to SHA-256 by the schema, hashes are lowercase 64-hex, and duplicates, unknown
keys or kind/key combinations fail manifest validation.

This is the §P additive-extension case. Prove old-reader compatibility against
the exact CLI reader at `track_base` discovered by the binding README rule: a
temporary detached worktree builds that revision with `--locked`, then its
`verify-run` must accept a complete new manifest containing
`parameter_sources` and reproduce the same resolved run. Repository inspection
shows that exact `track_base` `RunManifest` reader does not deny unknown
root-level fields; this compatibility result is a fixed acceptance requirement,
not an alternative branch. The current reader must accept all retained old
manifests. If the executable old-reader test contradicts that evidence, this PRD
fails and the managed run stops; it must not invent a new manifest version.
Legacy plain-object inputs leave the map absent, so every pre-existing manifest
and deterministic output remains byte-identical. Envelope and equivalent legacy
runs may differ only by this provenance field after canonical normalization.

`verify-run` validates a present tuple and continues to replay from
`resolved_theta`; it does not require the original parameter file to remain on
disk and does not silently reconstruct or rewrite one.

### 4. Tests and fixtures

Add integration coverage for:

- valid manual and estimated envelopes through every workflow above;
- exact output/state/observation equality with equivalent legacy values;
- manifest difference restricted to `parameter_sources`;
- model name/hash mismatch before backend construction;
- partial, extra, wrong-type and non-finite assignments;
- unknown parameter schema versions;
- a numeric legacy parameter named `schema_version`;
- legacy partial overrides and their exact existing diagnostics;
- unknown/partial/malformed manifest provenance tuples;
- the detached `track_base` old reader accepting a new complete manifest and
  the current reader accepting retained old manifests; and
- feature-off CUDA refusing normally rather than falling back.

Do not regenerate existing golden manifests. Add independent new fixtures under
`fixtures/parameters/cli/` where needed.

### 5. Documentation

Create `docs/guides/parameter-artifacts.md` documenting:

- legacy partial objects versus complete versioned artifacts;
- exact model binding and provenance rules;
- supported CLI workflows;
- the manifest source hash;
- strict failures and portability; and
- the runtime/estimator quarantine.

Update architecture contract/estimation notes to mark CLI admission complete but
producer adoption pending PRD 0004.

## Allowed files

- `crates/sembla-cli/src/shared.rs`
- `crates/sembla-cli/src/manifest.rs`
- `crates/sembla-cli/src/run.rs`
- `crates/sembla-cli/src/sweep.rs`
- `crates/sembla-cli/src/compare.rs`
- `crates/sembla-cli/src/diff_backends.rs`
- `crates/sembla-cli/src/verify.rs`
- `crates/sembla-cli/src/tests.rs`
- `crates/sembla-cli/tests/parameters_envelope.rs` (new)
- `crates/sembla-cli/tests/run.rs`
- `crates/sembla-cli/tests/plan_run.rs`
- `crates/sembla-cli/tests/sweep.rs`
- `crates/sembla-cli/tests/compare.rs`
- `crates/sembla-cli/tests/gpu_differential.rs`
- `crates/sembla-cli/tests/backend_selection.rs`
- `crates/sembla-cli/tests/run_manifest.rs`
- `scripts/check-manifest-old-reader.py` (new)
- `scripts/tests/test_check_manifest_old_reader.py` (new)
- `fixtures/parameters/cli/**` (new)
- `docs/guides/parameter-artifacts.md` (new)
- `docs/architecture/contracts.md`
- `docs/architecture/estimation.md`
- `docs/architecture/artifact-registry.json`

## Non-goals

- No runtime, CUDA, Lean, IR model/plan or state schema change.
- No legacy-file rewrite, conversion command or default output migration.
- No theta-file schema change or estimator implementation.
- No new backend abstraction or dependency.
- No modification of retained manifests/evidence.

## Test guidance

Run focused CLI tests plus:

```bash
cargo test --locked -p sembla-cli --test parameters_envelope
python3 -B scripts/check-manifest-old-reader.py
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
python3 -B scripts/check-artifact-registry.py
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. One CLI parser owns envelope/legacy dispatch and every parameter-file workflow
   uses it.
2. V1 artifacts fail on any model/hash/completeness/type mismatch before backend
   construction and reach runtime only as existing resolved values.
3. Legacy partial objects, diagnostics, outputs and manifests remain
   byte-identical.
4. Equivalent envelope and legacy execution produce identical scientific
   outputs, states and observations; only the permitted manifest provenance
   differs.
5. Optional manifest provenance is strict for current readers and demonstrably
   safe for retained old readers under §P.
6. `verify-run` validates but does not require or recreate the source artifact.
7. Documentation accurately distinguishes complete versioned artifacts from
   legacy overrides, and all Rust/determinism/full checks pass.

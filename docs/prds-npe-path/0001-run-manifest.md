# PRD 0001: Run manifest and `verify-run`

## Context

The run contract is **seed + IR + θ + determinism level ⇒ reproducible
results** (`DESIGN.md` §2), but today's CLI prints `results_sha256` /
`final_state_sha256` to stdout, where they evaporate — a result artifact that
does not record the contract's left-hand side does not *have* the contract
(`DESIGN.md` §5.4, `DECISIONS.md` §E7). The roadmap orders this **before** the
GPU backend: with one backend the manifest is a sidecar file; with two it is a
retrofit across both, and a result that cannot name its executing backend
cannot participate in the differential gate.

## Goal

Every CLI execution emits a canonical run-manifest sidecar, and a new
`sembla verify-run` command re-executes a run from its manifest plus the
original inputs and asserts the recorded hashes — the contract tested, not
asserted.

## Specification

- Manifest module in `sembla-cli` with serde types. Serialization is
  **canonical**: sorted keys, compact JSON, one trailing newline (mirror the
  IR canonical serializer's conventions). Two byte-identical runs emit
  byte-identical manifests — nothing time-, host-, or path-dependent may enter
  the file (`DESIGN.md` §5.3 corollary).
- `sembla run ... --out results.csv` also writes `results.csv.manifest.json`
  recording at minimum:
  - `schema_versions` — per-concern integers, starting
    `{"manifest": 1, "backend_identity": 1}`;
  - `ir_hash` + `ir_hash_algorithm` (SHA-256 over the canonical IR bytes; the
    implementer names the algorithm ID and freezes it in fixtures);
  - model name, `seed`, `dt`, `ticks`, `determinism_level` (`"A"`);
  - `resolved_theta` — sorted parameter name → value, after defaults and
    overrides;
  - population source: the numeric spec or file path *basename* plus a
    `population_sha256` over the initializer input (file bytes or canonical
    numeric spec), with its algorithm ID;
  - backend identity tuple `{"backend": "cpu-oracle", "precision": "f64",
    "fell_back": false}` — an all-present-or-all-absent tuple;
  - `enabled_flags`: sorted, deduplicated (empty today — the field exists so
    `DESIGN.md` §5.5 has somewhere to record the first flag);
  - `results_sha256`, `final_state_sha256`, each beside its algorithm ID;
  - `component_versions` — workspace crate name → version.
- `sembla sweep --out <dir>/` writes `<dir>/run-manifest.json`: the shared
  fields once, plus an `executions` array (one entry per draw: `k`, resolved
  θ, per-draw results hash). `sembla compare` records both scenarios
  analogously. The existing `manifest.csv` (θ values) is unchanged and
  unrelated; document the naming distinction.
- `sembla verify-run <manifest> <model.json> [population/param args]`:
  recomputes the IR hash, re-runs, and compares every recorded hash. Exit 0
  on match; exit 1 with a field-by-field diff on mismatch.
- Readers reject a partial backend-identity tuple (present-but-incomplete ⇒
  hard error naming the tuple), and reject unknown `schema_versions` majors.

## Non-goals

Replay bundles, event capture, provenance databases (`DESIGN.md` §8). A flag
registry (zero flags exist). GPU backend values (PRD 0009 wires the real
ones). Manifest-driven re-execution without the original model/population
inputs (the manifest records hashes, not contents).

## Acceptance criteria

1. `cargo test --workspace` green; all prior tests still pass.
2. Determinism: the same `run`, `sweep`, and `compare` invocations twice ⇒
   byte-identical manifests.
3. Round-trip CI test: for the SIR example and one canonical model,
   `verify-run` against a fresh manifest exits 0 — for `run` and for at least
   one `sweep` draw.
4. Tamper test: editing the recorded seed (or θ, or `dt`) in a manifest makes
   `verify-run` exit 1 and name the mismatched field.
5. Partial-tuple test: a manifest with `backend` present but `fell_back`
   absent is rejected by the reader with an error naming the tuple.
6. `docs/examples/sir.md` documents the manifest fields and the `verify-run`
   workflow.

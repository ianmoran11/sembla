# PRD 0003: Final-state export and chained annual runs

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K4 binds:
annual calibration windows are separate runs chained by hashed state
artifacts — there is no checkpoint/restart subsystem, and a chained run
pair is honestly not bitwise-equal to one continuous run.

PRD 0002 provides `sembla.state/v1` read/write/hash and generic loading.
This PRD adds the other half of the chain: exporting the final committed
state of a run in the same format, and recording the chain in run
manifests via two all-present-or-absent tuples (DESIGN.md §5.4 rule 3).

## Goal

`sembla run … --export-state <path>` writes the final committed state as a
`sembla.state/v1` artifact; run manifests record `initial_state` and
`exported_state` hash tuples; chained-run identity and honest
non-equivalence are both test-pinned; legacy manifests stay byte-identical.

## Specification

### 1. `--export-state` on `run`

- New flag `--export-state <path>` for `sembla run` only (`sweep` and
  `compare` do not export state in this folder; reject the flag there via
  the normal unknown-flag error). Works for legacy models and plan
  envelopes, CPU backend; with `--backend cuda` reject deterministically
  (`--export-state requires the cpu backend for now`) unless the final
  state is already synced host-side by the existing path — investigate,
  pick the honest option, and record it in the implementation notes.
- Export writes the **final committed state** (post final tick barrier,
  the same state the final state hash covers) for every table, via
  PRD 0002's `write`. Determinism: same run ⇒ identical artifact bytes;
  and exporting must not perturb the run — `results.csv`, state hash, and
  output hash are bitwise identical with and without `--export-state`
  (test both).
- Refuse to overwrite an existing file (deterministic error), mirroring
  whatever overwrite discipline existing outputs use — if `--out`
  currently overwrites silently, still refuse here and note the
  difference: state artifacts are chain links and clobbering one silently
  invalidates a chain.

### 2. Manifest tuples

In `crates/sembla-cli/src/manifest.rs`, add two optional
all-present-or-absent tuples (serde skip-if-none; readers reject partial
tuples, matching the existing tuple-discipline tests):

```rust
pub struct StateArtifactTuple {
    pub format: String,               // "sembla.state/v1"
    pub hash: sembla_ir::HashRecordV1 // domain "sembla.state-artifact/v1"
}
// RunManifest gains:
//   initial_state:  Option<StateArtifactTuple>  — set iff --population was a .state artifact
//   exported_state: Option<StateArtifactTuple>  — set iff --export-state ran
```

`initial_state.hash` is computed from the input file bytes at load time;
`exported_state.hash` from the written file. Legacy runs (numeric or
`SEMBLA_POP` populations, no export) carry neither field — a byte-identity
test against a pre-change manifest golden proves it. `verify-run` must
accept manifests carrying the new tuples (its parser rejects unknown
fields today? read it first — if so, teach it the two tuples; its
verification semantics are unchanged and it does **not** re-derive state
hashes from artifacts in this PRD).

### 3. Chained-run tests (the point of the PRD)

In `crates/sembla-cli/tests/` (new `chained_runs.rs`):

1. **Chain identity.** Run A: a state-loaded model (PRD 0002's
   `refs_small` fixture or the two_box state fixture), 12 ticks, seed
   `s₁`, `--export-state a.state`. Run B: same model, `--population
   a.state`, 12 ticks, seed `s₂`, different θ via `--params`. Assert:
   manifest A's `exported_state.hash` equals manifest B's
   `initial_state.hash` (digest equality, the chain link); both runs are
   individually reproducible bitwise on re-execution.
2. **Honest non-equivalence.** Run C: the same model from the same
   original state, 24 ticks, seed `s₁`. Assert C's outputs **differ** from
   the concatenation of A+B even when `s₂ = s₁` and θ is unchanged —
   pinned, with a comment stating why (tick coordinates restart at 0 in
   run B, so draws differ by design; DECISIONS §K4). This test exists so
   nobody later "fixes" chaining into a silent lie.
3. **Export purity.** Same run with and without `--export-state`:
   results/state-hash/output-hash bitwise identical.
4. **θ across the chain.** Runs A and B above already vary θ; additionally
   assert manifest B records its own resolved θ and its `initial_state`
   tuple — i.e. the manifest chain alone documents the annual-window
   story (window state in, θ for the window, window state out).

### 4. Documentation

Extend `docs/state-format.md` with a "Chained runs" section: the
export/load cycle, the manifest tuples, the non-equivalence caveat stated
plainly, and a worked two-window example (commands + which manifest fields
link). One paragraph in `docs/composition.md` only if it already documents
`run` flags (read first; keep minimal).

## Allowed files

- `crates/sembla-cli/src/main.rs`, `manifest.rs`,
  `crates/sembla-cli/tests/**` (+ new goldens per existing conventions)
- `crates/sembla-runtime/src/state_artifact.rs`, `lib.rs` (only if export
  needs a missing accessor for final committed state)
- `docs/state-format.md`, `docs/composition.md` (minimal)
- implementation notes/artifacts created by the managed run

## Non-goals

- Sweep/compare export, occupied-only or delta artifacts, compression.
- Any automation that chains runs (no drivers, no loops — the CLI runs
  one window; chaining is the caller's composition of runs).
- Re-deriving or verifying state hashes inside `verify-run`.
- Tick/calendar continuity mechanisms (`Expr::Tick` is deferred, §K2/§K9).

## Acceptance criteria

1. `./scripts/check.sh` passes; legacy manifests are byte-identical to
   pre-change goldens; no frozen artifact changed.
2. All four chained-run tests pass, including the pinned non-equivalence
   test with its explanatory comment.
3. Manifest tuples are all-present-or-absent, rejected when partial (test
   included), and absent for legacy runs.
4. `--export-state` is deterministic, refuses to overwrite, and provably
   does not perturb the run.
5. The CUDA decision (reject vs supported-via-existing-sync) is recorded
   in the implementation notes with the code evidence that justified it.
6. `docs/state-format.md` documents the chain with the non-equivalence
   caveat; `git diff --check` passes.

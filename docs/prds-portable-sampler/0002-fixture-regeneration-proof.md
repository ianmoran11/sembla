# PRD 0002: Downstream fixture regeneration and cross-platform proof

## Context

PRD 0001 changed θ-draw bits. Everything downstream of a sampled θ now has
stale expectations: sweep golden outputs, `(θ, x)` export fixtures, the
Python pipeline's checked-in smoke fixtures (whose sidecar hashes bind their
bytes), and the committed NPE reference artifacts. Runs with explicit θ
(defaults or `--params`) are untouched — the sampler is the only changed
code. This PRD regenerates the downstream surface coherently and establishes
the cross-platform result the change exists for.

## Goal

Every fixture and committed artifact downstream of θ draws is regenerated
from the `libm`-backed sampler; the full local suite is green; the
cross-platform claim is proven on CI or honestly recorded as pending it.

## Specification

- **Rust fixtures:** regenerate any golden files or embedded expectations in
  `sembla-runtime`/`sembla-cli` tests that contain sampled θ values or
  hashes derived from them (sweep manifest/summary/per-draw goldens, pairs
  export fixtures, run-manifest fixtures for sweep executions). Fixtures with
  no sampled-θ dependency must not change — verify with `git diff --stat`
  discipline and say so in the implementation notes.
- **Python fixtures:** regenerate `calibration/npe/tests/fixtures/` (smoke
  pairs CSVs and their `.meta.json` sidecars — the embedded `pairs_sha256`
  values must be recomputed by the exporter, not hand-edited).
- **NPE reference artifacts:** re-run the documented reference flow
  (`generate_data.sh`, training, SBC) and commit refreshed
  `calibration/npe/artifacts/` outputs. Acceptance thresholds are unchanged;
  the run must still report `pass: true`. If the environment cannot run the
  pipeline, record *unanswered* with the exact commands — do not keep stale
  artifacts silently (mark them stale in the artifacts README if not
  regenerated).
- **Cross-platform proof:** after all local tests pass on the implementing
  (macOS/aarch64) machine, the proof that Linux/x86_64 produces identical
  sampler bits is CI's `Rust` job on the landed commit. The PRD's
  implementation notes must record this as the closing observation: either
  the observed green CI run (if visible to the implementer), or an explicit
  *unanswered — verify CI run for commit `<sha>` passes
  `sampler_mapping_is_bitwise_frozen`* handoff line for the operator.
- Nothing in `examples/*.json`, the IR, Lean parity fixtures, or CUDA
  fixtures may change — θ draws never enter the IR (`DECISIONS.md` §G1), and
  the parity/execution-hash checks use explicit θ. A diff touching those is a
  scoping error; stop and flag it.

## Non-goals

New sampler behavior, thresholds, or export schema changes. Backfilling
reproducibility of pre-change sweeps (declared broken in §G6). CI workflow
edits.

## Acceptance criteria

1. `cargo test --workspace` and `./scripts/check.sh` green locally.
2. The Python suite (`pytest` in `calibration/npe`) green locally, or
   *unanswered* with reason per the honest-reporting convention.
3. Refreshed reference `diagnostics.json` committed with `pass: true` (or the
   stale-marking + unanswered handoff as specified).
4. A `git diff --stat` review in the implementation notes classifying every
   changed fixture as θ-downstream, and confirming zero changes under
   `examples/`, `frontend/`, and CUDA fixture paths.
5. The cross-platform closing observation recorded as specified (observed
   green CI, or the explicit operator handoff line naming the commit).

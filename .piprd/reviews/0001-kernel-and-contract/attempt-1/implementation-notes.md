# PRD 0001 implementation notes

## Frozen pre-refactor baseline

Recorded before changing `frontend/Sembla/DSL.lean` on 2026-07-20.

Commands run from `frontend/`:

- `lake build` — exit 0 (`Build completed successfully.`)
- `bash scripts/test-negative.sh` — exit 0 (`Lean positioned negative and positive elaboration tests passed`)
- `bash scripts/check-parity.sh` — exit 0 (`Lean export, validation, canonical-byte/normalized parity, observation parity, and execution-hash parity passed`)

Additional no-drift evidence from the repository root:

- `git diff --exit-code -- examples frontend/lake-manifest.json` — exit 0, no output

The baseline parity command retained the existing literal `cmp` checks; no fixture was regenerated and no dependency/manifest update was needed.

## Implementation

- Added one `SurfaceModel` collection value retaining the declaration name/token, optional runtime-name metadata/token, original `dt` syntax, and all ordered nested declarations.
- Moved validation, reference/type/schema/effect checking, the sole `Model.mk` emission, one-time term evaluation, and widget attachment into `elaborateSurfaceModel`.
- Reduced legacy `model%` to collection plus a call to the shared kernel.
- Added and imported `Sembla.SurfaceKernelTests`, with multiple declarations in every ordered category, cross-box transition-order guards, nested attribute/schema/output-field/effect guards, and one literal exact `IR.toJson` contract.

## Post-refactor checks

- `cd frontend && lake build` — exit 0; the imported `Sembla.SurfaceKernelTests` module built.
- `cd frontend && bash scripts/test-negative.sh` — exit 0; positioned diagnostics remained accepted byte-for-byte by the existing harness.
- `cd frontend && bash scripts/check-parity.sh` — exit 0; all canonical byte, normalized IR, observation, fixed-seed output, summary, state-hash, and output-hash checks passed.
- `./scripts/check.sh` — exit 0; full Lean/Rust/proof-hygiene/parity suite passed.
- `git diff --check` — exit 0.
- `git diff --exit-code -- examples frontend/lake-manifest.json` — exit 0, no output.
- `git diff --exit-code -- frontend/Sembla/IR.lean frontend/Sembla/Json.lean frontend/lakefile.toml` — exit 0, no output.

No canonical fixture, dependency manifest, IR/JSON module, Rust source, or CI file was changed.

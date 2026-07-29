# PRD 0002 implementation notes

## Pre-implementation baseline

Recorded on 2026-07-20 before changing PRD 0002 implementation files:

- `cd frontend && lake build` — exit 0 (`Build completed successfully.`)
- `cd frontend && bash scripts/test-negative.sh` — exit 0 (`Lean positioned negative and positive elaboration tests passed`)
- `cd frontend && bash scripts/check-parity.sh` — exit 0 (`Lean export, validation, canonical-byte/normalized parity, observation parity, and execution-hash parity passed`)
- `git diff --exit-code -- examples frontend/lake-manifest.json` — exit 0, no output

## Implementation

- Added option-B `ℝ` parameter binders, optional `~ LogNormal a b` priors, and new list-form system declarations with derived or explicit runtime names.
- Added one pure ASCII snake-case/Greek transliteration helper shared by new parameter and system parsing, with source-token-preserving grammar validation and runtime collision diagnostics.
- Preserved legacy parameter and system runtime names exactly; all new declarations still collect into `SurfaceModel` and use the PRD 0001 kernel.
- Added bare parameter lowering to `Expr.param`, exact attribute/parameter ambiguity rejection, and enum-aware `≠` lowering.
- Added only the frozen aliases `ℝ`, `·`, `∧`, `≠`, and `≤`, reusing existing type-check paths and IR nodes.
- Extended imported surface guards with legacy/new model and JSON twins, direct IR-node checks, prior/priorless checks, and hazard-widget equivalence.
- Added standalone derivation/override/forward-reference coverage and complete positioned negative probes registered in `frontend/scripts/test-negative.sh`.

## Post-implementation checks

- `cd frontend && lake build` — exit 0 (`Build completed successfully.`)
- `cd frontend && bash scripts/test-negative.sh` — exit 0 (`Lean positioned negative and positive elaboration tests passed`)
- `bash frontend/scripts/check-parity.sh` — exit 0 (`Lean export, validation, canonical-byte/normalized parity, observation parity, and execution-hash parity passed`)
- `./scripts/check.sh` — exit 0; full Lean, Rust, proof-hygiene, diagnostics, and parity suite passed.
- `git diff --check` — exit 0.
- Frozen-path diff over canonical examples, `Sembla/IR.lean`, `Sembla/Json.lean`, `Sembla/Models.lean`, frontend manifests, Rust crates, CI, and public docs — exit 0, no output.

No canonical model/fixture, IR/JSON, widget rendering, Rust, dependency, CI, or public documentation file changed.

## Revision after review

- Added `check_failure_exact` in `frontend/scripts/test-negative.sh`. It extracts every positioned `file:line:column: error: ...` line and compares the complete ordered set exactly.
- Switched every PRD 0002 negative probe to the exact helper while leaving legacy checks on the compatibility helper.
- Verified with a synthetic expected-plus-unexpected diagnostic probe that an additional positioned error is rejected.
- Re-ran `lake build`, `scripts/test-negative.sh`, `scripts/check-parity.sh`, `./scripts/check.sh`, `git diff --check`, and the frozen-path diff; all passed.

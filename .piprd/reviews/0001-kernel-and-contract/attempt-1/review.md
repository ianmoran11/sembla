# PRD 0001 review

## Decision

**APPROVED** — no blocking issues found.

## Acceptance criteria

1. **Legacy compatibility and diagnostics:** PASS. `lake build` and `bash scripts/test-negative.sh` pass. The unchanged negative harness reports `Lean positioned negative and positive elaboration tests passed`. Pre-refactor baseline evidence is recorded in `implementation-notes.md`.
2. **Single collected representation and kernel:** PASS. `SurfaceModel` is at `frontend/Sembla/DSL.lean:95`; legacy collection is at line 402; the shared kernel is at line 765; the only `Model.mk` emitter in `DSL.lean` is `modelTerm` at line 757; and `model%` is the thin adapter at line 887. Elaboration/evaluation and widget attachment remain in the shared kernel at lines 868–883. Existing specialized builder/checker helpers remain private and singular.
3. **Focused ordering contract:** PASS. `frontend/Sembla/SurfaceKernelTests.lean` has a multi-declaration model and 21 closed guards covering parameter, box, table/system, cross-box transition, input, output, view, wire, summary, attribute, schema, output-field, and effect order. Line 149 pins the full literal `IR.toJson` result. `frontend/Sembla.lean:9` imports the module.
4. **Canonical and execution parity:** PASS. `bash scripts/check-parity.sh` passes with canonical-byte, normalized IR, observation, CSV/summary, state-hash, output-hash, and execution parity green.
5. **Frozen files:** PASS. `git diff --exit-code` over `examples`, `frontend/Sembla/IR.lean`, `frontend/Sembla/Json.lean`, `frontend/lakefile.toml`, `frontend/lake-manifest.json`, Rust crates, and CI paths returns 0. Workspace status contains only allowed implementation files and managed `.piprd` artifacts.
6. **Required checks:** PASS. `lake build`, `scripts/test-negative.sh`, `scripts/check-parity.sh`, `./scripts/check.sh`, and `git diff --check` all return 0. The untracked new test module also has no whitespace errors.

## Blocking issues

None.

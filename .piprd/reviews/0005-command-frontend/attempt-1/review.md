# PRD 0005 review

## Decision

**APPROVED** — no blocking issues.

## Acceptance criteria

1. **Complete command grammar and ordinary constants:** PASS. The indentation-structured grammar covers the required header, model declarations, interleaved box declarations, grouped rows, empty systems/models, and all frozen feature forms. `defineCommandModel` installs exactly one namespace-qualified ordinary `Model` definition.
2. **Single semantic kernel:** PASS. Legacy and command collectors both feed `elaborateSurfaceModel`; no quoted command-to-`model%` lowering, duplicate IR builder, type/schema/name checker, or second widget path exists. `DSL.lean` retains one `Model.mk` emitter.
3. **Legacy twins, bytes, order, and widgets:** PASS. Full-feature and deliberately interleaved command fixtures assert structural equality, literal serialized `IR.toJson` equality, every emitted category/list order, and representative state/hazard widget-prop equality against legacy twins.
4. **Forward references, deterministic resolution, and positioned diagnostics:** PASS. Forward parameter/system/schema references and all arrow forms are positive-tested. All 57 command negatives have exactly one `check_failure_exact` registration and the complete harness passes. The grouped oversized-row regression reports exactly at `CommandGroupedOversizedRows.lean:5:22` after normalization preserves canonical source information.
5. **Legacy/canonical/runtime compatibility:** PASS. Legacy `model%` remains on the shared kernel; canonical models, exporter aliases, fixtures, parity behavior, and execution hashes remain unchanged.
6. **Frozen scope:** PASS. No IR, JSON, Rust/runtime, dependency, workflow, canonical-model, public-documentation, or example JSON path is changed.
7. **Required checks:** PASS. Focused command elaboration, complete exact negative harness, full Lean build, parity, the full repository suite, `git diff --check`, and the frozen-path diff pass.

## Widget cursor evidence

The implementation note records completed manual VS Code checks for distinct system, reaction-arrow, and general-transition anchors. Data-level state/hazard props equal the legacy twin.

## Independent checks

- `cd frontend && lake env lean Sembla/CommandFrontendTests.lean` — pass
- `cd frontend && bash scripts/test-negative.sh` — pass
- `cd frontend && lake build` — pass
- `bash frontend/scripts/check-parity.sh` — pass
- `./scripts/check.sh` — pass
- `git diff --check` — pass
- Frozen-path diff — pass
- Grouped oversized-row exact reproduction — one error at line 5, column 22

## Blocking issues

None.

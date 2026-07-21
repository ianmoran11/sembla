# PRD 0002 re-review

## Decision

**APPROVED** — no blocking issues remain.

## Acceptance criteria

1. **Option-B and legacy forms:** PASS. New binders, tilde priors, derived/overridden systems, and legacy spellings all collect into `SurfaceModel` and use the single `elaborateSurfaceModel` kernel.
2. **Legacy/new equality:** PASS. Imported guards compare parameter lists, complete `Model` values, and exact `toJson` strings.
3. **Bare parameters and ambiguity:** PASS. Bare references emit derived `Expr.param` nodes; metadata and hazard-widget equivalence are pinned; ambiguity uses the mandated source-positioned message.
4. **Name derivation and override:** PASS. The shared helper and tests cover all required ASCII, acronym, digit, underscore, Greek, override, collision, cross-box, unsupported-character, and forward-reference cases.
5. **Aliases:** PASS. Only `ℝ`, `·`, `∧`, `≠`, and `≤` were added. Structural and negative tests pin their required existing IR nodes and type rules.
6. **Frozen scope:** PASS. No canonical model/fixture, IR/JSON, Rust, dependency, manifest, CI, or public-doc changes exist.
7. **README checks and diagnostics:** PASS. `check_failure_exact` extracts every positioned error line in order and compares the complete string exactly; all 21 PRD 0002 negatives use it while legacy checks retain the compatibility helper. A synthetic extra-error probe was rejected. All required checks passed.

## Independent checks

- `bash -n frontend/scripts/test-negative.sh` — pass
- Synthetic expected-plus-extra positioned diagnostic probe — rejected as required
- `cd frontend && lake build` — pass
- `cd frontend && bash scripts/test-negative.sh` — pass
- `bash frontend/scripts/check-parity.sh` — pass
- `./scripts/check.sh` — pass
- `git diff --check` — pass
- Frozen-path diff — pass

## Blocking issues

None.

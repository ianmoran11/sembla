# PRD 0006 review

## Decision

**APPROVED** — no blocking issues.

## Acceptance criteria

1. **Canonical and human-facing migration: PASS.** `frontend/Sembla/Models.lean` contains exactly eight `sembla_model` declarations for the required constants. `Sembla.Demos.Modeling.featureTour` and tutorial Steps 01–05 are command-style. The imported `Sembla.CanonicalModelsTests` enumerates all eight constants and pins exact runtime model, parameter, box, table, and transition names/order.
2. **Literal canonical and alias parity: PASS.** `frontend/Main.lean` still accepts 42 lookup names. Static set comparison finds exactly 42 exported names in `check-parity.sh`, with no missing or extra aliases. Every primary/alias output is validated and literal-`cmp` compared to its checked fixture before supplemental `diff-ir`. Full parity passes fixed-seed CSV, summary, final-state/output hash, dynamics, and conservation checks.
3. **Public documentation: PASS.** `frontend/README.md` comprehensively and accurately documents command headers, names, parameters/priors, exact operators, arrows and strict inference/disambiguation, general transitions, `freq` restrictions, every declaration family, stable order, forward references, compatibility `model%`, direct IR, exact commands, the Lean/Rust boundary, deferred C(i)/E, and declaration-name widget checks across all three themes and dark/light/high-contrast modes.
4. **Authority documentation: PASS.** `docs/design/surface-syntax-options.md` marks B, A, C(ii), and D implemented on 2026-07-20; retains C(i)/E deferrals and the `model%` compatibility kernel; and correctly keeps policy-controller multi-guard/multi-effect rules general. `DECISIONS.md` §A7 records the same decision. `DESIGN.md` contains no stale surface-syntax example requiring modification and its semantic/proof claims are unchanged.
5. **Legacy regression and frozen scope: PASS.** The `model%` parser, full legacy model, option-B twins, arrow twins, frequency twins, and command/legacy full-feature equality remain. No IR, JSON, Rust/runtime, fixture, dependency, manifest, workflow, proof, assessment snapshot, `DESIGN.md`, or external Vault path changed.
6. **Final source audit: PASS.** The canonical, demo, and five tutorial model files contain 14 command declarations and no legacy `model%`, `hazard parameter`, `system ... as`, `countBy`, or `sizeBy` authoring forms. Legacy syntax remains in focused compatibility tests and documentation only.
7. **Required checks: PASS.** Full Lean build, complete positioned negative harness, canonical/alias parity, full repository suite, structural/literal-JSON migration comparisons, frozen-path audit, and `git diff --check` pass.

## Blocking issues

None.

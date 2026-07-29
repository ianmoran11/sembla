# PRD 0006 implementation notes

## Canonical migration

- Re-authored all eight declarations in `frontend/Sembla/Models.lean` with `sembla_model`, preserving Lean constant names, exact runtime model/table names, parameter/box/table/transition/effect/view/wire/summary order, comments, and scientific literals.
- Used option-B binders and tilde priors, reaction arrows for exact one-enum-guard/one-effect rules, and `freq` for every exact keyed frequency. The two SIR-policy controller rules remain ordered general transitions because they have compound guards and multiple effects.
- Added imported `Sembla.CanonicalModelsTests`, which enumerates all eight constants and pins model, parameter, box, table, and transition names/order.

## Human-facing examples

- Migrated `Sembla.Demos.Modeling.featureTour` and tutorial Steps 01–05 to the command surface while preserving every guard, progressive conceptual layer, wire delay/runtime-initialization warning, and widget demonstration.
- Temporary read-only comparisons against each pre-migration declaration proved structural `Model` equality and literal `Sembla.IR.toJson` equality for the feature tour and all five tutorial models.
- Step 06 inspection/export, Step 07 proofs, `DeepIR.lean`, and focused legacy fixtures were not changed.

## Exporter and literal parity

- `frontend/Main.lean` remains unchanged with 42 accepted lookup names.
- Expanded `frontend/scripts/check-parity.sh` to enumerate the nine previously omitted dotted/slash/concise SIR, SIR-policy, and observation aliases. A set comparison confirms all 42 lookup names are exported with no missing or extra aliases.
- Every alias output is now Rust-validated and compared with literal `cmp` against its canonical checked fixture before supplemental `diff-ir`.
- `bash frontend/scripts/check-parity.sh` completed successfully and ended with:

  `Lean export, validation, canonical-byte/normalized parity, observation parity, and execution-hash parity passed`

  This includes all eight canonical exports, all 42 aliases, fixed-seed CSV bytes, summaries, final-state/output hashes, nontrivial dynamics, and conserved state counts. No fixture was regenerated.

## Documentation and authority

- Rewrote `frontend/README.md` around the complete command syntax, its exact header/naming/operator/arrow/frequency restrictions, every command declaration family, stable order and forward references, the `model%` compatibility kernel, direct-IR machine path, Lean/Rust boundary, exact commands, parity semantics, and declaration-name widget checks for academic/editor/notebook themes in dark/light/high-contrast VS Code.
- Marked B, A, C(ii), and D implemented on 2026-07-20 in `docs/design/surface-syntax-options.md`; retained C(i) as demand-driven deferral and E as rejected/deferred for human authoring; corrected the policy-controller arrow claim; and linked implementation/tests.
- Added `DECISIONS.md` §A7 recording the adopted surface, byte-stable layered-kernel rule, compatibility kernel, derived-name contract, and C(i)/E deferrals.
- `DESIGN.md` contains no nested-list or `model%` syntax example, so no change was needed; its semantic/runtime/proof claims remain untouched.

## Scoped source audit

- `frontend/Sembla/Models.lean`: eight `sembla_model` declarations; no `model%`, `parameter <name>`, `system ... as`, `countBy`, or `sizeBy` authoring forms.
- `Sembla.Demos.Modeling` and tutorial Steps 01–05: six `sembla_model` declarations total and no legacy public authoring forms. Root demo prose now names the command surface.
- Required legacy parser and regression twins remain in `DSL.lean`, `SurfaceKernelTests.lean`, `ReactionArrowTests.lean`, `FrequencyTests.lean`, and `CommandFrontendTests.lean`.
- No IR, JSON, Rust/runtime, dependency, manifest, workflow, proof, checked example fixture, assessment snapshot, C(i), or E implementation changed. No external Obsidian/Vault copy was modified by this managed run.
- Manual widget theme/layout checks were not repeated for this syntax-only migration; the retained instructions name declarations rather than unstable line numbers.

## Validation

All completed successfully on the final implementation workspace:

- `cd frontend && lake build`
- `cd frontend && bash scripts/test-negative.sh` (also run by parity)
- focused `Sembla.CanonicalModelsTests.lean`
- structural and literal-JSON pre/post migration comparisons for the demo and five tutorial models
- `bash frontend/scripts/check-parity.sh`
- `./scripts/check.sh`
- `git diff --check`
- frozen-path and scoped source audits

No implementation change was staged or committed.

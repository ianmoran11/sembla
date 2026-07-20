# Implementation Plan

## Goal
Create a six-PRD `/piprd` track that layers options B, A, C(ii), and D over the existing two-pass `model%` elaborator, then migrates all human-facing Lean models to command syntax without changing exported IR bytes, dependencies, or runtime semantics.

## Tasks

1. **PRD 0001 — Freeze the surface contract and extract the reusable `model%` kernel**
   - File: `docs/prds-surface-syntax/0001-kernel-and-contract.md`
   - Changes: Require a behavior-preserving refactor of `frontend/Sembla/DSL.lean`: introduce one collected surface-model representation and one shared two-pass validation/emission path, while keeping the current `model%` term syntax accepted. Both later sugar and the command elaborator must call this path; they must not construct `Sembla.IR.Model` independently. Add a positive baseline that compares `IR.toJson` for equivalent declarations and keep the existing positioned negative suite unchanged in behavior. Record current declaration-order semantics for params, boxes, systems, transitions, ports, views, wires, summaries, schemas, and effects.
   - Acceptance: `cd frontend && lake build`; `bash frontend/scripts/test-negative.sh`; `bash frontend/scripts/check-parity.sh`; repo-root `./scripts/check.sh`. All eight exports must still pass literal `cmp` against `examples/*.json`, not merely `diff-ir`.

2. **PRD 0002 — Option B: binders, names, priors, and mathematical aliases**
   - File: `docs/prds-surface-syntax/0002-binders-names-notation.md`
   - Changes: Extend `frontend/Sembla/DSL.lean` and the positive/negative suites with bare parameter resolution, `ℝ`, tilde priors, derived table names, and the exact Unicode operators frozen below. Preserve legacy `parameter`, `prior`, `as`, and `Real` only as compatibility/kernel spellings; public examples stop using them in PRD 0006. Test each new form against an equivalent legacy model using byte equality.
   - Acceptance: Assert that bare `β` emits `Expr.param "beta"`, not a substituted default; tilde and legacy priors serialize identically; derived and explicit table names serialize identically; aliases produce the intended existing IR nodes. Exact negative diagnostics cover unknown bare identifiers, parameter/attribute ambiguity, duplicate derived parameter names, duplicate derived table names, unsupported name derivation, and the existing numeric/type failures.

3. **PRD 0003 — Option A: reaction arrows with deterministic inference**
   - File: `docs/prds-surface-syntax/0003-reaction-arrows.md`
   - Changes: Add arrow declarations to the existing transition syntax and shared elaborator in `frontend/Sembla/DSL.lean`. An arrow must lower only to one enum equality guard, the supplied hazard, and one assignment on the same enum attribute; the general `transition … where` form remains mandatory for additional guards/effects. Preserve original name tokens for widget anchors and diagnostics. Add legacy-versus-arrow byte twins and focused negative files.
   - Acceptance: Arrow and expanded transitions have identical `IR.toJson`; SIR-shaped arrow models compile; diagnostics name and point at unknown source/destination variants, unknown explicit systems/attributes, non-enum attributes, ambiguous system/state inference, and non-Real hazards. Self-loops are positive-tested. No inference may depend on declaration iteration order.

4. **PRD 0004 — Option C(ii): keyed `freq` only**
   - File: `docs/prds-surface-syntax/0004-keyed-frequency.md`
   - Changes: Add exactly `freq (<predicate>) over <ref>` as an atomic `semblaExpr`. Lower it to the same `Expr.div (Expr.agg count … predicate) (Expr.agg count … true)` tree currently produced by `countBy ref (predicate) / sizeBy ref`. Require a Boolean, row-local predicate and a declared `Ref` on the selected system. Explicitly defer C(i) comprehensions (`#{…}`): no dotted row-variable scope or set-builder syntax is accepted in this track.
   - Acceptance: A structural/JSON twin proves byte-identical lowering; tests reject unknown keys, non-`Ref` keys, non-Boolean filters, and aggregate/input/nested-frequency predicates with a diagnostic explaining that frequency predicates are row-local and joins use declared keys only. Existing `countBy`/`sizeBy` remains kernel-compatible.

5. **PRD 0005 — Option D: complete command-style frontend**
   - File: `docs/prds-surface-syntax/0005-command-frontend.md`
   - Changes: Add the `sembla_model` command grammar and elaborator in `frontend/Sembla/DSL.lean` (or a dependency-free `frontend/Sembla/CommandDSL.lean` imported by `frontend/Sembla.lean` if source size requires separation). It must collect the full current feature set—params, boxes, systems/attributes, inputs, arrow and general transitions, outputs, views, wires, and summaries—then invoke the shared kernel from PRD 0001. Blocks are indentation-delimited and contain no list brackets or separator commas. Add a full-feature command model equivalent to `Sembla.Demos.Modeling.featureTour`, command-specific negative tests, and widget cursor verification on original system/transition tokens.
   - Acceptance: The command twin and kernel twin have identical `IR.toJson`; forward refs and arbitrary interleaving collect correctly; stable partitioning preserves order within every IR list; all duplicate/unknown/type/schema diagnostics are positioned at command tokens; widget prop tests remain green. The command creates an ordinary namespace-respecting `Model` constant usable by the unchanged exporter.

6. **PRD 0006 — Final command-style migration and documentation**
   - File: `docs/prds-surface-syntax/0006-migrate-models-and-docs.md`
   - Changes: Convert all eight declarations in `frontend/Sembla/Models.lean`, `frontend/Sembla/Demos/Modeling.lean`, and all five `frontend/Sembla/Tutorial/Step*.lean` files to `sembla_model`. Keep legacy `model%` positive/negative fixtures as kernel regression coverage; add command fixtures rather than deleting the old coverage. Rewrite `frontend/README.md` and demo/tutorial prose to lead with command syntax, explain that `model%` is the stable low-level kernel, document name derivation/overrides and inference, and update widget cursor instructions so they do not rely on stale line numbers. Mark `docs/design/surface-syntax-options.md` as implemented for A–D(ii), with E and C(i) still deferred.
   - Acceptance: `frontend/scripts/check-parity.sh` literally `cmp`s all eight newly command-authored exports with checked-in JSON, including the exceptional `observations` table name `"Person"`; its fixed-seed CSV/hash checks remain identical. `lake build`, negative tests, proof checks, widget tests, and `./scripts/check.sh` pass. `frontend/lakefile.toml` and `frontend/lake-manifest.json` have no dependency changes.

## Binding README Constraints

Create `docs/prds-surface-syntax/README.md`; every PRD must require it to be read first and treat these points as binding:

- **Authority and blast radius:** `DESIGN.md` §§4.2/5.5, `DECISIONS.md` §§A1/A5/G3, `docs/design/surface-syntax-options.md`, and existing fixture contracts govern. This track changes the Lean surface only. No edits to `frontend/Sembla/IR.lean`, JSON encoding, Rust crates, `examples/*.json`, runtime kernels, proofs, or dependencies are allowed.
- **Kernel boundary:** the existing enclosing, multi-pass `model%` elaborator remains accepted and is the sole semantic kernel. New forms produce the same collected surface declarations and use the same validation/emission functions. Legacy tagged/bracketed spellings remain compatibility-only; D is the final public authoring style. No second IR builder and no option E/do-builder.
- **Byte identity:** success means literal bytes from `IR.toJson` and shell `cmp`, including names, exact scientific values, list/schema/effect order, and transition order (which controls rule IDs). `diff-ir` is supplemental and cannot replace `cmp`. Every sugar PRD adds a legacy-versus-new byte twin before canonical migration.
- **Exact option-B syntax:** pin `param β : ℝ := 0.8 ~ LogNormal(-0.2231, 0.25)` (parser-stable comma pair); priorless params omit the suffix. Bare-name lookup is attribute first or parameter only when exactly one is visible; declarations that make both visible are rejected rather than shadowed. `ℝ → Real`, `· → Expr.mul`, `∧ → Expr.and`, `≠ → Expr.ne`, and `≤ → Expr.le`, at the existing corresponding precedences. Do not opportunistically add `∨`, `¬`, `≥`, ASCII `!=`/`<=`, new prior families, or term-level Lean expressions.
- **Name derivation:** derive parameter IR names by snake-casing ASCII identifiers and transliterating supported Greek identifier components (`β→beta`, `γ→gamma`, etc.); derive system table names by documented ASCII CamelCase-to-snake_case rules. Preserve underscores/digits, define acronym boundaries with tests, and reject unsupported characters. Collisions after derivation are errors. System `(name := "…")` is the sole explicit table-name override and is required wherever derivation would change a frozen name, notably `observations`'s `"Person"`. The command header has optional `(name := "IR model name")`; canonical models use it whenever the Lean declaration name does not derive to the frozen model name.
- **Exact arrow syntax:** support `infect : S →[hazard] I`, `infect : health: S →[hazard] I`, and optional system disambiguation `infect on Person : health: S →[hazard] I`. Without `on`, infer the unique `(system, enum attribute)` candidate; with `on`, restrict inference to that system; with an attribute label, restrict to that attribute. Zero or multiple candidates are named errors listing candidates. Source and destination must be variants of the same enum attribute. No multi-guard or multi-effect arrow semantics.
- **Exact aggregate syntax:** only `freq (<predicate>) over employer`; parentheses and `over` key are mandatory. It is scoped to the transition/output/view's selected table and declared `Ref`, never a global frequency. C(i) set comprehensions remain deferred because no checked-in model requires them.
- **Exact command syntax:** pin the header as `sembla_model Sir (name := "sir_workplace_frequency_dependent") (dt := 0.25) where`; `(name := …)` is optional and `(dt := …)` mandatory. Pin systems as `system Person (name := "person") (rows := 1_000_000) where`, with optional `name` and no `where` for an empty system. Command attributes are `health : {S, I, R}`, `risk : ℝ`, `visits : Int`, and `employer : Employer`. General transitions use repeated `set attr := value` lines. Inputs use `input p where field : Ty`; outputs use `output p from System where field : Ty := count where …` or `:= sum (<expr>)`. Views are `view v := count System [where …]` or `view v := (sum|min|max) System [where …] using <expr>`. Wires retain `wire box port -> box port`. Summaries are `summary s := (sum|min|max|last|argmaxₜ) box.view`. Omitted categories need no empty block. Declarations may be interleaved, but relative order within each emitted IR list is textual and stable.
- **Diagnostics:** no accepted-and-ignored construct. Every feature PRD adds complete failing files under `frontend/Negative/` and exact expected `file:line:column: error: …` entries in `frontend/scripts/test-negative.sh`, plus positive files under `frontend/Positive/`. Semantic failures must use original syntax tokens; generic parser errors are not substitutes for required ambiguity/scope/type diagnostics.
- **Checks per PRD:** run `cd frontend && lake build`, `bash frontend/scripts/test-negative.sh`, `bash frontend/scripts/check-parity.sh`, and repo-root `./scripts/check.sh`. Preserve `Sembla.WidgetTests`, scientific tests, proof checks, exporter aliases, and Rust tests transitively.

## Files to Modify

- None while authoring the PRD set; implementation files and tests are named inside each PRD.

## New Files

- `docs/prds-surface-syntax/README.md` — run order, frozen syntax, global invariants, and check matrix.
- `docs/prds-surface-syntax/0001-kernel-and-contract.md` — shared-kernel refactor and baseline.
- `docs/prds-surface-syntax/0002-binders-names-notation.md` — option B.
- `docs/prds-surface-syntax/0003-reaction-arrows.md` — option A.
- `docs/prds-surface-syntax/0004-keyed-frequency.md` — option C(ii).
- `docs/prds-surface-syntax/0005-command-frontend.md` — option D implementation.
- `docs/prds-surface-syntax/0006-migrate-models-and-docs.md` — final migration and docs.

## Dependencies

- PRD 0001 is foundational and must land first.
- PRD 0002 provides bare params, name derivation, and notation consumed by arrows and commands.
- PRD 0003 depends on 0001–0002; PRD 0004 depends on 0001–0002 and is exercised inside arrows after 0003.
- PRD 0005 depends on all prior sugars and must reuse their syntax nodes/elaboration rather than reimplement them.
- PRD 0006 is migration-only and cannot begin until the full command feature twin, negatives, and widgets pass in 0005.

## Risks

- Lean indentation grammar can accidentally produce generic parse errors or lose source ranges; parser probes and command-specific positioned negatives are required before migration.
- Bare identifiers create a real parameter/attribute namespace collision; silently choosing one would change semantics, so the README must bind rejection.
- Arrow inference across multiple systems/state columns is otherwise declaration-order dependent; unique-candidate rules and explicit `on`/attribute escape hatches are mandatory.
- Name derivation can silently alter fixture strings; Greek transliteration, acronym behavior, collision tests, and explicit overrides must land before any canonical migration.
- Stable collection/partition order is semantic for schema columns, outputs, summaries, transition rule IDs, widgets, and runtime hashes.
- `observations` intentionally exports table `"Person"`, unlike normal snake-case derivation; it will fail byte parity unless explicitly overridden.
- Adding C(i) comprehensions, option E, generalized Lean terms, new IR constructs, prettier-printer work, or widget redesign would expand scope and should be rejected or moved to a later PRD track.

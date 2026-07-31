# PRD 0006: Migrate human-facing models and land syntax authority

## Context

Read the folder README first; its constraints bind. PRDs 0001–0005 have landed
and tested the complete mathematical surface as byte-identical sugar over the
legacy `model%` kernel. This final PRD makes `sembla_model` the public authoring
style, migrates every checked human-facing model, and records both what was
implemented and what remains deliberately deferred.

This is migration/documentation only. If a canonical model cannot re-export
literal fixture bytes using the new command syntax, stop and fix the surface
implementation from earlier PRDs; do not edit the fixture or bend the IR.

## Goal

All canonical and tutorial/demo human-authored models use the command frontend,
all exports/runtime hashes remain identical, public docs teach the new syntax,
and authority documents record A/B/C(ii)/D as implemented while C(i), E, and
target 1b remain open/deferred as applicable.

## Specification

### 1. Migrate all eight canonical model declarations

Rewrite the eight declarations in `frontend/Sembla/Models.lean` using
`sembla_model`, preserving their existing Lean constant names and exact
`Model.name` strings:

- `sir`
- `observations`
- `sirPolicy`
- `reversibleCtmc`
- `radioactiveDecayChain`
- `sisImportation`
- `seirsWaning`
- `noisyVoter`

Use explicit command `(name := "...")` overrides whenever constant-name
derivation does not equal the frozen `Model.name`. Use system `(name := "...")`
overrides whenever table-name derivation does not equal frozen bytes. In
particular, `observations` must preserve table name `"Person"`, while SIR must
preserve `"person"`/`"employer"`.

Migration rules:

- Use option-B parameter bindings, tilde priors, bare parameter references, and
  mathematical aliases where the exact current expression is representable.
- Prefer Greek bindings where they derive to frozen parameter names (`β`, `γ`,
  `λ_parent`, etc.); retain clear ASCII identifiers where transliteration would
  not reproduce a frozen name without invention.
- Convert every transition expressible as exactly one enum guard/effect to a
  reaction arrow. Keep controller/multi-effect/general rules in the command
  general-transition form.
- Replace every exact legacy `countBy key (predicate) / sizeBy key` idiom with
  `freq (predicate) over key`. Do not rewrite aggregates that are not exact
  frequencies.
- Preserve declaration and effect order exactly. Do not reorder for aesthetics.
- Preserve model comments explaining scientific semantics and runtime limits.

`frontend/Main.lean` exporter lookup/alias behavior must remain unchanged unless
a purely mechanical import/name adjustment is required. Extend
`frontend/scripts/check-parity.sh` so **every** arm/alias in `lookupModel` is
enumerated, including the currently omitted dotted/slash forms for `sir`,
`sirPolicy`, and `observations`. Export every alias and use literal `cmp` against
the canonical fixture/export bytes; `diff-ir` alone is insufficient. All
existing concise, camel-case, dotted, and slash-qualified aliases must resolve
to the same bytes.

### 2. Migrate checked demo/tutorial authoring examples

If present in the repository at implementation time, migrate:

- `frontend/Sembla/Demos/Modeling.lean`'s `featureTour`; and
- the five model-defining progressive tutorial modules
  `frontend/Sembla/Tutorial/Step01_Recovery.lean` through
  `Step05_PolicyFeedback.lean`.

Step 06 inspection/export and Step 07 proof files are not model declarations;
update imports/names/prose only if the migration requires it. Update
`frontend/Sembla/Demos.lean` and `frontend/Sembla/Tutorial.lean` root prose so
neither still describes `model%` as the complete public surface. Preserve every
existing `#guard`, widget demonstration, runtime initialization warning, and
honest target-1a/1b boundary. The tutorial must remain genuinely progressive:
each step adds the same conceptual layer as before.

Do not convert `DeepIR.lean` direct-constructor examples or legacy-focused
positive/negative kernel fixtures; those intentionally demonstrate lower-level
paths.

### 3. Keep explicit legacy regression coverage

Do not delete the legacy `model%` parser or all old-syntax tests. Keep at least:

- one full-feature legacy `model%` positive model;
- option-B legacy/new twins;
- arrow expanded/arrow twins;
- frequency expanded/frequency twins; and
- command/legacy full-feature equality.

These are the semantic-kernel regression suite. Public canonical/example files
must not need `model%`, `parameter <name>`, `system ... as "..."`, or exact
frequency-shaped `countBy / sizeBy` after migration, except inside comments
showing compatibility or deliberate tests.

Add a focused imported test that enumerates all eight canonical constants and
asserts their exact names/order remain frozen. Do not add a brittle repository
text grep that blocks intentional compatibility tests.

### 4. Update frontend documentation

Rewrite `frontend/README.md` so its primary complete example is the frozen
`sembla_model` command syntax. Document:

- command header, optional model name override, and mandatory `dt`;
- parameter bindings, tilde priors, bare names, supported aliases;
- system/table name derivation and explicit override;
- arrow inference/disambiguation and when general transitions are required;
- `freq`'s selected-system/Ref/row-local restrictions;
- command forms for systems/attributes, inputs, outputs, views, wires, and
  summaries;
- stable textual order and multi-pass forward references;
- `model%` as the supported low-level compatibility/kernel form;
- direct IR constructors as the machine-writer path;
- exact build, negative-test, export, and parity commands; and
- the unchanged boundary: Lean elaborates/inspects/serializes; Rust validates
  whole models and executes them.

Update widget cursor instructions to name declarations rather than hard-coded
line numbers. Keep all three themes and manual dark/light/high-contrast checks.

### 5. Land authority documentation

Update `docs/design/surface-syntax-options.md`:

- change status from exploratory to implemented, dated with the actual landing
  date;
- mark B, A, C(ii), and D implemented;
- state that `model%` remains the compatibility kernel;
- keep C(i) keyed comprehensions deferred until a real model requires them;
- keep E deferred/rejected for human authoring; and
- link to the final frontend README and tests; and
- correct the stale claim that arrows cover every canonical transition: the
  policy controller's multi-guard/multi-effect rules intentionally remain
  general transitions.

Add a concise decision entry to `DECISIONS.md` recording the adopted human
surface, byte-stable layered-macro rule, compatibility kernel, derived-name
contract, and C(i)/E deferrals. Update the relevant Lean/surface example in
`DESIGN.md` only as needed so the authority document no longer presents stale
nested-list syntax. Do not alter IR/runtime/proof claims.

Do not edit the dated `docs/archive/assessments/sembla-assessment-2026-07-18.md` snapshot.

### 6. Prove literal canonical parity

`frontend/scripts/check-parity.sh` must continue to:

- export all eight command-authored canonical models;
- run literal `cmp` against each checked-in JSON fixture;
- validate both sides in Rust;
- export every `frontend/Main.lean` alias and literal-`cmp` each alias output
  against its canonical fixture/export bytes;
- execute checked/exported models with fixed seeds;
- compare CSV bytes, summaries, final-state hashes, and output hashes; and
- assert nontrivial dynamics and conserved state counts.

No fixture regeneration is permitted. Record the successful command output in
implementation notes.

### 7. Final source audit

Record a scoped audit showing:

- canonical `Models.lean` declarations are all `sembla_model`;
- migrated demo/tutorial model declarations are command-style;
- legacy syntax remains only in focused compatibility tests/docs sections;
- no C(i), E, new dependency, IR, JSON, Rust, example fixture, workflow, or
  proof changes entered the diff; and
- external Obsidian/Vault copies were not modified by the managed run.

## Allowed files

- `frontend/Sembla/Models.lean`
- relevant `frontend/Sembla/Demos/*.lean`, `frontend/Sembla/Demos.lean`,
  `frontend/Sembla/Tutorial/Step*.lean`, and `frontend/Sembla/Tutorial.lean`
  model/root examples if present
- `frontend/Main.lean` only for mechanical name/import preservation if needed
- `frontend/scripts/check-parity.sh` to enumerate and byte-compare every exporter alias
- surface syntax/positive/negative/widget tests and `frontend/Sembla.lean`
- `frontend/README.md`
- `docs/design/surface-syntax-options.md`
- `DECISIONS.md`
- the relevant syntax example only in `DESIGN.md`
- implementation notes/artifacts

## Non-goals

Removing `model%`, deleting legacy regression tests, migrating direct IR machine
examples, C(i), E, formatter/autocomplete/pretty-printer work, widget redesign,
IR/JSON/Rust/runtime/backend changes, fixture regeneration, workflow YAML, proof
changes, assessment snapshot edits, or external Vault synchronization.

## Acceptance criteria

1. All eight canonical models and every present human-facing demo/tutorial model
   use `sembla_model` while retaining the same Lean constant and frozen runtime
   names.
2. Literal `cmp` proves every exported canonical JSON file and every
   `frontend/Main.lean` alias output byte-identical to the checked fixture;
   all fixed-seed CSV/summary/hash checks remain green.
3. Public docs comprehensively teach the frozen command syntax and honestly
   distinguish command surface, compatibility kernel, direct IR, Rust
   validation/execution, and deferred C(i)/E work.
4. `docs/design/surface-syntax-options.md`, `DECISIONS.md`, and the narrow
   `DESIGN.md` syntax example agree on the implemented/deferred status without
   changing semantic/proof claims.
5. Legacy `model%` and every syntax twin remain as focused regression coverage;
   no fixture, IR/JSON, Rust, dependency, workflow, proof, assessment, or Vault
   changes occur.
6. Source audit finds no legacy public authoring forms in migrated model files
   except intentional compatibility prose/tests.
7. All required README checks and `git diff --check` pass.

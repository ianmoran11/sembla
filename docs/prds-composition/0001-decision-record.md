# PRD 0001: Record the accepted Option D decisions

## Context

Read `docs/prds-composition/README.md` first; its constraints bind every PRD in
this folder. This PRD is documentation-only. The project rule (DESIGN.md §,
DECISIONS.md preamble) is that normative choices live in `DECISIONS.md`, and
`docs/design/option-d-architecture.md` §26.1 item 10 requires accepted choices
to be recorded there **before** implementation. Later PRDs cite these decisions
by number, so the section labels below are frozen.

## Goal

`DECISIONS.md` gains a new section J recording every decision the folder README
freezes, and both design documents gain status lines pointing at the accepted,
amended form, without changing any code, fixture, or existing decision text.

## Specification

### 1. Add section J to `DECISIONS.md`

Append a new top-level section after the existing section I:

```markdown
## J. Composition and the Option D architecture (accepted 2026-07-21)
```

Under it, add subsections `### J1.` through `### J12.` with exactly the
following subjects. For each, write 3–10 sentences in the existing DECISIONS.md
style: state the decision, the alternatives rejected, and the reason. Source
material is `docs/design/option-d-architecture.md` and the amendments in
`docs/prds-composition/README.md`; where they conflict, the README wins.

- **J1. Option D pipeline accepted.** Serialized composition source → one
  canonical Lean 4 linker → versioned flat executable plan → Rust validation
  and execution. Scope of the first release is the doc's Phases 0–4
  (decisions, plan envelope and stable identity, composition source, linker
  with product/wires/nesting, spec statements, surface syntax, bundle).
  Recursive runtime hierarchy (doc §24.2) and a free composition AST as the
  executable contract (§24.3) are rejected.
- **J2. Hash algorithm.** All new hashes are SHA-256 with domain separation:
  `SHA-256(domain-string ++ 0x00 ++ payload)`. Persisted hashes are records
  `{algorithm: "sha256", domain, digest}` with lowercase hex digests. blake3
  is rejected because `sha2` is already the approved manifest dependency and
  `scripts/check.sh` enforces the dependency policy.
- **J3. Stable identity grammar.** Copy the "Frozen identity grammar" section
  of `docs/prds-composition/README.md` essentially verbatim: slugs, stable
  declaration IDs (`def:`, `inst:`, `port:`, `wire:`, `model:`, `expose:`),
  structural occurrence chains (`occ:north/population`), transition occurrence
  identities (`occ:north/population#infect`), wire and mailbox identities, and
  plan leaf naming (occurrence slugs joined by `/`). Identities are never
  display names or traversal positions.
- **J4. RNG strategy (doc open question 2 resolved).** The Philox coordinate
  layout `[tick, rule_word, entity_id, draw_idx]` is unchanged. For versioned
  plans the `u32` rule word is content-addressed: the first 4 bytes,
  big-endian, of `SHA-256("sembla.rule-word/v1" ++ 0x00 ++
  transition-occurrence-identity)`. Reserved words `u32::MAX - 1` and
  `u32::MAX` and any collision between accepted identities are deterministic
  link/validation errors; identities are never reassigned. A persisted
  next-free registry is rejected as history-dependent; widening the
  coordinate is rejected as an RNG format change.
- **J5. Canonical JSON (`sembla.canonical-json/v1`).** Record the encoding
  rules from the README: UTF-8, no whitespace, no trailing newline, byte-wise
  sorted object keys, omitted (never `null`) optional fields,
  all-present-or-all-absent tuples, canonical-order arrays, serde_json-style
  escaping, and serde_json `float_roundtrip`-compatible number printing.
- **J6. Version strings.** Reproduce the README's frozen version-string table
  (source schema, plan schema, linker semantics, identity schemes, canonical
  encoding, source-map schema, hash domains including
  `sembla.rule-word/v1`). Unknown or missing required versions are rejected,
  never interpreted by best effort.
- **J7. Plan origins and the legacy path.** Versioned plan envelopes have
  exactly two origins in V1, `linked` and `direct_stable`. Unversioned model
  JSON (everything currently in `examples/`) is the envelope-free `legacy`
  path: it keeps dense declaration-order rule IDs under the scheme name
  `sembla.identity/legacy-positional-v1`, byte-identical behavior forever,
  and is never silently upgraded. The doc's "normalized legacy" origin is
  deferred.
- **J8. Parameter bindings (doc open question 5 resolved).** An instance
  binds each component parameter requirement to a model-level parameter
  **name**, never a literal. Binding two instances to the same model
  parameter is explicit sharing; distinct values require distinct model
  parameters. θ, `Param` resolution, priors, and sweep behavior are
  unchanged.
- **J9. `dt` and scheduler domains (doc open question 6 resolved).**
  Components never declare `dt`. The root composition declares `outer_dt`,
  and V1 has exactly one scheduler domain, `domain:global`, algorithm
  `tau_leap`, containing every leaf.
- **J10. Composition laws are byte-equality (doc open question 4 resolved).**
  Plan collections are sorted by stable identity, so product
  associativity/symmetry, alpha-renaming, and declaration-permutation laws
  are byte-equality of canonical plan cores. The plan **semantic** hash
  (domain `sembla.plan-core/v1`) covers `{schema_version, identity_scheme,
  model, identity}` and excludes `origin` and `linked_provenance`; the
  **envelope** hash (domain `sembla.plan-envelope/v1`) covers the whole
  envelope. Source maps and display names never enter the semantic hash.
- **J11. Hashes live outside the plan.** Plan files never embed their own
  hash records; hashes are computed over exact canonical file bytes and
  recorded in run manifests and bundle manifests. This removes the doc §5.3
  self-reference rules.
- **J12. Deferred constructs.** Synchronized transition families,
  `Share`/`Identify`, semantic invariants and constrained products,
  observational assertions, heterogeneous schedulers, explicit
  adapter/merge components, dynamic component topology, ACSet storage or any
  Julia dependency, non-Lean source producers, and any non-Lean linker are
  all out of scope for this release. Every one must be rejected with a
  deterministic error if it appears in an artifact, per DESIGN.md §5.5's
  no-inert-syntax rule.

### 2. Stamp the design documents

In `docs/design/option-d-architecture.md`, extend the `**Status:**` line (do
not delete the existing text) with:

```text
Accepted 2026-07-21 as amended by docs/prds-composition/README.md and
DECISIONS.md §J; the README's amendments win over this document.
```

In `docs/archive/design/composition-options-2026-07-21.md`, extend its `**Status:**` line with a
sentence noting that Option D was selected and recorded in DECISIONS.md §J,
and that `option-d-architecture.md` plus `docs/prds-composition/README.md`
supersede this note's open decisions.

### 3. What not to do

Do not renumber, reword, or delete any existing DECISIONS.md section. Do not
edit DESIGN.md in this PRD. Do not touch any code, fixture, script, or
example. Do not paraphrase version strings, identity grammar, or hash
constructions in a way that changes them — copy them exactly; later PRDs
implement against these strings.

## Allowed files

- `DECISIONS.md` (append section J only)
- `docs/design/option-d-architecture.md` (Status line only)
- `docs/archive/design/composition-options-2026-07-21.md` (Status line only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Any code, schema, fixture, or test change.
- Resolving doc open questions beyond those the README already resolves.
- Editing DESIGN.md (a pointer amendment lands in PRD 0012).

## Acceptance criteria

1. `grep -n '^## J\.' DECISIONS.md` and `grep -n '^### J12\.' DECISIONS.md`
   both match; subsections J1–J12 all exist with the subjects listed above.
2. J3, J4, J5, and J6 contain the exact frozen strings from the folder README:
   spot-check `sembla.rule-word/v1`, `sembla.identity/legacy-positional-v1`,
   `occ:north/population#infect`, and `sembla.canonical-json/v1` all appear in
   DECISIONS.md.
3. Both design docs' Status lines mention DECISIONS.md §J and the PRD folder
   README.
4. `git diff --stat` shows changes to exactly the three allowed documentation
   files.
5. `./scripts/check.sh` passes and `git diff --check` is clean.

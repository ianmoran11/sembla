# Composition (Option D) PRDs

Ordered PRD set implementing the **first composition release** of
[`docs/design/option-d-architecture.md`](../design/option-d-architecture.md)
(rollout Phases 0–4: decisions, plan envelope and stable identity, composition
source, canonical Lean linker with product/wires/nesting, formal spec
statements, surface syntax, and the artifact bundle). Run it from the Sembla
repository with:

```text
/piprd run docs/prds-composition
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this file
first; the constraints below are binding. When a PRD conflicts with this
README, this README wins; when this README conflicts with
`option-d-architecture.md`, this README is the accepted amendment and wins.

## Authority and scope

- `docs/design/option-d-architecture.md` is the architecture authority.
  `docs/archive/design/composition-options-2026-07-21.md` is background. This README plus PRD
  0001's decision record are the accepted, amended form of both.
- `DESIGN.md` §4 (semantics), §5 (determinism, manifest, flags) and
  `DECISIONS.md` §§A5, E1, E7, I1–I6 bind everything here.
- This track targets doc Phases 0–4 only. **Deferred and out of scope for the
  whole folder:** synchronized transition families, `Share`/`Identify`,
  semantic invariants/constrained products, heterogeneous schedulers, explicit
  adapters and merge components, dynamic component topology, ACSet storage,
  any Julia dependency, a non-Lean linker, and CUDA corpus extension.
- Existing artifacts are **byte-frozen compatibility contracts**: every file in
  `examples/*.json` and `examples/invalid/*.json`, all fixed-seed CSV goldens,
  state/output hashes for legacy runs, the negative-suite expectations, and
  the canonical-export parity checks in `frontend/scripts/check-parity.sh`.
  No PRD may regenerate or weaken them. Legacy unversioned model JSON keeps
  its current dense declaration-order rule IDs and behavior forever, under the
  explicit scheme name `sembla.identity/legacy-positional-v1`.

## Amendments to option-d-architecture.md (accepted here)

The architecture doc marks its exact strings and constructions as
illustrative. This run fixes them as follows; PRD 0001 records each in
`DECISIONS.md`.

1. **Hash algorithm is SHA-256**, not blake3. `sha256` is already the
   project's manifest hash algorithm and `sha2` is already an approved Rust
   dependency; adding blake3 would violate the dependency policy in
   `scripts/check.sh`. Hash records are `{algorithm: "sha256", domain, digest}`
   with lowercase hex digests.
2. **Occurrence IDs are structural chains, not hashes.** The Lean linker needs
   no cryptography to build identities (only to compress rule words). See the
   identity grammar below.
3. **Hashes live in the bundle manifest and run manifest, never inside the
   plan file.** This removes every self-reference rule from the doc's §5.3.
4. **RNG strategy (doc open question 2):** keep the Philox layout
   `[tick, rule_word, entity_id, draw_idx]` and map stable transition
   identities into the existing `u32` word content-addressedly, with reserved
   namespaces excluded and collisions rejected at link/validation time. The
   coordinate is not widened.
5. **Normalized-legacy plan origin is deferred.** Only two envelope origins
   exist in V1: `linked` and `direct_stable`. Raw unversioned model JSON is
   the third, envelope-free `legacy` path.
6. **Parameter bindings (doc open question 5):** an instance binds each
   component parameter requirement to a **model-level parameter name only**
   (never a literal). Two instances may bind the same model parameter; that
   is explicit sharing. Distinct per-instance values require two model
   parameters. This keeps θ, `Param` resolution, and the runtime unchanged.
7. **`dt` (doc open question 6):** components never declare `dt`. The root
   composition declares `outer_dt`; V1 has exactly one scheduler domain
   (`domain:global`, algorithm `tau_leap`) containing every leaf.
8. **Product symmetry (doc open question 4):** plan collections are sorted by
   stable identity, so symmetry/associativity/alpha-renaming laws are
   **byte-equality of plan cores**, not isomorphism arguments.
9. **Source-map fields (doc open question 7):** the source map and all display
   names are excluded from the plan semantic hash; identity maps are included.

## Frozen version strings

| Concern | String |
|---|---|
| Composition source schema | `sembla.composition-source/v1` |
| Executable plan schema | `sembla.executable-plan/v1` |
| Linker semantics | `sembla.linker/v1` |
| Stable identity scheme | `sembla.identity/stable-v1` |
| Legacy identity scheme | `sembla.identity/legacy-positional-v1` |
| Canonical encoding | `sembla.canonical-json/v1` |
| Source map schema | `sembla.source-map/v1` |
| Hash domains | `sembla.source-artifact/v1`, `sembla.plan-core/v1`, `sembla.plan-envelope/v1`, `sembla.bundle-root/v1`, `sembla.rule-word/v1` |

Unknown or missing required version strings are always rejected with a
deterministic error, never interpreted by best effort. `required_features`
and `enabled_features` must be present and exactly `[]` in V1; any entry is a
deterministic rejection naming the feature.

## Frozen identity grammar

- **Slug:** `[a-z][a-z0-9_]*` (ASCII; matches existing runtime snake_case
  names). No leading digit or underscore.
- **Stable declaration ID:** `<kind>:<slug>` with kinds `model`, `def`,
  `inst`, `port`, `wire`, `expose`. Examples: `def:population`,
  `inst:north`, `port:infection_count`, `wire:count_to_policy`.
- **Transition local ID:** the transition's existing runtime `name` slug
  (already stable and referenced by `fired:` columns).
- **Occurrence ID:** `occ:` followed by the slash-joined chain of instance-ID
  slugs from the root definition. Depth 1: `occ:population`. Nested:
  `occ:epidemic/population`, `occ:north/population`. The root definition
  itself is the empty chain `occ:`. Chains are built from instance
  **declaration IDs**, never display names, and never traversal positions.
- **Transition occurrence identity:** `<occurrence-id>#<transition-name>`,
  e.g. `occ:population#infect`, `occ:north/population#infect`.
- **Wire occurrence identity:** `<owner-occurrence>#wire:<wire-slug>`, e.g.
  `occ:#wire:count_to_policy` for a root-owned wire and
  `occ:north#wire:count_to_policy` for the same wire inside instance `north`.
- **Mailbox identity:**
  `mbox:<wire-occurrence>|<source-occurrence>.<port-slug>|<target-occurrence>.<port-slug>`.
  Including both endpoints disambiguates fan-out.
- **Plan leaf name:** the occurrence chain slugs joined by `/`
  (`population`, `epidemic/population`). Slugs contain no `/` or `.`, so this
  cannot collide with existing `box.table.attr` report naming. Display names
  are non-semantic and live only in the source map.

**Rule word derivation (the RNG contract):**

```text
rule_word = big-endian u32 of the first 4 bytes of
  SHA-256( "sembla.rule-word/v1" ++ 0x00 ++ transition-occurrence-identity )
```

Words equal to `u32::MAX - 1` or `u32::MAX` (reserved sweep/prior namespaces
in `crates/sembla-runtime/src/rng.rs`) and any collision between two accepted
identities are deterministic link/validation errors — never silently
reassigned. The documented remedy for a collision is renaming a stable ID.
Because the word is a pure function of the identity, inserting or removing an
unrelated sibling never changes any other transition's draws — this is the
headline property and PRD 0004/0007 encode it as tests.

**Domain-separated hashing:** every hash in this track is
`SHA-256(domain-string ++ 0x00 ++ payload-bytes)` with the domain from the
table above, except raw `sha256` of exact file bytes where a PRD explicitly
says so.

## Frozen canonical JSON encoding (`sembla.canonical-json/v1`)

Applies to every **new** artifact (composition source, executable plan,
bundle manifest). It does not apply to legacy `examples/*.json`.

- UTF-8, no BOM, **no insignificant whitespace**, no trailing newline; the
  file bytes are exactly the canonical bytes.
- Object keys sorted by byte-wise lexicographic order. Optional absent fields
  are **omitted**, never `null`. Related optional fields form
  all-present-or-all-absent tuples; readers reject partial tuples.
- Arrays: plan collections sorted by stable identity (leaves by occurrence
  path, transitions by `(leaf, name)`, wires/mailboxes by identity string);
  source collections preserve author order.
- Strings: escape only `"`, `\`, and control characters (`\b \t \n \f \r`,
  else `\u00xx` lowercase); all other characters literal UTF-8.
- Numbers: integers in plain decimal without leading zeros or `+`; non-integer
  numerics reuse the exact printing conventions of the existing Lean canonical
  model writer (`frontend/Sembla/Json.lean`) so `dt: 0.25` prints identically
  from Lean and round-trips through `serde_json` (`float_roundtrip` is already
  enabled).
- Versioned parsers reject unknown fields (`deny_unknown_fields` in serde;
  explicit checks in Lean).

## The frozen worked example

All fixtures across the folder use one family of models so every PRD's
artifacts compose. Primitive bodies are **adapted from the two boxes of
`examples/sir_policy.json`** (already-valid IR bodies) with `rows` reduced to
test scale (Person 1000, Employer 50, Controller 1).

- `def:population` — ports: output `port:infection_count` schema
  `[infected : Int]` from Person; input `port:restriction_modifier` schema
  `[restriction : Real]`. Parameter requirements: exactly the `Param` names
  its transitions reference. Views `S`, `I`, `R`.
- `def:policy` — ports: input `port:infection_count` schema
  `[infected : Int]`; output `port:restriction_modifier` schema
  `[restriction : Real]` from Controller.
- `def:independent_epidemic_policy` — composite: instances `inst:population`,
  `inst:policy`; **no wires, no exposures** (the product `Population ⊗ Policy`).
- `def:epidemic_policy` — composite: the two instances plus wires
  `wire:count_to_policy` (`population.infection_count →
  policy.infection_count`) and `wire:restriction_to_population`
  (`policy.restriction_modifier → population.restriction_modifier`),
  `delay_ticks: 1` each.
- `def:two_regions` — composite: instances `inst:north`, `inst:south`, both
  of `def:epidemic_policy`; no wires of its own.
- `def:regional_response` — composite: instance `inst:epidemic` of
  `def:epidemic_policy`; exposes `epidemic.infection_count` as outer port
  `port:regional_infection_count`; hides `epidemic.restriction_modifier`
  (both only meaningful once `def:epidemic_policy` itself exposes those ports
  — PRD 0009 adds those exposures to its fixture variant).

Stable IDs in hand-authored fixtures **must** use exactly the slugs above,
because PRD 0011's Lean surface derives the same slugs from identifiers and
the twin-conformance test compares plan-core bytes.

## Required checks for every PRD

From the repository root, each implementation and review must run and pass:

```bash
./scripts/check.sh
cd frontend && lake build
bash frontend/scripts/check-parity.sh
git diff --check
```

`./scripts/check.sh` already includes `cargo fmt --check`, clippy with
`-D warnings`, all Rust tests, proof hygiene, and the Lean parity harness.
PRDs that touch surface syntax must also run
`bash frontend/scripts/test-negative.sh`. Any PRD whose diff would change a
byte of `examples/*.json` or an existing golden has failed by definition.

## Global non-goals

- No new Rust dependencies except `sha2` added to `crates/sembla-ir`
  (PRD 0003). No new Lean dependencies at all (no crypto library: PRD 0002
  implements SHA-256 in pure Lean). No blake3 anywhere.
- No changes to Philox, `exp_f64` sampling, conflict argmin semantics, effect
  application, or numeric behavior. Only the *rule word fed to Philox and
  tie-breaks* becomes plan-supplied for versioned plans.
- No behavior widgets, no calibration/NPE changes, no CUDA work, no
  population-format changes.
- No editing of `.piprd/`, CI workflows, or external vault copies.

## Run order

1. `0001-decision-record.md` — record the accepted Option D decisions in
   `DECISIONS.md` and stamp the design docs.
2. `0002-lean-sha256.md` — pure-Lean SHA-256, domain-separated hash records,
   rule-word derivation, cross-language known-answer constants.
3. `0003-plan-envelope-rust.md` — versioned `ExecutablePlanV1` envelope,
   identity map, and validation in `sembla-ir`; explicit legacy branch.
4. `0004-runtime-identity-and-manifest.md` — plan-supplied rule words drive
   Philox and tie-breaks; CLI accepts envelopes; run-manifest identity tuple.
5. `0005-lean-plan-export.md` — Lean plan envelope emission; `direct_stable`
   exports of existing canonical models; cross-language hash parity.
6. `0006-composition-source-schema.md` — `CompositionSourceV1` types, canonical
   JSON round-trip, and the hand-authored fixture corpus.
7. `0007-linker-product.md` — canonical linker stages for definitions,
   instances, parameters, and product; occurrence identities; `sembla-link`.
8. `0008-linker-wires.md` — delayed wires, mailbox identities, single-driver
   and schema validation; delay-trace and twin-plan tests.
9. `0009-linker-nesting.md` — exposure, hide, rename, visibility enforcement,
   flattening equivalence, alpha-rename/permutation byte-invariance.
10. `0010-formal-spec.md` — observation contract, independent denotation
    skeletons, preservation statements that typecheck.
11. `0011-component-syntax.md` — `sembla_component`/`sembla_composition`
    surface syntax, source export, Lean/hand-authored twin conformance.
12. `0012-bundle-and-docs.md` — bundle manifest with integrity hashes, linked
    provenance in run manifests end-to-end, documentation and acceptance sweep.

Later PRDs depend on every earlier PRD. Do not combine or reorder them, and
do not implement a later PRD's constructs early "while you are in there" —
each PRD's linker/validator must reject constructs from later PRDs with a
deterministic error naming the construct.

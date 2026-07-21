# PRD 0007: Canonical linker — definitions, instances, parameters, product

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. Sources
(PRD 0006) and plans (PRDs 0003–0005) both exist; this PRD builds the bridge:
the canonical Lean linker `linkV1`, covering definition resolution, instance
expansion, parameter binding, occurrence identity, and unwired composition
(product). Wires land in PRD 0008 and exposure/hiding in PRD 0009 — sources
containing those constructs must be **rejected here** with a deterministic
`unsupportedConstruct` error (README rule: later-PRD constructs are never
half-implemented early).

Key semantics restated (DECISIONS §J3/§J4, architecture doc §7.4/§10.2):

- Expansion walks from the root definition. Each composite instance extends
  the occurrence chain by its instance-id slug; a leaf's occurrence is
  `occ:<chain>` (slugs joined by `/`), and its plan box name is the chain
  itself (`north/population`).
- Two instances of one definition yield disjoint occurrence chains and
  therefore disjoint transition identities and rule words. Rule words are
  content-addressed; the linker never assigns from a counter.
- The linker is a pure function of the parsed source value: no filesystem
  order, no name-derived ordering, no randomness. Errors are collected where
  safe and sorted deterministically.

## Goal

`linkV1` lowers wire-free sources to validated `ExecutablePlanV1` values with
origin `linked`, full identity maps, source hash, linker descriptor, and a
minimal source map; a `sembla-link` executable emits canonical plan + report;
golden linked plans validate and run under Rust; the product-noninterference
law holds on draws, states, and views.

## Specification

### 1. Error model — `frontend/Sembla/Composition/Errors.lean`

```lean
inductive LinkErrorCodeV1 where
  | unknownVersion | unsupportedFeature | unsupportedConstruct
  | duplicateStableId | missingDefinition | recursiveDefinition
  | missingPort | inaccessibleDescendantPort
  | directionMismatch | schemaMismatch
  | multipleDrivers | unboundParameter | ambiguousParameterBinding
  | identityCollision | reservedRuntimeIdentity
  | invalidSummary
  deriving Repr, BEq, Ord

structure LinkErrorV1 where
  code : LinkErrorCodeV1
  message : String        -- human text; may improve between versions
  primary : StableId      -- the id the error anchors to
  related : List StableId
  deriving Repr, BEq
```

Codes are stable API within `sembla.linker/v1`; the full inductive is defined
now even though `missingPort`…`multipleDrivers` fire only from PRD 0008/0009.
Error lists returned by `linkV1` are sorted by `(code, primary, message)` —
never by discovery order.

### 2. Linker — `frontend/Sembla/Composition/Link.lean`

```lean
structure LinkResultV1 where
  plan : Plan.ExecutablePlanV1
  report : LinkReportV1          -- warnings + statistics; never semantic

def linkV1 (src : CompositionSourceV1) (sourceCanonicalBytes : String) :
    Except (List LinkErrorV1) LinkResultV1
```

(The canonical source bytes are passed in so the linker can embed the source
hash without re-encoding — the caller renders once, hashes once, links once.)

Stages, in order; a stage that would dereference invalid structure stops the
pipeline, otherwise collect errors across independent items:

1. **Envelope validation.** `wellFormed` (PRD 0006) is assumed already run by
   the parser, but re-check `schemaVersion` and `requiredFeatures == []`
   (`unknownVersion` / `unsupportedFeature`).
2. **V1 construct gate.** Any composite containing `wires`, `exposures`, or
   `hiddenPorts` → one `unsupportedConstruct` error per offending declaration
   naming the construct and the PRD that adds it. (PRDs 0008/0009 relax this
   gate construct by construct.)
3. **Definition collection.** Index definitions by id; duplicate →
   `duplicateStableId`. Missing `rootDefinition` → `missingDefinition`.
4. **Dependency analysis.** Resolve every `InstanceDeclV1.definition`;
   missing → `missingDefinition`. Detect definition-reference cycles by DFS
   from the root over definition ids → `recursiveDefinition` (report the
   cycle's ids in `related`).
5. **Parameter resolution.** For each instance of a primitive definition:
   every `parameterRequirement` must be bound exactly once to an existing
   model parameter (`unboundParameter` if unbound or the model parameter
   does not exist; `ambiguousParameterBinding` for duplicate bindings of one
   requirement, extra bindings naming unknown requirements, or a binding
   whose model parameter's type mismatches the use — type check may be
   deferred to IR validation if precise, but reject unknown names here).
   Composite instances carry bindings down: a composite's instances bind
   against the same single model-level parameter namespace in V1 (composites
   do not declare their own requirements in V1 fixtures; if a composite
   declares `parameterRequirements`, reject with `unsupportedConstruct` —
   record this simplification in the report statistics).
6. **Instance expansion.** Recursively expand the root: composite instances
   extend the chain; primitives produce leaves. For each leaf, instantiate
   the primitive body: copy tables/inputs/outputs/views verbatim; rewrite
   every `Param` reference in transitions/outputs/views from requirement name
   to the bound model parameter name via a **total** structural traversal of
   `IR.Expr` (match every constructor explicitly with no wildcard, so a
   future IR constructor breaks the build here rather than silently passing
   through). Box name = occurrence chain (`population`,
   `north/population`).
7. **Identity construction.** Leaves `occ:<chain>`; transitions
   `occ:<chain>#<name>`; rule words via `Sembla.Hash.ruleWord`. Reserved
   word → `reservedRuntimeIdentity`; duplicate word across accepted
   identities → `identityCollision` listing both identities in
   `related` (documented remedy: rename a stable id).
8. **Summary resolution.** Each `SourceSummaryV1.instancePath` must resolve
   to an expanded leaf occurrence and its `view` must exist on that leaf →
   plan summary over `<chain>.<view>`; otherwise `invalidSummary`.
9. **Flat model assembly.** `IR.Model` with `name := slug of modelId`
   (strip `model:`), `dt := outerDt`, `params := parameters`, boxes from
   leaves, no wires (this PRD), summaries from stage 8. Apply canonical
   ordering (PRD 0005 §2's sort rules).
10. **Plan assembly.** Origin `linked`; identity map as PRDs 0003/0005;
    `linked_provenance` with
    `source_hash := hashRecord "sembla.source-artifact/v1" sourceBytes`,
    the frozen `LinkerDescriptorV1`, and the source map (§3).
11. **Final validation.** Run a Lean-side plan validity check
    `planValidCheck : ExecutablePlanV1 → Bool` mirroring PRD 0003 §4 rules
    (at minimum: bijections, sortedness, word distinctness/reservation,
    features empty). `linkV1` returns `.ok` **only if** this check passes;
    a failure is a linker bug surfaced as a deterministic internal error.
    Structure the code as `if planValidCheck plan then .ok … else .error …`
    so PRD 0010 can state validity-by-construction cheaply.

### 3. Source map — `frontend/Sembla/Composition/SourceMap.lean`

Minimal V1 (this is the `source_map` value inside `linked_provenance`;
schema string `sembla.source-map/v1`):

```json
{
  "schema_version": "sembla.source-map/v1",
  "leaves": [ { "occurrence": "occ:north/population",
                "definition": "def:population",
                "instance_path": ["inst:north", "inst:population"],
                "display_path": "north/population" } ],
  "boundary": [],
  "hidden": []
}
```

`leaves` sorted by occurrence. `boundary`/`hidden` are populated in PRD 0009
but present (empty) from day one so the plan schema doesn't change.
Display names/paths live only here and never affect the semantic hash
(DECISIONS §J10 — provenance is excluded from it by construction).

### 4. `sembla-link` executable

New `frontend/LinkMain.lean` + lakefile `lean_exe sembla-link` (add to
`defaultTargets`):

```text
sembla-link <source.json> --plan <out.plan.json> [--report <out.report.json>]
```

Reads the file, parses (PRD 0006 parser — accepts non-canonical formatting),
re-renders canonically, hashes, links, writes the canonical plan bytes.
Errors print one line each, sorted:
`link error <code> at <primary>: <message>` and exit nonzero. The report is
non-semantic JSON `{warnings: [...], statistics: {leaves: n, transitions: n,
mailboxes: n}}`; it is never hashed and never required to interpret the plan.

### 5. Fixtures, goldens, and tests

- **Linked goldens.** Link `solo_population`, `independent_epidemic_policy`,
  and `two_independent_regions` sources; check in
  `fixtures/plans/linked/<name>.plan.json`. Append a delimited parity
  section to `frontend/scripts/check-parity.sh`: run `sembla-link` on each
  checked-in source fixture and `cmp` the plan output.
- **Rust acceptance.** Extend the PRD 0005 walking test (or add one) so every
  `fixtures/plans/linked/*.plan.json` passes `sembla validate` including
  canonicality — Rust must accept `linked` provenance now; if PRD 0003's
  validator needs its linked-branch rules exercised/adjusted (descriptor
  strings, source-map opacity), do it here **without** weakening any check.
- **Noninterference (the law).** CLI/runtime test: run
  `linked/solo_population.plan.json` and
  `linked/independent_epidemic_policy.plan.json` at the same seed/ticks and
  assert the population leaf's per-tick view columns, fired columns, and
  deferred counts are **bitwise identical**. This works because both plans
  contain the same `occ:population#…` identities → same words → same draws.
  Also assert `two_independent_regions`' four leaves have four disjoint
  occurrence chains and pairwise-distinct rule words (Lean `#guard`).
- **Lean linker tests** (`frontend/Sembla/Composition/LinkTests.lean`):
  error cases built by mutating fixture values in Lean — duplicate def id,
  missing definition, a two-definition cycle, unbound parameter, duplicate
  binding, unknown binding target, summary to missing leaf/view, wire
  present (→ `unsupportedConstruct` naming PRD 0008), exposure present
  (→ naming PRD 0009); each `#guard`s the exact sorted error-code list.
  Determinism: link the same value twice, `#guard` byte-equal rendered
  plans; permute `definitions` list order, `#guard` byte-equal plans
  (definition order is not semantic).

## Allowed files

- `frontend/Sembla/Composition/Errors.lean`, `Link.lean`, `SourceMap.lean`,
  `LinkTests.lean` (new); `Fixtures.lean` only if a test needs a small
  mutated variant helper
- `frontend/LinkMain.lean` (new), `frontend/lakefile.toml`,
  `frontend/Sembla.lean`
- `frontend/scripts/check-parity.sh` (append delimited section only)
- `fixtures/plans/linked/**` (new)
- `crates/sembla-cli/tests/**`, `crates/sembla-ir/src/plan.rs`/`validate.rs`
  (linked-branch acceptance only, no weakened checks),
  `crates/sembla-ir/tests/**`
- implementation notes/artifacts created by the managed run

## Non-goals

- Wires/mailboxes (PRD 0008); exposure/hide/visibility (PRD 0009).
- Composite-declared parameter requirements, renames, or any §J12 construct.
- Denotational semantics or theorems (PRD 0010).
- Surface syntax (PRD 0011); bundles (PRD 0012).
- Changing plan schema, hash payloads, or any golden from PRDs 0003–0005.

## Acceptance criteria

1. `lake build` + full extended parity script pass: the three linked goldens
   are byte-reproduced from their checked-in sources by `sembla-link`.
2. Every `fixtures/plans/linked/*.plan.json` passes `sembla validate`
   (including canonicality) and carries a complete linked-provenance tuple
   whose `source_hash` digest equals SHA-256 (domain
   `sembla.source-artifact/v1`) of the checked-in source bytes — pinned by a
   Rust test.
3. The noninterference test passes bitwise on views, fired, and deferred
   columns for the population projection.
4. Repeated-composite distinctness: `two_independent_regions` has 4 leaves,
   disjoint chains, pairwise-distinct words (Lean guards).
5. Every listed error case yields exactly its expected sorted code list;
   wire/exposure sources are rejected with `unsupportedConstruct` naming the
   later PRD.
6. Linking is deterministic and definition-order-insensitive (byte-equality
   guards).
7. `./scripts/check.sh` and `git diff --check` pass; no legacy golden or
   example changed.

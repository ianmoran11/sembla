# PRD 0009: Linker nesting — exposure, hiding, visibility, byte-invariance laws

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. The
linker handles product (PRD 0007) and wires between direct primitive children
(PRD 0008). This PRD completes the first-release composition algebra:
composite nesting with **exposure** (zero-delay boundary aliasing), **hiding**
(boundary-port removal), and the visibility rules that make composites real
abstraction boundaries. It also lands the strongest law tests in the folder:
alpha-renaming and declaration-permutation byte-invariance of linked plans.

Semantics restated (architecture doc §8/§9, DECISIONS §J3/§J10):

- A composite's boundary ports exist only via exposures. Exposing an inner
  port creates a typed alias — **no mailbox, no tick, no state**.
- A parent may wire/expose/hide only a **direct child's boundary port**;
  reaching through to an unexposed descendant is `inaccessibleDescendantPort`.
- Exposure chains resolve transitively: wiring to a composite's boundary port
  ultimately lands on a leaf occurrence's port; the mailbox belongs to the
  wire, endpoints named by the resolved leaf occurrences.
- Hiding removes a boundary port from the composite's public interface; the
  owner may not also wire or expose that hidden port. Hidden inputs read
  empty (V1 rule; unwired inputs are legal).
- Display renames do not exist as a construct; display names are non-semantic
  and never reach the plan core.
- Wrapping an instance in a new named composite **changes** occurrence chains
  (and therefore words) by design — that is not a violation of any law; the
  laws below are about renames and permutations, which must be invisible.

## Goal

`two_regions` and `regional_response` link to golden plans; visibility and
hidden-port violations reject deterministically; exposure adds no mailbox and
no delay (proven by trace); alpha-rename and permutation invariance hold as
byte-equality of linked plans.

## Specification

### 1. Boundary resolution in the linker

Remove the PRD 0008 gate on `exposures`/`hiddenPorts`. Implement:

- **Composite boundary construction.** A composite definition's effective
  boundary ports are its exposures' `outerPort`s. Validate per composite:
  outer port ids unique (`duplicateStableId`); `innerInstance` is a direct
  child; `innerPort` exists on that child's boundary (its definition ports
  for a primitive; its exposed boundary for a composite child) —
  `missingPort` otherwise; direction is inherited from the inner port.
  Exposing a port of a *grandchild* directly is impossible by construction
  (only direct-child ids are in scope) — but a wire or exposure that names a
  composite child's **unexposed** port must produce
  `inaccessibleDescendantPort` (distinct from `missingPort`: the port exists
  below but is not exposed; the message must say which composite failed to
  expose it).
- **Hidden ports.** Each `HiddenPortV1` names a direct child instance and
  one of that child's boundary ports (`missingPort` if absent). A port both
  hidden and wired, or hidden and exposed, by the same owner →
  deterministic error (reuse `duplicateStableId`? No — add nothing: use
  `directionMismatch`? No). Extend `LinkErrorCodeV1` with one code
  `hiddenPortConflict` for exactly this (the inductive gains a constructor;
  this is the one permitted error-model change in this PRD).
- **Transitive resolution.** `resolveBoundary : (child instance, port) →
  (leaf occurrence, port name)` follows exposure links downward through
  arbitrarily many composite levels. Wires lower to flat `IR.Wire`s between
  resolved leaf boxes exactly as in PRD 0008; the mailbox identity uses the
  wire's own occurrence (owner chain + wire slug) and the **resolved leaf**
  endpoint occurrences.
- **Root boundary.** Exposures on the root definition are legal and appear
  in the source map's `boundary` section; they produce no plan objects
  (there is no outer parent to consume them). Hidden root ports likewise go
  to `hidden`.

### 2. Source map completion

Populate the `boundary` and `hidden` sections defined (empty) in PRD 0007:

```json
"boundary": [ { "outer": "port:regional_infection_count",
                "leaf": "occ:epidemic/population",
                "port": "infection_count",
                "path": ["expose:regional_count", "expose:epidemic_count"] } ],
"hidden":   [ { "instance": "inst:epidemic", "port": "port:restriction_modifier" } ]
```

(`path` lists the exposure ids traversed, outermost first.) Sorted by
`outer`/`(instance, port)` respectively.

### 3. Fixtures and goldens

Using the `epidemic_policy_exposed` definition from PRD 0006 (which exposes
`population.infection_count` as boundary `port:infection_count` and
`policy.restriction_modifier` as `port:restriction_modifier`):

- Link `two_regions` → `fixtures/plans/linked/two_regions.plan.json`:
  4 leaves (`north/population`, `north/policy`, `south/population`,
  `south/policy`), 4 mailboxes (two per region, distinct wire occurrences
  `occ:north#wire:count_to_policy` vs `occ:south#wire:count_to_policy`),
  root summary resolved onto `north/population.I`.
- Link `regional_response` → `…/regional_response.plan.json`: 2 leaves
  (`epidemic/population`, `epidemic/policy`), exactly 2 mailboxes (the
  exposure adds none), boundary + hidden sections populated.
- Add and link `wrapped_ping_pong`: `def:wrapped_ping` exposing
  `ping.pulse`, `def:wrapped_pong` exposing `pong.pulse` (input), and
  `def:wrapped_ping_pong` wiring `wping.pulse -> wpong.pulse` across the two
  composite boundaries. 2 leaves, **1 mailbox**.
- Extend the parity section for the new sources and linked plans; extend the
  Rust walking validate test.

### 4. Law and behavior tests

1. **Exposure adds no delay.** Run linked `wrapped_ping_pong` and linked
   flat `ping_pong` (PRD 0008): `seen_yes` flips at tick 1 in **both** CSVs
   (leaf names and words differ — chains differ — but the deterministic
   ping/pong construction makes the trace shape identical; that is why these
   fixtures are hazard-`1e300` deterministic). Mechanical assertion on both
   CSVs.
2. **Occurrence distinctness.** Lean `#guard`s: `two_regions` yields 4
   distinct chains, 8 distinct transition identities, pairwise-distinct
   words; north and south mailbox identities differ.
3. **Alpha-rename invariance (byte law).** In `Fixtures.lean`, build a
   variant of `two_regions` changing **every `displayName`** (definitions,
   instances, ports) to arbitrary different strings, keeping all stable ids.
   `#guard`: rendered linked plan bytes are **identical** except inside
   `linked_provenance.source_map` — implement by comparing the two plans
   with `source_map` blanked, and additionally compare their semantic-hash
   inputs byte-for-byte (they must be equal; display names appear nowhere in
   the core).
4. **Permutation invariance (byte law).** Variants permuting the order of
   `definitions`, of `instances` within a composite, of `wires`, and of
   `exposures`: linked plan bytes identical (full equality including source
   map, which is sorted). This is DECISIONS §J10 made executable.
5. **Instance-rename identity change (negative law).** Rename instance id
   `inst:north` → `inst:west` (a *semantic* refactor): `#guard` the semantic
   hash **changes**. This pins that stable-id renames are model changes, per
   the architecture doc §7.4.
6. **Visibility negatives** (extend `LinkTests.lean`, pinned code lists):
   wire targeting a composite child's unexposed port
   (`inaccessibleDescendantPort` naming the composite); exposure of a
   missing child port (`missingPort`); duplicate outer port ids; hidden +
   wired conflict (`hiddenPortConflict`); hidden + exposed conflict; wire to
   a hidden boundary port (`inaccessibleDescendantPort` or
   `hiddenPortConflict` — choose one, document it in the error message, and
   pin it); exposure direction mismatch when wiring
   (input exposed, used as wire source → `directionMismatch`).

### 5. Run goldens

CLI run goldens (CSV + hashes, fixed seed) for `two_regions` and
`regional_response`, following PRD 0004 §5 conventions. Also assert in the
`two_regions` run that north and south region columns **differ** at the
chosen seed (distinct words ⇒ independent draws — a sanity check that the
regions are not accidentally correlated; pick a seed/tick count where they
visibly diverge and pin the golden).

## Allowed files

- `frontend/Sembla/Composition/Link.lean`, `Errors.lean` (the one new
  constructor), `SourceMap.lean`, `Fixtures.lean`, `LinkTests.lean`,
  `SourceTests.lean`
- `frontend/Sembla.lean`, `frontend/scripts/check-parity.sh` (append only)
- `fixtures/composition-source/**`, `fixtures/plans/linked/**`,
  `fixtures/plans/goldens/**`
- `crates/sembla-cli/tests/**`, `crates/sembla-ir/tests/**` (walking/run
  tests only; validator logic changes only if a linked-form check from
  PRDs 0003/0008 needs the boundary/hidden source-map fields accepted —
  never weakened)
- implementation notes/artifacts created by the managed run

## Non-goals

- Renaming constructs, adapters, merges, families, invariants, schedulers
  (§J12).
- Runtime changes of any kind.
- Making wrapping-in-a-composite identity-preserving — it is not, by design.
- Cross-boundary reach-through convenience of any kind.

## Acceptance criteria

1. `lake build` + extended parity: `two_regions`, `regional_response`, and
   `wrapped_ping_pong` linked goldens byte-reproduce from sources; all pass
   Rust validation + canonicality.
2. `regional_response` and `wrapped_ping_pong` plans contain exactly 2 and 1
   mailboxes respectively (exposure allocated none) — asserted by test, and
   the no-added-delay CSV law (§4.1) passes.
3. Byte laws hold: display-rename invariance (core + semantic-hash input
   equality), permutation invariance (full plan equality), and stable-id
   rename divergence, all as Lean guards.
4. Occurrence/word distinctness guards for `two_regions` pass; run goldens
   land and reproduce bitwise, with north/south divergence pinned.
5. Every visibility negative yields its pinned deterministic code list;
   `hiddenPortConflict` is the only error-model addition.
6. `./scripts/check.sh` and `git diff --check` pass; no legacy artifact
   changed.

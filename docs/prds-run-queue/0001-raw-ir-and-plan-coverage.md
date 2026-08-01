# PRD 0002: Stabilize raw IR/plan coverage

## Dependencies

PRD 0001 accepted.

Binding contract: [track README](../prds-lean-ir-formalization/README.md).

## Context

The serialization-friendly raw contract covered by this PRD is exactly the
public structures, their fields and the inductive constructors declared in:

- `frontend/Sembla/IR.lean`;
- `frontend/Sembla/Composition/Source.lean`;
- `frontend/Sembla/Composition/SourceMap.lean`; and
- `frontend/Sembla/Plan.lean`.

This includes the raw model, hierarchical composition source, source-map
provenance, plan origin, plan identity/provenance records and
`ExecutablePlanV1`. It excludes canonical-JSON syntax/encoders, hash
implementations, linker algorithms/errors and bundle artifact containers.
Version constants that govern covered version fields are evidence for those
fields, but are not themselves fields or constructors.

The track cannot claim completeness without an exhaustive owned inventory.

## Goal

Create a machine-checked constructor/field classification and raw fixtures
without changing public structures or bytes.

## Requirements

1. Add `Sembla.Semantics.Raw` as imports/aliases and exhaustive classifiers,
   avoiding duplicate raw definitions.
2. Add `docs/design/lean-ir-coverage.md` covering every current structure field
   and inductive constructor in the exact four-module boundary above, including
   `SourceMapLeafV1`, `SourceMapBoundaryV1`, `SourceMapHiddenV1` and
   `SourceMapV1`.
3. Give each item PRD 0002 as its **raw inventory owner**, plus exactly one
   **meaning/invariant owner** and any later theorem dependencies. The latter is
   either a numbered PRD in this track or the explicitly deferred future
   composition formalization track; do not invent current-track composition
   checker or semantic obligations.
4. Record two independent classifications for every item:
   - foundational role: semantic, structural, observational or
     provenance-only; and
   - frontend/checking status: surface-produced, raw-only accepted,
     contextually rejected or deferred composition input.
5. Add compile-time coverage guards that fail when either a covered inductive
   gains an unclassified constructor or a covered structure gains an
   unclassified field. Structure guards must depend on complete constructor
   arity or equivalent environment metadata; a named-field pattern that can
   silently omit a new field is insufficient.
6. Add raw fixtures covering all variants, including priors, nested aggregates,
   grouped views, multi-claim transitions, key-ordering raw syntax, both
   `PortDirection` and `ComponentBodyV1` constructors, populated bindings,
   wires, exposures and hidden ports, every source-map section, both plan
   origins, optional provenance combinations, source summaries and version
   fields.
7. Preserve current canonical exports and bytes. Stop on a raw-contract
   discrepancy.

## Allowed files

- `frontend/Sembla/Semantics/Raw.lean`
- `frontend/Sembla/Semantics/RawTests.lean`
- `frontend/Sembla/Semantics.lean`
- `frontend/Sembla.lean`
- `docs/design/lean-ir-coverage.md`
- `docs/design/lean-ir-semantics.md`

## Non-goals

- Checked types, resolution or behavioral meaning.
- Raw schema changes.
- Composition-source checking or semantics.
- Canonical-JSON syntax, byte-encoder or hash proofs.
- Linker algorithms/errors or bundle artifact coverage.

## Test and proof guidance

Use exhaustive definitions and `#guard` fixtures. The coverage document must
link each item to its concrete guard/fixture, raw inventory owner and
meaning/invariant owner.

For negative evidence only, the implementation may temporarily modify a covered
raw declaration file even though those files are not in the final allowed-file
set. Add one constructor to a covered inductive and one field to a covered
structure, and demonstrate that `Sembla.Semantics.Raw` fails specifically at the
corresponding coverage guard. Restore both files exactly before continuing.
Persistent raw-contract edits are forbidden, and the final diff must contain no
changes to any of the four covered declaration files.

## Acceptance criteria

1. Every covered field/constructor has PRD 0002 as raw inventory owner, exactly
   one numbered meaning/invariant owner or explicit future-composition owner,
   and no unexplained later dependency.
2. Every covered item has both a foundational-role classification and a
   frontend/checking-status classification, with no conflation between them.
3. Compile-time guards reject both a temporary unclassified inductive
   constructor and a temporary unclassified structure field; the diagnostic
   identifies the relevant guard, and both mutations are completely removed.
4. The four covered raw declaration files and all existing canonical export
   fixtures remain byte-for-byte unchanged.
5. Build, automated proof audit, proof hygiene and full checks pass.

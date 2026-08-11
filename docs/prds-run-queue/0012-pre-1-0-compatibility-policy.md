# PRD 0001: Record the pre-1.0 compatibility and conformance policy

max_review_cycles: 3

## Dependencies

The binding [contract-governance README](../prds-contract-governance/README.md) and the aggregate-first decision PRD are
accepted. `DECISIONS.md` §O therefore exists. The architecture atlas and
artifact registry are committed and pass their current checks.

## Context

Sembla has strong individual contracts, strict readers, canonical bytes, golden
fixtures and cross-language parity, but no repository-wide rule for evolving a
published contract. The artifact registry already classifies identifiers as
`frozen`, `versioned`, `legacy` or `internal`. This PRD gives those classes
normative meaning before `sembla.parameters/v1` is introduced.

This is policy, not a schema migration. It must not change production code or
serialized fixtures.

## Goal

Append `DECISIONS.md` §P, publish a maintained compatibility guide and bind the
roadmap/architecture documentation to an explicit pre-1.0 policy that removes
all discretionary compatibility choices from later PRDs.

## Requirements

### 1. Append `DECISIONS.md` §P

Add:

```markdown
## P. Contract compatibility and backend conformance (accepted 2026-08-11)
```

with P1–P8 in the existing decision / rejected alternatives / reason style:

1. **P1 — Registry stability classes are normative.** `frozen` bytes, identity
   and semantics do not change; `versioned` `/vN` meanings remain immutable;
   `legacy` readers remain until a later removal decision; `internal` and
   `test-identifier` carry no public promise. Reject case-by-case undocumented
   compatibility judgments.
2. **P2 — Incompatible evolution gets a new concern-local version.** Removing,
   renaming or changing the meaning/type/requiredness of a field requires a new
   schema identifier/version. An additive field is compatible only where the
   contract explicitly declares an extension posture and retained old-reader
   tests prove it is ignored safely. Strict unknown-field readers remain strict.
   Reject a repository-wide version integer and silent reinterpretation.
3. **P3 — Legacy forms are explicit dispatch branches.** Unversioned model JSON
   and plain JSON parameter objects remain readable throughout this pre-1.0
   policy. Writers prefer current versioned forms when a versioned equivalent
   exists. Removal requires a later decision, inventory and migration path.
4. **P4 — Migration is explicit and identity-changing.** Migrations are named,
   deterministic, separately invocable and tested against frozen before/after
   fixtures. They never occur silently inside parsing or simulation. A change to
   identity-bearing bytes produces new hashes/provenance. Canonicalization is
   not migration.
5. **P5 — Conformance is contract-specific evidence.** Every public/frozen or
   versioned contract names owners, readers, writers and executable evidence in
   the registry. Golden bytes, negative fixtures and independent validators are
   retained; documentation alone is not conformance.
6. **P6 — CPU is the semantic oracle; CUDA equality is behavioral.** No backend
   trait or shared implementation is introduced. Backend-semantic changes
   require a CPU-safe corpus plus qualifying real-device evidence. Compilation,
   stubs, fallback and unavailable results are not qualifying evidence.
7. **P7 — Shared Lean/Rust schema generation remains gated.** Do not introduce
   schema code generation now. Reconsider it only through a later decision when
   an accepted cross-language schema revision or retained drift defect shows
   material dual-maintenance cost. Reject speculative generation infrastructure.
8. **P8 — Additive parameter envelope and widget hold.** Authorize the new
   estimator-neutral `sembla.parameters/v1` artifact while retaining legacy
   plain objects. It ends at the CLI boundary and does not enter runtime, plans
   or backends. The Lean widget/headless split is explicitly deferred.

Do not edit §§A–O except a generated contents link if one exists.

### 2. Maintained compatibility guide

Create `docs/design/contract-compatibility.md` containing:

- the four stability classes and exact evolution rules from §P;
- producer, current-reader, old-reader and migration responsibilities;
- unknown-version and unknown-field behavior;
- legacy support/removal procedure;
- canonicalization, hashing and provenance consequences;
- conformance evidence requirements;
- CPU/CUDA evidence levels; and
- the trigger/non-goals for shared schema generation.

Use examples from existing model/plan/state/pairs/targets contracts, but do not
invent support commitments beyond current readers. Tables must link to the
artifact registry rather than duplicate its full entry list.

### 3. Update maintained indexes

- Link the guide from `docs/design/README.md` and `docs/architecture/contracts.md`.
- Update `docs/architecture/fitness-functions.md` with the future registry
  conformance requirement without claiming its implementation before PRD 0005.
- Register `prds-contract-governance/` in `docs/prds/TRACKS.md` and add one
  roadmap link under the pre-1.0 contract milestone.
- Update `docs/design/estimation-protocol.md` only to state that §P authorizes an
  additive envelope; PRDs 0002–0004 implement it. Do not claim it is already
  accepted by code.

## Allowed files

- `DECISIONS.md`
- `docs/design/contract-compatibility.md` (new)
- `docs/design/README.md`
- `docs/design/estimation-protocol.md`
- `docs/architecture/contracts.md`
- `docs/architecture/fitness-functions.md`
- `docs/prds/TRACKS.md`
- `docs/ROADMAP.md`

## Non-goals

- No production code, test, fixture, registry-shape or canvas change.
- No schema generator, JSON Schema, migration executable or compatibility shim.
- No removal of a legacy reader.
- No hardware execution, network access or paid resource.
- No Lean widget change.

## Test guidance

Run:

```bash
python3 -B scripts/check-markdown-links.py
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. §P contains P1–P8 with every decision, rejection and reason above.
2. Every artifact-registry stability class has one unambiguous evolution and
   removal policy.
3. The policy preserves strict readers, legacy model/parameter input and
   concern-local versioning; migration is never implicit.
4. The policy authorizes `sembla.parameters/v1` without claiming implementation
   or changing runtime/backend boundaries.
5. The policy makes qualifying real-device evidence mandatory for CUDA
   conformance and explicitly rejects unavailable/fallback evidence.
6. Shared schema generation and the Lean widget split remain deferred under
   explicit triggers/non-goals.
7. All links and full repository checks pass with no code or fixture changes.

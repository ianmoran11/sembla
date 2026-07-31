# PRD 0012: Artifact bundle, end-to-end provenance, documentation, acceptance sweep

## Context

Read `docs/prds-composition/README.md` first; its constraints bind. Every
pipeline stage exists: surface → source → linker → plan → validate → run,
with identity and hashes throughout. This PRD closes the release: the logical
artifact bundle (architecture doc §6, as amended by DECISIONS §J11 — hashes
live in manifests, not plans), bundle verification in Rust, linked provenance
flowing end-to-end into run manifests, user documentation, and a mechanical
sweep of the release acceptance criteria.

Bundle layout restated (frozen):

```text
<bundle-dir>/composition-source.json   canonical source bytes
<bundle-dir>/executable-plan.json      canonical plan bytes
<bundle-dir>/link-report.json          non-semantic report
<bundle-dir>/bundle-manifest.json      versions, hash records, relationships
```

## Specification

### 1. Bundle emission — extend `sembla-link`

```text
sembla-link <source.json> --bundle <dir>
```

writes all four files (creating `<dir>`; refuse to overwrite a non-empty
directory with a clear error). `bundle-manifest.json` is canonical JSON:

```json
{
  "bundle_schema": "sembla.bundle/v1",
  "canonical_encoding": "sembla.canonical-json/v1",
  "source": {
    "schema": "sembla.composition-source/v1",
    "path": "composition-source.json",
    "hash": { "algorithm": "sha256", "domain": "sembla.source-artifact/v1", "digest": "…" }
  },
  "linker": {
    "semantics": "sembla.linker/v1",
    "implementation": "lean4"
  },
  "plan": {
    "schema": "sembla.executable-plan/v1",
    "identity_scheme": "sembla.identity/stable-v1",
    "origin": "linked",
    "enabled_features": [],
    "path": "executable-plan.json",
    "semantic_hash": { "algorithm": "sha256", "domain": "sembla.plan-core/v1", "digest": "…" },
    "envelope_hash": { "algorithm": "sha256", "domain": "sembla.plan-envelope/v1", "digest": "…" }
  },
  "source_map_schema": "sembla.source-map/v1",
  "bundle_integrity": { "algorithm": "sha256", "domain": "sembla.bundle-root/v1", "digest": "…" }
}
```

All hashes over exact file bytes (source/plan files are canonical by
construction) except `bundle_integrity`, whose payload is (README §hash
rules): canonical manifest bytes **with the `bundle_integrity` key omitted**,
followed by, for each of the three named files in lexicographic path order,
the UTF-8 of `path`, a `0x00`, and the raw digest bytes. The manifest itself
is not a named-file input. Implement the payload construction once in Lean
(emission) and once in Rust (verification) against this exact spec —
cross-checked by the fixture.

`--plan`/`--report` single-file modes remain unchanged.

### 2. Bundle verification — `sembla-cli`

New command `sembla bundle-verify <dir>`:

1. read the manifest; reject unknown `bundle_schema`/encoding/domains or
   any absent required field (all-present-or-absent tuple rules);
2. recompute all three file hashes and the bundle integrity; any mismatch →
   nonzero exit naming the failing record;
3. run full plan validation + canonicality on `executable-plan.json`;
4. check manifest/plan agreement: origin, schemas, identity scheme,
   `enabled_features`, and that the plan's embedded
   `linked_provenance.source_hash` equals the manifest's source hash record;
5. print one `ok` line per check, deterministic order.

The plan must remain runnable when copied out of the bundle (it embeds its
provenance) — covered by a test that copies `executable-plan.json` to a temp
path and runs it.

### 3. End-to-end provenance in run manifests

`sembla run` of a linked plan already writes the `plan` +`linked_source`
tuples (PRD 0004 §4). Add the end-to-end test now that linked plans exist:
run `fixtures/plans/linked/two_regions.plan.json`, assert the manifest
contains the complete `plan` tuple, and a `linked_source` tuple whose
`source_hash` digest equals the checked-in source fixture's hash; assert a
`direct_stable` plan run manifest has no `linked_source`; assert
`verify-run` accepts a plan-run manifest end-to-end (extend `verify-run` to
plan files: same parse dispatch, recompute the semantic hash, compare —
keeping its legacy behavior untouched).

### 4. Golden bundle fixture

Check in `fixtures/bundles/epidemic_policy/` (the four files, produced by
`sembla-link --bundle` from the checked-in `epidemic_policy` source).
Parity: append a section linking to a temp dir and `cmp`-ing all four files.
Rust: a CLI test runs `bundle-verify` on the checked-in bundle; negative
tests corrupt (in a temp copy) each of: one byte of the source, one byte of
the plan, the report (must **fail** integrity too — the report is named in
the manifest even though non-semantic), a manifest digest, and a manifest
with `bundle_integrity` omitted — each failing with the named record.

### 5. Documentation

- New `docs/guides/composition.md`: a practical guide — authoring components
  (PRD 0011 syntax), exporting sources, linking (`sembla-link`, single-file
  and bundle modes), validating/running plans, reading identity tuples in
  manifests, the identity grammar and what refactors preserve identity
  (display renames, permutations) versus change it (stable-id renames,
  moving across composite boundaries), and the legacy/`direct_stable`/
  `linked` origin distinctions. Link to DECISIONS §J for normative rules.
- `DESIGN.md`: one short amendment note in §4.4 (composition) pointing at
  `docs/guides/composition.md` and DECISIONS §J — follow the existing amendment
  style in the file header (dated, one sentence per addition). Do not
  rewrite any existing prose.
- `docs/design/option-d-architecture.md`: extend the Status line: first
  release (doc Phases 0–4) implemented via `docs/prds-composition/`;
  remaining phases unimplemented.
- `README.md` (repo root): add the composition pipeline to the feature
  summary and a three-command example (author → link → run) if the README
  has such sections; keep the diff minimal.

### 6. Release acceptance sweep

In the implementation notes, walk architecture doc §26.2 and record, for
each of its 20 criteria: **met** (with the test/fixture/PRD that proves it),
**deferred** (with the §J12/§J13 decision deferring it — e.g. criterion 15's
non-Lean fixture, criterion 18's CUDA composition corpus), or **n/a**. Any
criterion that is neither met nor explicitly deferred by a recorded decision
fails this PRD. Then run the complete check battery:

```bash
./scripts/check.sh
cd frontend && lake build && bash scripts/test-negative.sh
bash frontend/scripts/check-parity.sh
git diff --check
```

## Allowed files

- `frontend/LinkMain.lean`, `frontend/Sembla/Composition/**` (bundle
  emission only), `frontend/Sembla.lean`
- `crates/sembla-cli/src/main.rs`, `manifest.rs`,
  `crates/sembla-cli/tests/**`
- `frontend/scripts/check-parity.sh` (append only)
- `fixtures/bundles/**` (new)
- `docs/guides/composition.md` (new), `DESIGN.md` (§4.4 amendment note + header
  amendment line only), `docs/design/option-d-architecture.md` (Status
  only), `README.md` (minimal additions)
- implementation notes/artifacts created by the managed run

## Non-goals

- Archive formats, provenance databases, event streams, replay bundles
  (DESIGN.md §5.4's explicit exclusions).
- Relinking tooling, historical-linker retention automation (doc §20.3) —
  note as future work in `docs/guides/composition.md`.
- Any new linker/runtime/schema behavior; this PRD only packages, verifies,
  records, and documents.
- Sweep/NPE/CUDA integration with plans.

## Acceptance criteria

1. `sembla-link --bundle` reproduces the checked-in
   `fixtures/bundles/epidemic_policy/` byte-for-byte (parity section), and
   `sembla bundle-verify` passes on it.
2. Every corruption negative fails naming the specific failing record; the
   copied-out plan runs standalone.
3. The end-to-end provenance test passes: linked-run manifests carry both
   complete tuples with the correct source digest; `direct_stable` runs
   carry no `linked_source`; `verify-run` works for plan runs and is
   unchanged for legacy runs.
4. `docs/guides/composition.md` exists and covers authoring→link→run, identity
   preservation rules, and origins; DESIGN.md and the architecture doc carry
   their amendment lines; no other prose rewritten.
5. The implementation notes contain the complete 20-point §26.2 sweep with
   every point met, explicitly deferred, or n/a — none silent.
6. The full check battery passes; `git diff --check` clean; no legacy
   artifact changed anywhere in the folder's history (final spot-check:
   `git diff <run-start> -- examples/` is empty).

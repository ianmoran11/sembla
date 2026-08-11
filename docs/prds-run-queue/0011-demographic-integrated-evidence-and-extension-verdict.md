# PRD 0010: Integrated foundation evidence and semantic-extension verdict

## Dependencies

PRDs 0001–0009 accepted. Read the binding [demographic-spine README](../prds-demographic-spine/README.md) first. This PRD integrates and
documents existing results; it may fix a directly discovered documentation or
check-script defect within its allowlist, but it does not add another model
feature to obtain a preferred verdict.

## Context

The foundation track was designed to answer two separate questions:

1. can aggregate demographic models identify parameters and reproduce expected
   micro summaries without broad NPE; and
2. can Lean static fusion support person identity, lifecycle, fixed geography
   and stock-state domain extensions without changing executable IR/Rust?

The answer may include scientific failure or a measured frontend limitation.
The final record must distinguish implemented capability, bounded workaround and
still-deferred core semantics. Synthetic labour/justice conformance is not
scientific driver validation.

## Goal

Land one hash-complete foundation evidence index, make the lightweight fixture
checks permanent, update maintained documentation to the measured state, and
record an exact go/defer verdict for all six framework needs without adding IR
or runtime semantics.

## Specification

### 1. Foundation evidence index

Create `docs/evidence/demographic-spine-foundation/README.md` and
`foundation-verdict.json`. The JSON is canonical and names hashes/relative paths
for:

- PRD 0002 profile fit and identification result;
- PRD 0003 expectation implementation/fixture contract;
- PRD 0004 parity, selection and chain-comparison evidence;
- pure facet theorem/fixture results;
- demographic, labour and justice model/plan/state build reports;
- lifecycle row/generation evidence;
- regional and labour→justice CRN comparisons; and
- exact reproduction/check commands.

Do not copy large evidence into a competing summary. Link the authoritative
artifact and quote only mechanically derived headline values.

### 2. Six-need verdict

For each need, record `implemented`, `bounded` or `deferred`, its exact evidence
and the next trigger. PRD 0010 is reachable only after PRDs 0001–0009 are
accepted; a frozen-boundary blocker in an earlier pilot stops the managed run at
that PRD and therefore produces no artificial final `failed` verdict:

1. common person identity;
2. lifecycle isolation through present-row guards and atomic entry reset;
3. slot reuse/generation;
4. sparse justice/case records;
5. state-level feedback; and
6. different timescales.

The expected architectural classification is evidence-tested, not assumed:

- static one-box identity, atomic entry reset/present-row isolation,
  reusable-generation fixture and fixed-enum dispatch may be `implemented` only
  if their structural and runtime checks pass;
- sparse records remain at most `bounded` by person-aligned/fixed-pool state;
- different timescales remain at most `bounded` by one global tick/phase state;
- first-class shared tables, person-row wires, dynamic allocation and
  heterogeneous schedulers remain `deferred`.

Every `deferred` item names a concrete future evidence trigger. Do not create a
follow-on PRD or choose its design here.

### 3. Permanent lightweight check

Finalize `scripts/check-demographic-spine.sh` so a normal local run:

- regenerates all small synthetic model/plan/state fixtures into a temp tree and
  compares exact bytes;
- runs focused Lean facet tests and negative diagnostics;
- validates each plan/state and executes the small deterministic/CRN lifecycle
  cases;
- checks evidence manifests for path/hash consistency; and
- performs no network, paid-hardware, full 15-year replicate or NPE work.

Add it to `scripts/check.sh` only after measuring and recording its local wall
time. Keep the permanent check bounded to the small foundation fixtures; PRD
0004's expensive evidence is verified by hashes and focused reducer tests, not
rerun on every repository check.

Add script-level tests for missing/stale artifacts, changed hashes, wrong model
name, skipped required case and execution from outside the repository root.

### 4. Maintained documentation

Update:

- `docs/design/demographic-spine-and-modular-calibration.md` status from accepted
  foundation design to implemented foundation, while retaining §O authority and
  every non-claim;
- `docs/models/demographic-spine-foundation.md` with final model/fixture hashes,
  exact supported capability and local timing;
- `docs/guides/person-facets.md` with the final API/syntax/check workflow;
- the Australian aggregate-calibration guide with PRD 0004's selected status;
- `docs/overview.md` and `docs/ROADMAP.md` with a concise implemented-foundation
  statement that does not call synthetic domain models credible baselines;
- `docs/README.md`, `docs/models/` index if one exists, and
  `docs/evidence/README.md` with canonical links; and
- `docs/prds-demographic-spine/README.md` with a completion/status note and
  residual risks, without rewriting its frozen original constraints.

Do not alter the justice semantic documents except existing links if they are
broken. Do not move the proposal to the archive: it remains current technical
interpretation for the foundation.

### 5. Compatibility and forbidden-diff audit

Retain a machine-readable audit in the evidence directory proving the track
made no changes to:

- the frozen IR, JSON/plan/composition source/linker modules named by the track
  contract;
- any Rust crate or Cargo dependency file;
- existing canonical examples/plans/source/bundles/state fixtures;
- the frozen Australian model module and its subdirectory named by the track
  contract;
- retained Australian evidence and gravity/NPE artifacts; and
- state/schema/version/hash/identity strings.

Use repository history from the PRD 0001 parent commit through the current
working tree, not a hand-written list of “expected unchanged” hashes alone.
Unrelated pre-existing changes are not attributed to this track.

### 6. Final validation report

Record commands, versions, hardware and results for:

```bash
bash scripts/check.sh
python3 -B -m unittest discover -s data/abs/tests -p 'test_*.py'
bash scripts/check-abs-data.sh
(cd frontend && lake build)
bash frontend/scripts/test-negative.sh
bash frontend/scripts/check-parity.sh
bash frontend/scripts/check-proofs.sh
python3 scripts/check-markdown-links.py
python3 scripts/check-prd-allowlist.py --all
git diff --check
```

A hardware-unavailable optional CUDA check is labelled unavailable. All required
CPU/document/data/frontend checks must pass.

## Allowed files

- `docs/evidence/demographic-spine-foundation/README.md` (new)
- `docs/evidence/demographic-spine-foundation/foundation-verdict.json` (new)
- `docs/evidence/demographic-spine-foundation/compatibility-audit.json` (new)
- `docs/evidence/demographic-spine-foundation/final-validation.json` (new)
- `docs/evidence/README.md`
- `docs/design/demographic-spine-and-modular-calibration.md` (status/results
  links only)
- `docs/models/demographic-spine-foundation.md`
- `docs/guides/person-facets.md`
- `docs/guides/australian-population-aggregate-calibration.md`
- `docs/overview.md`
- `docs/ROADMAP.md`
- `docs/README.md`
- `docs/design/README.md` (status wording only if needed)
- `docs/prds/TRACKS.md` (status wording only if used)
- `docs/prds-demographic-spine/README.md` (append-only completion note)
- `scripts/check-demographic-spine.sh`
- `scripts/check.sh` (one bounded check invocation only)
- `scripts/tests/test_check_demographic_spine.py` (new)
- implementation notes/artifacts created by the managed run

## Non-goals

- No source model/compiler feature, parameter refit, evidence rerun or fixture
  regeneration beyond exact deterministic checking of already accepted outputs.
- No real labour/justice data or scientific model-card claim.
- No new follow-on PRD, IR/runtime design or resolution of a deferred item.
- No changing a measured status to make the headline “four of six.”
- No paid-hardware requirement or benchmark generalization.

## Acceptance criteria

1. Every required validation command passes and is retained with versions,
   hardware and status; optional CUDA unavailability is honest.
2. The evidence index and canonical verdict resolve/hash every authoritative
   artifact and derive rather than transcribe headline values.
3. All six framework needs have evidence-backed statuses and triggers; sparse
   rows and heterogeneous timing are not called implemented by fixed-pool/global
   tick workarounds.
4. The permanent foundation check is bounded, tested, root-independent and
   invoked by `scripts/check.sh` without rerunning expensive calibration
   evidence.
5. Maintained docs distinguish the implemented foundation, synthetic domain
   conformance, aggregate calibration result and all residual non-claims.
6. The compatibility audit proves no executable IR/plan/composition/Rust,
   dependency, Australian baseline, retained evidence or existing canonical
   artifact change across the track.
7. The track README receives an append-only completion/residual-risk note and
   indexes/links are consistent.
8. No implementation is added merely to alter a bounded/deferred verdict; an
   earlier frozen-boundary blocker would have stopped the run before this PRD.

# Composition integration PRDs

Ordered PRD set that connects the implemented Option D composition pipeline
(`docs/prds-composition/`, DECISIONS.md §J) to the three workflows that are
the project's point: GPU execution, NPE calibration sweeps, and
common-random-number policy contrasts — plus the missing composition
structure widget. Run from the Sembla repository with:

```text
/piprd run docs/prds-composition-integration
```

`README.md` is ignored by `/piprd run`. Every numbered PRD must read this
file first; the constraints below are binding. When a PRD conflicts with this
README, this README wins.

## Preconditions

The managed run requires a clean relevant working tree. The pending
composition-demos work (modified `README.md`, `docs/guides/composition.md`,
`frontend/Main.lean`, `frontend/Sembla/Demos.lean`,
`frontend/scripts/check-parity.sh`, plus `fixtures/demos/composition/` and
`frontend/Sembla/Demos/Composition*.lean`) must be committed (or reverted)
**before** starting. Do not let a PRD absorb that diff.

## Authority and scope

- `DESIGN.md`, `DECISIONS.md` (especially §J), `docs/ROADMAP.md` (as re-cut
  by PRD 0001), and `docs/prds-composition/README.md`'s frozen contracts all
  bind. The plan/source schemas, identity grammar, version strings, canonical
  encoding, and hash constructions are **frozen** — no PRD in this folder may
  change a schema, version string, hash payload, or identity rule.
- **No new semantics.** This folder adds no model constructs, no linker
  behavior, no runtime semantics. It routes already-validated plans into
  existing execution paths and renders existing source structure. Anything
  from DECISIONS §J12's deferred list (families, adapters, merges,
  invariants, heterogeneous schedulers, dynamic topology, non-Lean
  producers) remains out of scope.
- Existing artifacts stay byte-frozen: `examples/**`, all CSV/hash goldens,
  linked plan fixtures under `fixtures/plans/**`, bundle fixtures, the
  negative suite, and the CUDA differential evidence. A diff to any of them
  is a failed PRD.

## Decisions frozen for this folder (recorded in DECISIONS §J14 by PRD 0001)

1. **CUDA keying.** The CUDA backend follows the same split PRD 0004 of the
   composition track gave the CPU runtime: the content-addressed `rule_word`
   is used wherever a rule identity enters a Philox counter or an ordering/
   tie-break key; the dense `rule_id` ordinal remains for indexing, buffer
   layout, codegen specialization, and diagnostics. Legacy models have
   `rule_word == rule_id`, so legacy CUDA behavior is bit-identical — the
   existing differential goldens prove it.
2. **GPU evidence discipline.** Hosted CI has no GPU; the differential
   corpus runs via the PRD-0009 remote runbook
   (`crates/sembla-cuda/scripts/run-differential-corpus.sh`) and its stub
   workflow is never presented as evidence (`docs/contributing/ci.md`). PRDs in this
   folder therefore split acceptance into **local criteria** (must pass in
   the managed run without a GPU: compilation, corpus listing, graceful
   skips, legacy goldens unchanged) and **hardware criteria** (recorded in
   the runbook/evidence conventions, executed manually later). A PRD is
   approvable on local criteria alone; hardware criteria must be *listed*
   in its implementation notes as pending.
3. **Mixed identity schemes never compare.** `compare` rejects a legacy
   model in one arm and a plan envelope in the other with a deterministic
   error. CRN pairing across the legacy/stable identity boundary is
   meaningless and must not be silently computed.
4. **Plan sweeps carry the plan tuple, not `ir_hash`.** A sweep over a plan
   envelope records the existing `PlanIdentityTuple` (and `LinkedSourceTuple`
   when origin is `linked`) in its manifests via the existing
   `plan_identity_tuples` helper; the legacy `ir_hash` field stays absent.
   Legacy sweeps are byte-unchanged.
5. **`--dt` overrides never apply to plans** (already enforced for `run`;
   the same rejection extends to `diff-backends`). A plan is edited and
   re-linked/re-canonicalized, never mutated at the CLI.
6. **`--all-examples` grows a sibling, not a new meaning.**
   `diff-backends --all-examples` keeps its exact current behavior; a new
   `--all-plan-fixtures` flag walks `fixtures/plans/*.plan.json` and
   `fixtures/plans/linked/*.plan.json`. The corpus runbook runs both.
7. **Widgets render structure only.** The composition widget is a structure
   widget in the DESIGN.md §3 taxonomy (zero runtime cost, rendered from
   elaborated values). No behavior widgets, no simulation calls, no new
   frontend dependencies beyond the existing ProofWidgets stack.

## Required checks for every PRD

From the repository root, each implementation and review must run and pass:

```bash
./scripts/check.sh
cd frontend && lake build
bash frontend/scripts/check-parity.sh
git diff --check
```

PRD 0005 (widgets/surface) must also run
`bash frontend/scripts/test-negative.sh`. CUDA-touching PRDs must
additionally confirm `cargo build -p sembla-cuda` and the crate's
non-GPU tests pass locally, and that GPU-gated tests skip cleanly (not
fail) on a machine without CUDA.

## Run order

1. `0001-roadmap-and-decisions.md` — re-cut `docs/ROADMAP.md` post-
   composition; record §J14 integration decisions.
2. `0002-cuda-plans-and-diff-backends.md` — plan envelopes on the CUDA
   backend; `diff-backends` over plans; differential corpus extension.
3. `0003-sweep-plans.md` — `sweep` over plan envelopes; NPE `(θ, x)` export
   for composed models; plan tuples in sweep manifests.
4. `0004-compare-crn-plans.md` — `compare` over plan envelopes; the CRN
   noninterference contrast test; counterfactual demo wiring.
5. `0005-composition-widget.md` — composition wiring/nesting structure
   widget; leaf-body panels re-attached in component bodies.

Later PRDs depend on earlier ones only where stated (0003 and 0004 are
independent of 0002 except for shared helper code; 0005 is independent of
all but 0001). Still: do not reorder, and do not implement a later PRD's
flags or commands early.

## Global non-goals

- No schema, version-string, hash, identity, linker, or runtime-semantics
  changes.
- No golden regeneration; no edits under `examples/**` or existing
  `fixtures/**` files (adding new fixture files is allowed where a PRD says
  so).
- No new Rust or Lean dependencies.
- No CUDA kernel algorithm changes beyond the rule-word keying of decision 1.
- No behavior widgets, no `Share`/families/adapters, no non-Lean producers.
- No editing `.piprd/`, CI workflow semantics (docs-only mentions are fine
  where a PRD says so), or external vault copies.

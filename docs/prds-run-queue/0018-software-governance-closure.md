# PRD 0007: Close the software governance track

max_review_cycles: 3

## Dependencies

Contract-governance PRDs 0001–0006 are accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md). The
qualifying-CUDA evidence track has not run and must remain explicitly pending.

## Context

This PRD adds no capability. It audits the accepted policy, parameter contract,
producer adoption, registry-driven documentation and CPU-safe conformance
runner as one architecture change, and leaves a deterministic handoff for the
separately approved paid-device run.

## Goal

Publish a machine-checkable software closure report, update maintained
architecture/roadmap status truthfully, and prove that every frozen boundary and
retained evidence family remained unchanged.

## Requirements

### 1. Compatibility audit

Create `docs/evidence/contract-governance/software-closure.json` with schema
`sembla.contract-governance-closure/v1`. Generate it from repository state and
include:

- `track_base` and `software_current` discovered exactly by the binding README's
  first-parent commit-subject rule, plus the contiguous PRD sequence;
- compatibility-policy section/version;
- two named registry/index snapshots: hashes/counts read from
  `software_current`, and hashes/counts from the PRD 0018 worktree after the
  closure schema/projection is registered and the index regenerated;
- parameter schema, positive/negative fixture and cross-language test hashes;
- legacy/envelope conformance results;
- maintained aggregate producer sidecar inventory/hashes;
- backend corpus/report schema and CPU/offline conformance results produced in a
  temporary clean detached worktree at `software_current`;
- hashes and explicit unchanged verdicts for all pre-track model, plan,
  composition, identity, canonical JSON, state, bundle, hash-vector and retained
  scientific-evidence fixture families; and
- `qualifying_cuda_status: "pending_separate_paid_plan_approval"`.

A companion `SHA256SUMS` covers the closure report and generated contract index.
`scripts/check-contract-governance-closure.py` supports explicit `--write` and
read-only `--check`; both derive the exact baseline path sets/globs from the
binding README, reject empty/missing/unaccounted families and compare baseline
bytes from Git objects rather than the dirty worktree. Write mode creates a
temporary clean detached worktree at `software_current` for CPU corpus and
command evidence, then removes it. The report describes
accepted implementation through `software_current` (the 0017 commit); PRD 0018
is evidence/docs only and does not create a self-referential commit hash. No
manually edited pass field is accepted.

Register `sembla.contract-governance-closure/v1`, map it to the existing
registry/conformance canvas concept and regenerate the contract index before the
registry freshness check. The report's implementation/frozen-family evidence is
bound to `software_current`; its `closure_architecture` snapshot is explicitly
bound to the post-registration PRD 0018 worktree registry/index bytes. The
report never hashes itself. `SHA256SUMS` may cover both final report and index
without entering either one's own hash payload.

### 2. Maintained documentation

Update architecture and documentation indexes so they state:

- §P is the compatibility authority;
- `sembla.parameters/v1` is implemented and used by the maintained
  aggregate-first output path while legacy objects remain supported;
- registry facts generate the complete contract index and coverage-check the
  conceptual canvas;
- the versioned backend corpus and CPU/offline runner are implemented;
- no qualifying CUDA evidence exists yet for this commit/corpus; and
- the separate GPU PRD folder is the only path that may close that gate.

Update `Contracts.canvas`, `Runtime.canvas` and `Estimation.canvas` only for
small status/link text changes. Preserve current geometry, zero overlaps and no
file-preview nodes.

The roadmap may mark software governance complete but must leave real-device
conformance pending. It must not mark schema code generation or Lean widget
separation planned/implemented.

### 3. Track/run documentation

Update the contract-governance README with the accepted software status and
exact GPU handoff. Update `docs/prds/TRACKS.md` and `docs/evidence/README.md`.
Do not move queued PRD files during the managed run; moving them back to their
binding folders is post-run housekeeping because `/piprd` captures queue paths
at startup.

### 4. Final validation

Run and retain concise command/exit evidence for:

- full architecture registry/index/canvas tests;
- parameter contract and CLI conformance;
- ABS parameter-sidecar regeneration;
- CPU backend corpus twice;
- Rust workspace checks;
- determinism;
- Lean proof/parity checks; and
- the complete repository gate.

No check may access network, cloud, credentials or GPU. CUDA-feature compilation
is required where already supported locally; hardware execution is not.

## Allowed files

- `docs/evidence/contract-governance/software-closure.json` (new)
- `docs/evidence/contract-governance/SHA256SUMS` (new)
- `scripts/check-contract-governance-closure.py` (new)
- `scripts/tests/test_check_contract_governance_closure.py` (new)
- `scripts/check.sh`
- `docs/architecture/artifact-registry.json`
- `docs/architecture/generated-contract-index.md`
- `docs/architecture/README.md`
- `docs/architecture/contracts.md`
- `docs/architecture/runtime.md`
- `docs/architecture/estimation.md`
- `docs/architecture/fitness-functions.md`
- `docs/architecture/Contracts.canvas`
- `docs/architecture/Runtime.canvas`
- `docs/architecture/Estimation.canvas`
- `docs/design/contract-compatibility.md`
- `docs/design/estimation-protocol.md`
- `docs/guides/parameter-artifacts.md`
- `docs/README.md`
- `docs/ROADMAP.md`
- `docs/prds/TRACKS.md`
- `docs/evidence/README.md`
- `docs/prds-contract-governance/README.md`

## Non-goals

- No new production behavior, contract field, fixture, estimator, semantic or
  backend change.
- No GPU execution, paid plan, Terraform, network or credential access.
- No false completion of the real-device gate.
- No Lean widget split, shared schema generation or PRD-file relocation.
- No editing retained scientific evidence.

## Test guidance

Run every command required by the binding README plus:

```bash
python3 -B scripts/check-contract-governance-closure.py
python3 -B scripts/render-contract-index.py --check
python3 -B scripts/run-backend-conformance.py --cpu
python3 -B scripts/run-backend-conformance.py --cpu
bash scripts/check-abs-data.sh
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. The generated closure report is hash-complete, reproducible and derived from
   actual command/fixture evidence rather than hand-entered verdicts.
2. Commit discovery yields the unique contiguous 0012–0017 managed sequence;
   the report binds its parent as `track_base` and 0017 as `software_current`
   without self-reference.
3. Every non-empty frozen family/path from the binding baseline inventory is
   proven unchanged, and every addition is accounted for by an explicitly new
   root.
4. Parameter core/CLI/producer, registry/index/canvas and CPU corpus gates all
   pass together.
5. Maintained docs accurately mark software work complete and qualifying CUDA
   evidence pending the separately approved exact paid plan.
6. Canvases retain valid links, no previews, no overlaps and readable status
   cards.
7. The closure identifier/projection and regenerated index pass freshness checks;
   no production schema/semantic change is introduced by this closure PRD.
8. All focused, Rust, ABS, determinism, Lean and full repository checks pass,
   with no staged files.

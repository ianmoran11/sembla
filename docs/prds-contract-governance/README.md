# Contract governance and backend conformance PRDs

**Status:** approved for implementation on 2026-08-11.

This track implements the cross-cutting actions accepted after the architecture
review:

1. define a pre-1.0 compatibility and migration policy;
2. add the estimator-neutral `sembla.parameters/v1` artifact;
3. make the artifact registry drive factual contract documentation and cover the
   conceptual `Contracts.canvas`;
4. establish a fixed CPU/CUDA behavioral-conformance corpus and evidence
   format; and
5. prepare, but do not provision, the separate qualifying-CUDA evidence run.

The Lean widget/headless split is explicitly deferred and is outside this
track. No PRD may edit widget modules, widget imports or widget tests.

The numbered software PRDs are currently moved into
[`docs/prds-run-queue/`](../prds-run-queue/) so one managed run can execute the
existing prerequisite, the demographic-spine foundation and this track in
order. They remain governed by this README while queued. `/piprd` requires the
path to the first **file**, not the folder.

The paid-device evidence PRDs remain in
[`docs/prds-contract-governance-gpu/`](../prds-contract-governance-gpu/) and are
a separate managed run after exact paid-plan approval.

## Preconditions

Before starting the software queue:

- the current architecture work, this README and every queued PRD must be
  reviewed and committed;
- the Sembla working directory must be clean;
- `.piprd-config.json` must exist and every configured model/provider must be
  authenticated;
- Git author identity and repository hooks must work without prompts; and
- the existing full repository gate must pass.

The queued observation-builder PRD runs first. The demographic-spine decision
then records aggregate-first calibration as normative before this track changes
contract policy. If either prerequisite is not approved, stop; do not weaken a
later PRD to bypass it.

Before starting the separate GPU run, every additional precondition in its
binding README must hold, including explicit operator approval of the exact
saved paid plan. General approval of this track is not approval to spend money.

## Authority

- [`DESIGN.md`](../../DESIGN.md) defines model, runtime and reproducibility
  architecture.
- [`DECISIONS.md`](../../DECISIONS.md) is normative. The demographic track adds
  §O; this track adds the next available section for compatibility governance.
- [`docs/design/estimation-protocol.md`](../design/estimation-protocol.md)
  defines the quarantined estimator/simulator boundary.
- [`docs/architecture/artifact-registry.json`](../architecture/artifact-registry.json)
  is the machine-readable inventory of artifact identifiers and owners.
- CPU execution remains the semantic oracle. CUDA remains an independent
  implementation governed by behavioral conformance, not a shared backend
  trait.

When a numbered PRD conflicts with this README, this README wins.

## Compatibility policy this track must record

The first PRD records these choices without reopening them:

- `frozen`: accepted bytes, identity and semantics do not change;
- `versioned`: a published `/vN` meaning is immutable; an incompatible shape or
  meaning requires `/vN+1` and an explicit migration;
- `legacy`: read compatibility remains until a later normative removal
  decision; legacy input is never silently reinterpreted;
- `internal` and `test-identifier`: no public compatibility promise, but tests
  remain truthful about their purpose;
- unknown fields remain rejected wherever the schema-of-record currently uses
  strict decoding;
- migration is explicit, deterministic, separately testable and never hidden
  inside simulation semantics;
- canonicalization and migration are distinct operations;
- a migration that changes identity-bearing bytes produces new identity and
  provenance; and
- shared Lean/Rust schema generation is not introduced by this track. Revisit it
  only through a separate decision when an accepted cross-language schema
  revision demonstrates material dual-maintenance cost.

Legacy plain JSON parameter objects remain supported. The new parameter
envelope is additive and does not alter runtime `ParamEnv`, executable plans,
state artifacts or backend APIs.

## Deterministic governance baseline

The closure PRD discovers its baseline from managed commit history; no agent
chooses a SHA. On the first-parent chain it must find exactly one commit with
subject `Implement 0012-pre-1-0-compatibility-policy`. `track_base` is that
commit's parent. It must then find the unique contiguous managed commits through
`Implement 0017-backend-conformance-corpus`; that 0017 commit is
`software_current` and is HEAD when PRD 0018 begins. Missing, duplicate,
non-contiguous or unexpected intervening commits stop closure.

The PRD 0018 report describes accepted implementation through
`software_current`. Its own later commit contains evidence/docs only and is not a
self-referential input hash.

For every glob below, enumerate files from the `track_base` Git tree, require at
least one match, and compare those exact baseline paths byte-for-byte at
`software_current`; additions outside the baseline path set do not hide a
modification/deletion:

```text
examples/**/*.json
fixtures/australian-population/**
fixtures/bundles/**
fixtures/composition-source/**
fixtures/demographic/**
fixtures/demos/**
fixtures/hash/**
fixtures/performance/**
fixtures/plans/**
fixtures/state/**
fixtures/validation-negative/**
calibration/npe/**
data/abs/contracts/**
data/abs/extracts/**
data/abs/params/**
data/abs/reference/**
data/abs/targets/**
docs/evidence/**
```

New allowed roots (`fixtures/parameters/**`,
`fixtures/backend-conformance/**`, new `*.parameters.json` sidecars and
`docs/evidence/contract-governance/**`) are audited separately against their new
contracts. The checker rejects empty families, missing baseline paths and any
file not accounted for by either the frozen-baseline inventory or an explicitly
new root.

## Frozen boundaries

No software PRD in this track may change:

- existing model, executable-plan, composition-source, source-map, bundle,
  state, canonical-JSON, identity, linker or hash-domain bytes/semantics;
- retained ABS, gravity, NPE, demographic benchmark or other scientific
  evidence;
- existing canonical plan/model/state/bundle fixtures except to add independent
  assertions proving they remained unchanged;
- the estimator quarantine or the rule that Rust/Lean do not know estimator
  implementations;
- CPU/CUDA arithmetic, RNG coordinates, transition semantics or backend
  selection policy; or
- dependencies, public Lean syntax, widgets or formal proof scope.

The new conformance runner observes existing behavior. It must not make the two
backends share an implementation or normalize scientifically meaningful
outputs.

## Unattended-run rules

All decisions are frozen in these PRDs. Implementation/review agents must not
ask the operator to choose a schema, tolerance, migration rule, GPU model or
fallback. If repository evidence makes a requirement impossible, stop with the
exact failing criterion and command rather than inventing a new policy.

Software PRDs must use no network, cloud API, Terraform, Keychain credential or
paid resource. Every acceptance command must be non-interactive. PRDs refer to
“this PRD at its current path” because queue files move; no command may hard-code
its own filename.

Every numbered PRD must run the checks relevant to its files plus:

```bash
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

Rust PRDs also run `bash scripts/check-rust.sh` and
`bash scripts/check-determinism.sh`. Architecture PRDs run all architecture checker
unit tests and both architecture checkers. No PRD regenerates a frozen fixture
to make a check pass.

## Software run order

| Track PRD | Queued purpose |
| --- | --- |
| 0001 | Record pre-1.0 compatibility and migration policy |
| 0002 | Define the strict additive `sembla.parameters/v1` core contract |
| 0003 | Admit the envelope at CLI parameter boundaries without runtime leakage |
| 0004 | Emit provenance-bearing parameter artifacts from maintained data workflows |
| 0005 | Derive factual contract documentation and canvas coverage from the registry |
| 0006 | Freeze the CPU-safe backend conformance corpus and canonical report |
| 0007 | Close the software track and publish the hardware-pending verdict |

All later PRDs depend on all earlier PRDs. Do not reorder or combine them.

## Global non-goals

- No `Estimator` or `SimulationBackend` trait.
- No shared-schema generator, JSON Schema toolchain or event/replay framework.
- No Lean widget/headless split.
- No removal of legacy model or parameter input.
- No migration of historical evidence or in-place rewriting of user files.
- No CUDA optimization, kernel rewrite or performance threshold.
- No claim of qualifying GPU evidence from compilation, stubs, unavailable
  reports, CPU fallback or old evidence bound to a different commit/corpus.
- No paid-resource provisioning in the software queue.

# PRD 0004: Emit provenance-bearing parameter artifacts

max_review_cycles: 3

## Dependencies

Contract-governance PRDs 0001–0003 and the demographic-spine aggregate
calibration PRDs are accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md). The selected aggregate
path and its profile/parity/selection evidence exist and are immutable inputs to
this PRD.

## Context

The core and CLI can now validate parameter artifacts, but an architectural
contract is incomplete until a maintained estimator produces it. Existing
annual, gravity, profiled, aggregate and retained NPE JSON value files must not
be rewritten: they are established artifacts and evidence. New deterministic
sidecars can bind those exact values to their model and evidence.

## Goal

Add a standard-library Python writer/reader, emit `sembla.parameters/v1`
sidecars for the maintained aggregate-first outputs, consume those sidecars in
the maintained chain, and preserve every pre-existing parameter/evidence byte.

## Requirements

### 1. Independent Python contract implementation

Add `data/abs/parameter_artifact.py` implementing the exact PRD 0002 shape and
canonical bytes without importing Rust, Lean, `calibration/npe`, third-party
packages or model-parser internals. It must:

- validate strict keys, tagged provenance, slugs, paths, lowercase hashes,
  canonical reference order and finite numeric values;
- read the exported Australian model JSON only to verify name, complete
  parameter inventory/types and canonical IR hash;
- write compact recursively key-sorted JSON with no trailing newline;
- hash referenced input/diagnostic bytes before writing;
- refuse missing paths, absolute/escaping paths and a value map not exactly
  equal to its declared plain source file; and
- provide no generic estimator interface.

Cross-language tests feed Python-written bytes to the Rust/CLI reader and
Rust-golden bytes to the Python reader.

### 2. Deterministic sidecars

For every 2010–2024 output that exists from the accepted aggregate-first track,
write adjacent files:

```text
data/abs/params/profiled/<year>.parameters.json
data/abs/params/aggregate/<year>.parameters.json
```

The plain `<year>.json` remains the value authority for compatibility; the
sidecar `values` must equal it exactly by parameter name/numeric value.

Profiled provenance uses estimator name `data.abs.profile_fit`, the producing
source version/hash, and exactly the inputs the fitter truly consumed: the
annual O-D extract (`od_counts`), state/direction/sex/age composition extract
(`migration_composition`), frozen prior registry (`priors`) and gravity
implementation (`spatial_profile`). It must not claim to consume a
`sembla.targets/v1` ledger when it fitted those raw extracts. The profile fit
report is diagnostics.

Selected aggregate provenance uses estimator name
`data.abs.aggregate_calibrate`, the producing source version/hash, the profiled
artifact or declared prior-centre source selected by the frozen branch, all
actually consumed fitted `sembla.targets/v1` ledgers with role `targets`, and
the parity/selection/chain reports as diagnostics.
The selected branch and scientific status stay in diagnostics; the generic
parameter schema gains no method-specific fields.

Do not emit wrappers for historical retained NPE evidence in this PRD. Future
NPE runs may use the generic writer, but old evidence remains byte- and
meaning-immutable.

### 3. Maintained workflow adoption

Update the aggregate profile/selection workflow so regeneration emits sidecars
atomically after its existing plain files and evidence are complete. A failed
sidecar must not leave a partially published artifact.

The maintained selected aggregate chain must pass the aggregate
`<year>.parameters.json` sidecars to `sembla run --params`. Tests prove its
scientific outputs and resolved theta equal the legacy plain-file path and that
new run manifests bind the exact sidecar hash. Historical reproduction commands
continue accepting plain files.

### 4. Regeneration and refusal tests

Extend `scripts/check-abs-data.sh` to regenerate all sidecars twice and compare
bytes. Add tests for:

- every profile/selection branch, including prior-centre fallback;
- exact 377-value agreement with each adjacent plain file;
- exact model/hash/target/diagnostic binding;
- deterministic path/reference ordering and bytes;
- missing/stale input or diagnostic hashes;
- changed plain values after sidecar construction;
- malformed Python and Rust fixtures in both readers;
- atomic publication failure; and
- envelope-versus-legacy chain equality with manifest provenance isolated.

Capture SHA-256 inventories of `data/abs/params/gravity/**`, retained calibrated
files and retained evidence before/after; any change is a PRD failure.

### 5. Documentation

Update the aggregate calibration and parameter-artifact guides with producer,
consumer, evidence and fallback examples. Update the estimation protocol and
architecture notes to state that the maintained aggregate-first path now emits
the versioned output contract while legacy objects remain supported.

## Allowed files

- `data/abs/parameter_artifact.py` (new)
- `data/abs/profile_fit.py`
- `data/abs/aggregate_calibrate.py`
- `data/abs/chain.py` (parameter-input selection only)
- `data/abs/tests/test_parameter_artifact.py` (new)
- `data/abs/tests/test_profile_fit.py`
- `data/abs/tests/test_aggregate_calibrate.py`
- `data/abs/tests/test_calibrate.py`
- `data/abs/params/profiled/*.parameters.json` (new)
- `data/abs/params/aggregate/*.parameters.json` (new)
- `scripts/calibrate-australian-population-aggregate.sh`
- `scripts/check-abs-data.sh`
- `docs/guides/parameter-artifacts.md`
- `docs/guides/australian-population-aggregate-calibration.md`
- `docs/design/estimation-protocol.md`
- `docs/architecture/contracts.md`
- `docs/architecture/estimation.md`
- `docs/architecture/artifact-registry.json`

## Non-goals

- No edit to plain annual/gravity/profiled/aggregate/calibrated parameter values.
- No edit or relabelling of retained NPE/scientific evidence.
- No change to `calibration/npe`, runtime, CUDA, Lean, model/plan/state contracts
  or target ledgers.
- No generic estimator framework, new dependency, network access or migration
  of user files.
- No requirement that historical commands stop using legacy objects.

## Test guidance

Run:

```bash
python3 -B -m unittest discover -s data/abs/tests -p 'test_*.py'
bash scripts/check-abs-data.sh
cargo test --locked -p sembla-cli --test parameters_envelope
bash scripts/check-rust.sh
bash scripts/check-determinism.sh
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. Python and Rust independently accept/reject the same complete v1 fixtures and
   emit identical canonical bytes.
2. Every accepted profile/aggregate year has a deterministic adjacent sidecar
   exactly equal in values to its untouched plain file.
3. Estimated provenance binds the exact model, target/input and diagnostic
   bytes; stale or partial provenance cannot publish.
4. The maintained aggregate chain consumes envelopes and remains scientifically
   identical to the legacy path while manifests record source identity.
5. Two complete offline regenerations are byte-identical and atomic-failure
   tests leave no partial artifact.
6. Existing gravity, annual, calibrated and retained evidence bytes are
   unchanged.
7. Quarantine, ABS, Rust, determinism and full repository checks pass.

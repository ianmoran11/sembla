# PRD 0005: Make the registry drive contract documentation and atlas coverage

max_review_cycles: 3

## Dependencies

Contract-governance PRDs 0001–0004 are accepted. Read the binding [contract-governance README](../prds-contract-governance/README.md). The
artifact registry includes `sembla.parameters/v1`; the five architecture
canvases pass their existing readability checks.

## Context

`artifact-registry.json` is machine-checked metadata, while `Contracts.canvas`
is a hand-maintained conceptual view. The current checkers do not prove that
registry facts appear in generated documentation or that every registered
contract is represented by a conceptual canvas node. The layout should remain
human-curated; ownership/version facts must have one authority.

## Goal

Generate the complete factual contract index from the registry and make an
explicit registry projection coverage-check the conceptual Contracts canvas,
without generating geometry or making the canvas normative.

## Requirements

### 1. Complete post-demographic contract inventory

Before projecting the canvas, scan exactly these bytewise-sorted roots/extensions:

```text
crates/*/src/**/*.rs
frontend/Sembla/**/*.lean                 (exclude *Tests.lean)
calibration/npe/**/*.py                   (exclude tests, .venv, caches)
data/abs/**/*.py                          (exclude tests and caches)
data/abs/contracts/**/*.json
data/abs/params/profiled/**/*.json
data/abs/params/aggregate/**/*.json
docs/evidence/demographic-spine-foundation/**/*.json
docs/evidence/demographic-spine-foundation/**/*.md
```

Do not scan PRD prose, archive/history, dependencies, virtual environments,
build outputs or arbitrary repository Markdown. Scan production source,
generated contract files and retained evidence created by queued demographic
PRDs 0002–0011. Register
every new versioned identifier with owner, producers, consumers, kind,
stability and executable evidence. The inventory must include at least:

- `sembla.abs-profile-fit/v1`;
- `sembla.abs-expectation/v1`;
- `sembla.abs-raw-parity-contract/v1`;
- `sembla.abs-aggregate-selection/v1`;
- `sembla.demographic-spine-lifecycle/v1`; and
- every additional `sembla.*/vN` emitted by those accepted PRDs.

Extend discovery tests across the relevant generated contract/evidence roots so
a source/evidence identifier cannot exist without registration. Do not register
identifiers appearing only in PRD prose as implemented; require an actual owner
and producer path.

### 2. Registry-owned canvas projection

Add one top-level `canvas_projection` object to
`docs/architecture/artifact-registry.json`:

```json
{
  "canvas": "docs/architecture/Contracts.canvas",
  "entry_nodes": {
    "sembla.example/v1": "existing_canvas_node_id"
  }
}
```

The mapping must contain every registry identifier exactly once, contain no
extra identifier, and reference an existing non-group node in
`Contracts.canvas`. Multiple related identifiers may map to one conceptual
node. The mapping is factual coverage only; it does not claim one visual card
per schema.

Node IDs are stable internal architecture identifiers. Renaming/removing a
mapped node requires updating the registry in the same change. Every
`artifact-schema`, `data-contract`, hash/identity/encoding/protocol and embedded
version must map to a semantically appropriate concept or the explicit artifact
registry/conformance node; no silent exclusion list is allowed.

### 3. Deterministic generated index

Add `scripts/render-contract-index.py` producing
`docs/architecture/generated-contract-index.md` from the registry. It must:

- sort entries by bytewise identifier;
- include identifier, kind, stability, owner, producers, consumers and mapped
  canvas node;
- render repository-relative Markdown links for every path;
- contain a generated-file warning and exact regeneration/check command;
- write deterministic UTF-8/LF/final-newline bytes; and
- support `--check` that compares in memory and exits nonzero without rewriting.

The generated file contains no hand-maintained commentary. Commentary remains in
`contracts.md` and the canvas.

### 4. Checker integration

Extend `scripts/check-artifact-registry.py` to validate the projection and
rendered-index freshness. Extend `scripts/check-architecture-canvases.py` only
as necessary to expose/validate mapped node IDs. Keep responsibilities clear:
registry facts/coverage in the registry checker; geometry, links and styling in
the canvas checker.

`bash scripts/check.sh` runs check mode only. It must never rewrite docs.

### 5. Canvas update

Update `Contracts.canvas` so:

- `sembla.parameters/v1` replaces the former anonymous-parameter gap as an
  implemented contract;
- conceptual cards remain compact and non-overlapping;
- every mapped node has a readable role and repository path or documentation
  link; and
- one registry/conformance card links the generated index and registry.

Do not create file-preview nodes, live portals, one card per identifier or a
large unreadable identifier dump. Preserve the current conceptual groups and
relationships unless a minimal new parameter card is needed.

### 6. Tests

Focused tests must reject:

- omitted, extra or duplicate projection identifiers;
- missing/group/unknown canvas nodes;
- stale or manually edited generated rows;
- reordered/non-deterministic rendering;
- broken owner/producer/consumer links; and
- canvas overlap/file-preview regressions.

Tests generate temporary registries/canvases; they do not mutate repository
files.

### 7. Documentation

Update architecture README/contracts/fitness-function notes to state the
three-layer authority explicitly:

1. registry owns factual contract metadata and projection;
2. generated index displays all registry facts; and
3. canvas owns only conceptual layout and navigation.

## Allowed files

- `docs/architecture/artifact-registry.json`
- `docs/architecture/generated-contract-index.md` (new, generated)
- `docs/architecture/Contracts.canvas`
- `docs/architecture/README.md`
- `docs/architecture/contracts.md`
- `docs/architecture/fitness-functions.md`
- `scripts/render-contract-index.py` (new)
- `scripts/check-artifact-registry.py`
- `scripts/check-architecture-canvases.py`
- `scripts/tests/test_render_contract_index.py` (new)
- `scripts/tests/test_check_artifact_registry.py`
- `scripts/tests/test_check_architecture_canvases.py`
- `scripts/check.sh`

## Non-goals

- No generated canvas geometry or replacement of the curated conceptual view.
- No contract/schema/runtime/CLI/Lean/CUDA behavior change.
- No external Obsidian vault write from repository checks.
- No file-preview nodes, live portals, network access or new dependency.
- No manual duplicate of registry facts in another maintained table.

## Test guidance

Run:

```bash
python3 -m unittest -v \
  scripts/tests/test_render_contract_index.py \
  scripts/tests/test_check_artifact_registry.py \
  scripts/tests/test_check_architecture_canvases.py
python3 -B scripts/render-contract-index.py --check
python3 -B scripts/check-artifact-registry.py
python3 -B scripts/check-architecture-canvases.py
bash scripts/check.sh
python3 scripts/check-prd-allowlist.py <this PRD at its current path>
git diff --check
```

## Acceptance criteria

1. Every implemented identifier introduced by queued demographic PRDs 0002–0011
   is registered from real owner/producer/evidence paths, and discovery rejects
   omissions.
2. Every registry entry maps exactly once to a valid conceptual
   `Contracts.canvas` node with no exception list.
3. The generated index contains every factual field for every entry, is
   deterministic and fails check mode on any manual/stale edit.
4. Registry and canvas checkers reject all specified coverage/readability
   regressions without rewriting files.
5. The canvas presents the implemented parameter artifact and remains compact,
   non-overlapping and free of previews/portals.
6. Architecture docs state the registry/index/canvas authority split without
   making diagrams normative.
7. Existing executable contract bytes and behavior are unchanged, and focused
   plus full repository checks pass.

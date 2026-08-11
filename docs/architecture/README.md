# Architecture atlas

**Status:** Maintained navigation and orientation material. It is not a source of semantic truth.

Open [`Architecture.canvas`](Architecture.canvas) in Obsidian with the **Advanced Canvas** plugin. The atlas uses the Advanced JSON Canvas `1.0-1.0` format:

- lightweight overview cards link to nested subsystem canvases without rendering every detail at once;
- implementation files appear as compact role/path cards instead of vault-dependent file previews;
- groups show conceptual ownership and hierarchy;
- solid arrows show runtime/data dependencies;
- dashed arrows show governance, validation or conformance relationships; and
- dotted borders mark implementation choices rather than stable contracts.

## Canvas hierarchy

The overview deliberately links to—rather than live-embeds—the detailed canvases. Live portals made four complete diagrams render simultaneously, obscuring both labels and edges at overview scale.

- [`Architecture.canvas`](Architecture.canvas) — the whole system in roughly a dozen conceptual boxes.
- [`Frontend.canvas`](Frontend.canvas) — Lean IR, checking, authoring, composition, widgets, models and the production/test split.
- [`Contracts.canvas`](Contracts.canvas) — model/plan, state, run, estimation and identity/hash contracts.
- [`Runtime.canvas`](Runtime.canvas) — IR validation, CPU oracle, CUDA lowering/execution and CLI workflow orchestration.
- [`Estimation.canvas`](Estimation.canvas) — observed data, simulator-facing protocol, estimator implementations and parameter/results flow.

Each canvas links to maintained notes rather than attempting to display every source file:

- [Frontend boundary](frontend.md)
- [Contract boundary](contracts.md)
- [Runtime and backend boundary](runtime.md)
- [Estimation boundary](estimation.md)
- [Architectural fitness functions](fitness-functions.md)

## Authority

The hierarchy is deliberately split:

1. [`DESIGN.md`](../../DESIGN.md) and [`DECISIONS.md`](../../DECISIONS.md) are normative.
2. Rust/Lean interfaces, serialized artifacts and executable checks are operationally authoritative.
3. The [artifact registry](artifact-registry.json) is machine-checked ownership metadata.
4. These canvases and notes are human navigation views.

A canvas disagreement with code or a fitness function is a documentation defect; the canvas never authorizes a dependency.

## Keeping the atlas current

The conceptual node layout is maintained manually because architectural importance cannot be inferred from file counts. Several underlying facts are checked automatically:

- Cargo workspace edges: `scripts/check-rust-architecture.py`;
- Lean import boundaries: `frontend/scripts/check-imports.py`;
- versioned contract ownership: `scripts/check-artifact-registry.py` and `artifact-registry.json`;
- cross-language byte parity: `frontend/scripts/check-parity.sh`; and
- deterministic result contracts: `scripts/check-determinism.sh`.

When a checked boundary changes, update the relevant nested canvas and note in the same change. Do not add a node for every file; add one only when it represents a distinct responsibility, contract or separately changeable subsystem.

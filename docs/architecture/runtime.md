# Runtime and backend boundary

## Dependency direction

```text
                     +--- sembla-cpu <---+
                     |                   |
sembla-ir <- sembla-runtime              +--- sembla-cli
     ^               |                   |
     +---------------+--- sembla-cuda <--+
```

`sembla-cpu` and `sembla-cuda` are sibling implementations over
`sembla-runtime`. CUDA's only dependency on the CPU crate is a dev-dependency
for oracle comparisons. `sembla-cli` depends directly on each lower crate.
`scripts/check-rust-architecture.py` checks the exact workspace edge matrix,
the CUDA-to-CPU dev-only rule, and prevents backend vocabulary or direct
reporting from entering library sources.

## Responsibilities

### `sembla-ir`

Owns serialized model/plan types, semantic validation, canonical serialization and stable identities. It performs no execution and no CLI reporting.

### `sembla-runtime`

Owns backend-neutral execution contracts and deterministic primitives:

- `state.rs` — double-buffered columnar state;
- `params.rs` / `observation.rs` — resolved parameters and observation data;
- `rng.rs` / `prior.rs` — coordinate-addressed Philox and prior sampling;
- `state_artifact.rs` — portable state serialization; and
- `population.rs` — deterministic synthetic population tooling.

The hidden `engine` API exposes the small set of resolved-state primitives
required by implementation crates. Application code uses `core`; the runtime
does not select or depend on a backend. Device-observation eligibility is an
IR-only capability decision here, so CUDA does not reach into CPU evaluation.

### `sembla-cpu`

Owns the deterministic CPU oracle:

- `eval.rs` — snapshot-only expression interpretation;
- `executor.rs` — tick execution, conflict resolution and observation
  reduction; and
- `error.rs` — CPU execution failures.

CPU performance spikes and execution-specific regression tests live with this
crate, keeping CPU work independent from the runtime core and CUDA backend.

### `sembla-cuda`

Owns whole-plan CUDA C generation, NVRTC compilation and device execution. Shared plain-data host types live in `types.rs` and compile with or without the `cuda` feature. The feature-off stub reports unavailability and never hides a CPU fallback.

CPU and CUDA are not implementations of a general plugin trait. They are
sibling crates representing the reference semantics and one production
accelerator, held together by per-tick/final-state/result differential tests.
Add a third path only when a normalized kernel representation makes it cheaper
than another independent semantics implementation.

### `sembla-cli`

Owns process-level orchestration and reporting. Its source is split by workflow:

| Module | Responsibility |
| --- | --- |
| `main.rs` | Dispatch and frozen command surface |
| `shared.rs` | Input, parameter, population and identity plumbing |
| `inspect.rs` | Validation, hashing, bundle verification and IR diff |
| `synth.rs` | Synthetic population/state tooling |
| `run.rs` | Single execution, result serialization and timing |
| `sweep.rs` | Draw orchestration, concurrency, CUDA admission and pair export |
| `verify.rs` | Manifest replay and verification |
| `compare.rs` | Model/parameter-arm comparison |
| `diff_backends.rs` | CPU/CUDA conformance workflow |
| `manifest.rs` | Run/bundle/pairs provenance contracts |

The CLI prints warnings and errors returned by libraries. It is the only layer allowed to know both user workflow and concrete backend details.

## Standing complexity

Expression semantics, checked arithmetic, Philox and racing-clock behavior exist once in the CPU interpreter and once in generated device code. This duplication is deliberate but expensive. Every semantic change needs validation, both implementations and differential evidence; do not add a third copy or a backend-local expression whitelist.

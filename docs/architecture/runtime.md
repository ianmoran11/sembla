# Runtime and backend boundary

## Dependency direction

```text
sembla-ir <- sembla-runtime <- sembla-cuda <- sembla-cli
     ^              ^               ^             |
     +--------------+---------------+-------------+
```

`sembla-cli` also depends directly on each lower crate. `scripts/check-rust-architecture.py` checks the exact workspace edge matrix and prevents backend vocabulary or direct reporting from entering core library sources.

## Responsibilities

### `sembla-ir`

Owns serialized model/plan types, semantic validation, canonical serialization and stable identities. It performs no execution and no CLI reporting.

### `sembla-runtime`

Owns the deterministic CPU oracle:

- `eval.rs` — expression interpretation and `ParamEnv`;
- `executor.rs` — tick execution, conflict resolution and observation reports;
- `state.rs` — double-buffered columnar state;
- `rng.rs` / `prior.rs` — coordinate-addressed Philox and prior sampling;
- `state_artifact.rs` — portable state serialization; and
- `population.rs` — deterministic synthetic population tooling.

Device-observation eligibility lives beside the oracle because eligibility means preserving the oracle's exact observation contract. It returns plain capability data; the runtime does not select or depend on a backend.

### `sembla-cuda`

Owns whole-plan CUDA C generation, NVRTC compilation and device execution. Shared plain-data host types live in `types.rs` and compile with or without the `cuda` feature. The feature-off stub reports unavailability and never hides a CPU fallback.

CPU and CUDA are not implementations of a general plugin trait. They are the reference semantics and one production accelerator, held together by per-tick/final-state/result differential tests. Add a third path only when a normalized kernel representation makes it cheaper than another independent semantics implementation.

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

# Backend agent guide

This repository owns the Rust IR, validator, CPU runtime, CUDA backend, CLI,
runtime fixtures, and scientific execution workflows. The Lean authoring
frontend is maintained in `ianmoran11/sembla-lean`.

## Runtime boundaries

- `sembla_runtime::core` is the backend-neutral API for parameters, state,
  observation values, and device-observation eligibility.
- `sembla-cpu` is the CPU evaluator and execution crate. Its evaluator and
  executor modules are private behind the crate-root API.
- `sembla_runtime::engine` contains hidden implementation primitives required
  by execution backends; application code should use `sembla_runtime::core`.
- Production CUDA code must depend on `sembla_runtime::core`, not CPU execution
  internals. `sembla-cuda` may depend on `sembla-cpu` only as a dev-dependency
  for oracle comparisons.
- Keep backend selection and publication policy in `sembla-cli`; neither
  runtime boundary selects a backend.

## Working contract

- Run `./scripts/check-rust.sh` for ordinary Rust changes.
- Run `./scripts/check-determinism.sh` when execution or serialization changes.
- Run `./scripts/check-abs-data.sh` for Australian population data changes.
- Run `python3 -B scripts/check-frontend-compatibility.py` when changing
  frontend-emitted fixtures, canonical bytes, compatibility pins, or exported
  Australian population data.
- Do not require Lean or Lake from backend build, test, or release paths.
- Treat checked-in examples, fixtures, evidence, schema strings, hash domains,
  and canonical bytes as frozen unless a contract change explicitly authorizes
  regeneration.
- `crates/sembla-ir` is the schema of record for model and executable-plan
  input. Cross-repository changes must add backend acceptance before the Lean
  frontend begins emitting a new contract.

Frontend-generated data is exported explicitly with:

```sh
./scripts/export-frontend-data.sh /tmp/sembla-frontend-data
```

Never write into a sibling frontend checkout implicitly.

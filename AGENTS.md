# Backend agent guide

This repository owns the Rust IR, validator, CPU runtime, CUDA backend, CLI,
runtime fixtures, and scientific execution workflows. The Lean authoring
frontend is maintained in `ianmoran11/sembla-lean`.

## Working contract

- Run `./scripts/check-rust.sh` for ordinary Rust changes.
- Run `./scripts/check-determinism.sh` when execution or serialization changes.
- Run `./scripts/check-abs-data.sh` for Australian population data changes.
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

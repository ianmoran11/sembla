# Repository boundary

Sembla is maintained as two independently buildable repositories:

| Repository | Owns | Ordinary check |
| --- | --- | --- |
| [`ianmoran11/sembla`](https://github.com/ianmoran11/sembla) | Rust IR and validation, CPU/CUDA execution, CLI, runtime fixtures, data and calibration workflows | `./scripts/check.sh` |
| [`ianmoran11/sembla-lean`](https://github.com/ianmoran11/sembla-lean) | Lean authoring, checked semantics, composition linking, widgets, proofs, and canonical contract production | `./scripts/check-local.sh` |

Neither ordinary check assumes a sibling checkout. Cross-repository validation
is initiated by `sembla-lean/scripts/check-backend-compat.sh` and requires an
explicit backend path. `compat/frontend.json` and
`sembla-lean/compat/backend.json` record reviewed commits and contract names.

## Contract changes

1. Add backend acceptance while retaining the previous accepted contract.
2. Land frontend emission and conformance tests against that backend commit.
3. Update both compatibility pins after the corresponding commits are public.
4. Remove old acceptance only through a separate compatibility decision.

Frontend syntax that lowers to an existing accepted artifact can change
independently. Backend performance or execution changes can change
independently when accepted artifacts and observable semantics remain stable.

Generated Australian population tables cross the boundary only through
`scripts/export-frontend-data.sh <output-directory>`. No script writes into or
discovers a sibling checkout implicitly.

# PRD 0002 Review

## Assessment: REVISE

### Blocking findings

1. **Critical — the proposed reduction cannot report a failure.** `sembla_reset_status` initializes `status[1]` to zero (`crates/sembla-cuda/src/codegen.rs:1156`), so `atomicMin(status + 1, candidate)` can never record a nonnegative candidate (`crates/sembla-cuda/src/codegen.rs:1962-1968`). The helper never writes `status[0]`; it stores a packed key in `status[2]`, but the host only treats `status[0] != 0` as failure (`crates/sembla-cuda/src/backend.rs:931-936`) and `device_status` reads the code from `status[0]` (`crates/sembla-cuda/src/backend.rs:1121-1139`). This violates deterministic minimum-index reporting and preserves neither the status code nor candidate.
2. **Critical — the four validation kernels remain single-threaded and are still launched with `(1,1,1)`.** Their emitted openings retain the `blockIdx.x != 0 || threadIdx.x != 0` guard at `crates/sembla-cuda/src/codegen.rs:1251`, `:1351`, `:1558`, and `:1662`. Claims/effect/output loops remain scalar with bare status writes at `:1370-1373`, `:1589-1593`, and `:1734`. The host still launches `validate_transition`, `validate_claims`, `validate_effects`, and `validate_outputs` with `one` at `crates/sembla-cuda/src/backend.rs:615`, `:685`, `:778`, and `:882`; no `backend.rs` implementation diff exists. The central parallelism requirement is therefore not implemented.
3. **High — mandatory source tests are absent and required validation is red.** The diff adds no `#[test]` or assertion and changes no `crates/sembla-cuda/tests/**` file. Existing assertions still reject `atomicMin` (`crates/sembla-cuda/src/codegen.rs:2282-2283`, `:2366-2367`) and the checked-in source golden comparison remains at `:2420-2425`. `scripts/check-rust.sh` and `cargo test --locked` both exited 101 with three failures: `generation_is_deterministic_and_has_one_kernel_per_transition`, `incompatible_claims_are_checked_serially_before_parallel_argmin`, and `sir_source_matches_checked_in_golden`. Thus acceptance criteria 2-4 and 6 are not met.

### Acceptance criteria

| Criterion | Result | Evidence |
|---|---|---|
| Local 1 | FAIL | `cargo build --locked -p sembla-cuda --features cuda` passed; `cargo build --locked --workspace` passed; `scripts/check-rust.sh` failed in `sembla-cuda` tests (exit 101). |
| Local 2 | FAIL | All four kernel guards remain; only the generic helper loop at `codegen.rs:652` uses a grid stride; no required source assertion was added. |
| Local 3 | FAIL | Bare `status[1] = candidate` remains in claims at `codegen.rs:1370-1373`; equivalent bare assignments remain in effects/output; no required source assertion was added. |
| Local 4 | FAIL | Remaining status kernels retain guards, but no confinement assertion was added. |
| Local 5 | PASS | `git diff --quiet HEAD -- examples fixtures calibration crates/sembla-cli/tests/fixtures crates/sembla-cuda/tests docs/evidence` passed; protected files are byte-unchanged. |
| Local 6 | FAIL | `cargo test --locked` exited 101: 13/16 `sembla-cuda` library tests passed and three failed. |
| Hardware 7 | PENDING | `nvidia-smi` is unavailable; pending is permitted by DECISIONS.md §J14.2. |
| Hardware 8 | PENDING | No GPU run and no factored host reduction test; pending is permitted by DECISIONS.md §J14.2. |

Implementation scope is otherwise confined to allowed `crates/sembla-cuda/src/codegen.rs`; `.piprd/**` changes are runner metadata/review artifacts. Protected paths have no byte diff. `git diff --cached --quiet` passed, so there are no staged files.

### Command outcomes

- `cargo build --locked -p sembla-cuda --features cuda` — passed.
- `cargo build --locked --workspace` — passed.
- `scripts/check-rust.sh` — failed, exit 101, three `sembla-cuda` library tests failed.
- `cargo test --locked` — failed, exit 101, same three tests failed.
- Protected-path `git diff --quiet` command — passed.
- `git diff --cached --quiet` — passed.
- `nvidia-smi` availability check — unavailable; hardware criteria pending.

## Recommendation

**REVISE**

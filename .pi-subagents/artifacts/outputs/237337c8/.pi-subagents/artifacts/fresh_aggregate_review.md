## Review
- **Note (medium): aggregate failures report the reduction-buffer slot as the aggregate identity, and multi-aggregate error precedence is not CPU-ordered.** Every aggregate writes errors into the same `aggregate_errors[part]` / `aggregate_errors[2 + group]` positions (`crates/sembla-cuda/src/codegen.rs:957-978`), and `sembla_check_aggregate_errors` copies that buffer position into `status[1]` (`crates/sembla-cuda/src/codegen.rs:989`). The backend then formats `status[1]` as an aggregate number (`crates/sembla-cuda/src/backend.rs:794-798`). For example, aggregate 5 overflowing in partial 0 is reported as “aggregate 0 overflowed Int.” Because each phase checks only after all aggregates have run (`crates/sembla-cuda/src/backend.rs:413-456`, `559-601`, `647-689`), simultaneous failures are selected by part/group slot rather than aggregate/evaluation order, unlike the CPU’s fail-fast expression evaluation (`crates/sembla-runtime/src/executor.rs:649-680`, `720-755`). Give each error record its aggregate index and preserve generated/CPU evaluation order (without atomics), and add an exact multi-aggregate diagnostic test.
- **Note (low): the runtime evaluator’s module contract still documents the old reduction order.** `crates/sembla-runtime/src/eval.rs:6-7` says aggregate sums use one sequential ascending-row pass, while the implementation now deliberately uses two contiguous halves and a fixed merge (`crates/sembla-runtime/src/eval.rs:1543-1545`, `1594-1654`). Update the module documentation so it describes the new canonical Level-A schedule.

## Residual CUDA-hardware uncertainty
Neither `nvidia-smi` nor `nvcc` is available on this host. The CUDA-feature Rust targets compile, but NVRTC compilation and execution of generated CUDA were not exercised. Consequently the ignored GPU tests for phase/liveness behavior, prospective/shared aggregates, ordered Int/Int `f64` promotion, Philox equality, oracle hashes, and repeatability remain hardware-unanswered, consistently with `crates/sembla-cuda/GPU-STATUS.md`.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Two actionable findings are cited with severity and exact file/line evidence; CUDA-hardware uncertainty is stated separately."
    }
  ],
  "changedFiles": [
    "crates/sembla-cuda/src/backend.rs",
    "crates/sembla-cuda/src/backend_stub.rs",
    "crates/sembla-cuda/src/codegen.rs",
    "crates/sembla-cuda/src/error.rs",
    "crates/sembla-cuda/src/lib.rs",
    "crates/sembla-cuda/tests/fixtures/sir.generated.cu",
    "crates/sembla-runtime/src/eval.rs",
    "crates/sembla-runtime/src/executor.rs",
    "crates/sembla-runtime/tests/eval.rs",
    "rust-toolchain.toml",
    "crates/sembla-cuda/GPU-STATUS.md",
    "crates/sembla-cuda/README.md",
    "crates/sembla-cuda/scripts/run-gpu-tests.sh",
    "crates/sembla-cuda/tests/absence.rs",
    "crates/sembla-cuda/tests/gpu_oracle.rs",
    "crates/sembla-cuda/tests/gpu_philox.rs",
    "crates/sembla-cuda/tests/gpu_semantics.rs"
  ],
  "testsAddedOrUpdated": [
    "crates/sembla-cuda/src/codegen.rs",
    "crates/sembla-cuda/tests/fixtures/sir.generated.cu",
    "crates/sembla-cuda/tests/absence.rs",
    "crates/sembla-cuda/tests/gpu_oracle.rs",
    "crates/sembla-cuda/tests/gpu_philox.rs",
    "crates/sembla-cuda/tests/gpu_semantics.rs",
    "crates/sembla-runtime/tests/eval.rs"
  ],
  "commandsRun": [
    {
      "command": "git status --short; git diff --stat; git diff --cached --stat; git diff --name-only",
      "result": "passed",
      "summary": "Inspected the uncommitted surface and confirmed the staged diff is empty."
    },
    {
      "command": "git diff --check",
      "result": "passed",
      "summary": "No whitespace errors."
    },
    {
      "command": "cargo fmt --all -- --check",
      "result": "passed",
      "summary": "Formatting is clean."
    },
    {
      "command": "cargo test -p sembla-cuda",
      "result": "passed",
      "summary": "14 codegen/unit tests and 2 absence tests passed; feature-gated GPU tests were not selected."
    },
    {
      "command": "cargo test -p sembla-runtime",
      "result": "passed",
      "summary": "Runtime unit/integration tests passed; one release-only performance tripwire remained ignored."
    },
    {
      "command": "cargo test -p sembla-cuda --features cuda --no-run",
      "result": "passed",
      "summary": "All CUDA-feature Rust test targets compiled without executing a device backend."
    },
    {
      "command": "cargo test -p sembla-cuda --features cuda semantic_gpu_fixtures_validate_without_a_device",
      "result": "passed",
      "summary": "The CPU/codegen semantic fixture test passed with the CUDA feature enabled."
    },
    {
      "command": "cargo test --workspace",
      "result": "passed",
      "summary": "Default-feature workspace test suite passed; only the documented release performance test was ignored."
    },
    {
      "command": "grep -RIn -E 'atomic(Add|Min|Max|CAS|...)|atomic_' crates/sembla-cuda",
      "result": "passed",
      "summary": "No emitted CUDA atomic operation was found; matches were only negative source assertions."
    },
    {
      "command": "command -v nvidia-smi; command -v nvcc",
      "result": "passed",
      "summary": "Neither CUDA hardware tooling command is available on this host."
    }
  ],
  "validationOutput": [
    "Scheduling aggregates are launched against current `state` before transition kernels at crates/sembla-cuda/src/backend.rs:413-496.",
    "Effect-only aggregate activity is derived from winning candidates and evaluated before effect preparation at crates/sembla-cuda/src/codegen.rs:912-929 and crates/sembla-cuda/src/backend.rs:548-625.",
    "Wired-output aggregate slots are rebuilt against `next_state` at crates/sembla-cuda/src/backend.rs:644-683; output collection walks only wires at crates/sembla-cuda/src/codegen.rs:238-263.",
    "Shared schedule/output aggregates are placed in both phase lists by crates/sembla-cuda/src/codegen.rs:799-818 and covered by crates/sembla-cuda/src/codegen.rs:1811-1818; unwired aggregate omission is covered at lines 1747-1754.",
    "Rows::Input ordered comparisons cast both numeric operands to double at crates/sembla-cuda/src/codegen.rs:634-650, matching CPU input_number conversion at crates/sembla-runtime/src/eval.rs:1178-1182.",
    "GPU semantic fixtures cover stale prospective-output overflow, transition-only post-effect overflow, inactive/active effect aggregates, and >2^53 input ordering at crates/sembla-cuda/tests/gpu_semantics.rs:239-300 and 349-409.",
    "No result-bearing CUDA atomics were found by source search."
  ],
  "residualRisks": [
    "Generated CUDA was not compiled by NVRTC or executed because no CUDA GPU/toolkit tooling is available.",
    "All ignored device oracle, Philox, repeatability, and GPU semantic tests remain unanswered on hardware.",
    "Aggregate device diagnostics currently lose aggregate identity and CPU error precedence."
  ],
  "noStagedFiles": true,
  "diffSummary": "10 tracked files are modified (1674 insertions, 341 deletions) and the CUDA README/status/script plus four integration-test files are new; changes implement phased aggregate generation/execution, fixed two-part reductions, prospective outputs, checked parallel staging, and CUDA acceptance coverage.",
  "reviewFindings": [
    "medium: crates/sembla-cuda/src/codegen.rs:957-989 and crates/sembla-cuda/src/backend.rs:794-798 - aggregate error slots are mislabeled as aggregate IDs and do not preserve CPU aggregate error precedence",
    "low: crates/sembla-runtime/src/eval.rs:6-7 - module documentation still claims the removed one-pass reduction order",
    "no blockers"
  ],
  "manualNotes": "Read-only review; implementation files were not modified. The required review artifact is the only file written."
}
```

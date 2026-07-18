# Code Context

## Files Retrieved
1. `docs/prds-npe-path/0008-cuda-backend.md` (lines 1-68) - production requirements: optional compilation, deterministic NVRTC codegen, fixed order, no fallback, ignored GPU tests, unanswered reporting.
2. `Cargo.toml` (lines 1-16) - root workspace has three explicit members, resolver 2, shared version/edition, and no workspace dependency convention yet.
3. `spikes/precision/Cargo.toml` (lines 1-34) - detached nested workspace; `default = []`, empty `cuda` feature.
4. `spikes/precision/build.rs` (lines 1-100) - feature/toolkit probing and conditional nvcc/static-runtime linking.
5. `spikes/precision/src/cuda.rs` (lines 1-145, 220-369) - `CudaStatus`, `CudaError`, status gating, FFI probe/run/benchmark, and absence tests.
6. `spikes/precision/src/cuda/f64_native.cu` (lines 1-150, 151-390) - Philox, fixed two-pass reduction, map, lexicographic argmin, retained benchmark allocations, C ABI/device probe.
7. `spikes/precision/README.md` (lines 1-180) - isolation/build commands, exact arithmetic contract, retained buffers, and honest result conventions.
8. `spikes/precision/infra-hyperstack/README.md` (lines 1-215) - safe remote lifecycle, validation, evidence collection, and mandatory destruction.
9. `spikes/precision/infra-hyperstack/remote-run-spike.sh` (lines 1-76) - toolkit/device checks, provenance exports, locked CUDA invocation, completion hash.
10. `crates/sembla-ir/src/validate.rs` (lines 55-108) - `ValidatedTransition`, `ValidatedModel`, stable transition/rule assignment.
11. `crates/sembla-runtime/src/rng.rs` (lines 1-105) - authoritative coordinate Philox and open-interval `f64` conversion.
12. `crates/sembla-runtime/src/eval.rs` (lines 300-570 and surrounding evaluator implementation) - expression structural/syntax-tree semantics and aggregate cache.
13. `crates/sembla-runtime/Cargo.toml` (lines 1-11) and `crates/sembla-ir/Cargo.toml` (lines 1-12) - current minimal path-dependency convention.

## Key Code

- `ValidatedModel::transitions()` preserves validator-assigned source order; `validate()` assigns `rule_id` while iterating boxes then transitions (`crates/sembla-ir/src/validate.rs:64-108`). This is the canonical order for generated transition kernels and names.
- CPU randomness is pure on `(seed, tick, rule_id, entity_id, draw_idx)`; `draw_u32x4` and `uniform_f64` define exact packing and conversion (`crates/sembla-runtime/src/rng.rs:1-105`). The spike CUDA functions `philox4x32_10` and `uniform_open_f64` mirror it (`spikes/precision/src/cuda/f64_native.cu:28-55`).
- Spike ordering is concrete: two ascending halves then left+right merge (`reduce_partial_kernel`, `reduce_finish_kernel`, lines 59-81), and contested winner key `(t_bits, rule_id, entity_id)` (`argmin_kernel`, lines 104-128). No result-bearing atomics occur.
- Spike compile flags are important parity evidence: `--fmad=false --prec-div=true --prec-sqrt=true` (`spikes/precision/build.rs:35-47`). NVRTC equivalents should be passed explicitly (`--fmad=false`, `--prec-div=true`, `--prec-sqrt=true`; plus a fixed C++ standard and architecture policy), and the exact option vector should be testable.
- Existing `CudaStatus::{FeatureDisabled, ToolkitAbsent, DeviceUnavailable(String), Available}` produces stable diagnostics (`spikes/precision/src/cuda.rs:25-49`), but the spike treats absence as a successful benchmark no-op (`run_cuda_f64_accuracy_smoke`, lines 120-127). Production construction must instead return a typed error and never return a CPU executor.

## Architecture

### Recommended dependency and feature design

Add `crates/sembla-cuda` as a normal workspace member so its pure codegen/golden tests always run. Give it:

```toml
[features]
default = []
cuda = ["dep:cudarc"]
[dependencies]
sembla-ir = { path = "../sembla-ir" }
sembla-runtime = { path = "../sembla-runtime" }
cudarc = { version = "<pinned after API check>", optional = true, default-features = false, features = ["driver", "nvrtc", "dynamic-loading"] }
```

Then any referencing production crate uses `sembla-cuda = { path = "../sembla-cuda", optional = true }` and `cuda = ["dep:sembla-cuda", "sembla-cuda/cuda"]`. Keep every default feature empty. Do **not** use a production `build.rs`, `nvcc`, `cc`, CUDA headers, static `cudart`, or link directives: those make feature builds/toolkit discovery occur at compile/link time and repeat the spike's cfg complexity. Runtime NVRTC is required for arbitrary models anyway.

`cudarc` is the most feasible high-level Rust choice because it covers Driver API memory/module/launch plus NVRTC and can dynamically load CUDA libraries. `cust` is a viable driver abstraction but commonly needs a separate NVRTC binding and has historically had toolkit/build-script friction; raw `cuda-driver-sys`/`nvrtc-sys` maximizes control but greatly expands unsafe surface and may link eagerly. **Unanswered dependency detail:** verify and pin the current `cudarc` release and exact feature names/MSRV before implementation; they are not present in `Cargo.lock`, and this read-only analysis did not fetch crates.io. Dynamic loading must be confirmed by a clean non-CUDA host build/test.

This interpretation preserves root `cargo build/test --workspace`: `sembla-cuda` itself compiles pure APIs/codegen without its `cuda` feature, while runtime-bound modules are `#[cfg(feature = "cuda")]`. A feature-off `Backend::new` entry can return `CudaUnavailable::FeatureDisabled`, or the constructor can only exist behind the feature; prefer an always-present request API for explicit local diagnostics and CLI wiring.

### Deterministic generation and dump

Create a pure `generate(model: &ValidatedModel) -> Result<GeneratedCuda, CodegenError>` independent of CUDA. Iterate slices in model/validator order; never `HashMap` iteration. If lookup maps are needed, sort keys or use `BTreeMap`. Emit one normalized UTF-8 source string with fixed `\n`, fixed indentation, fixed helper order, transition symbols such as `sembla_transition_{rule_id:08x}`, and real constants via exact hexadecimal bit reconstruction (for example `__longlong_as_double(0x...ULL)`) rather than locale/decimal formatting. Preserve AST operand order and parentheses; do not algebraically simplify/reassociate. Hash the generated bytes for cache/module identity.

Dump only after successful generation and before compilation. Recommended `SEMBLA_CUDA_DUMP_DIR`: create `<model-source-sha256>.cu` atomically (temporary file + rename), never timestamp/PID-based names, and return a clear I/O error rather than silently skipping. Repeated generation must produce identical bytes and the same path. Tests should compare two generations, compare models built independently, and check a committed SIR golden file.

NVRTC compilation should consume exactly those bytes and a fixed options vector. Cache PTX by `(source hash, compiler options, NVRTC version, compute capability)`; PTX itself need not be a golden because toolchain versions change. Record the source hash/options/NVRTC and driver versions in diagnostics.

### Explicit absence and testability

Separate pure orchestration from the real loader via a small internal trait (for example `CudaApi { load_driver; device_count; create_context; load_nvrtc; compile; ... }`) or injectable probe function. `Backend::new_with_api` can deterministically test:

- driver library missing -> `cuda backend unavailable: CUDA driver library not found`;
- `device_count == 0` / CUDA no-device -> `cuda backend unavailable: no CUDA device found`;
- NVRTC library missing -> `cuda backend unavailable: NVRTC library not found`;
- compile failure -> include NVRTC log and dumped-source path when present.

The public `Backend::new` uses the real implementation. No-device tests must assert the typed variant and stable display string using a fake API, and assert no CPU/runtime executor is constructed. This is superior to relying on the developer machine truly having no GPU. A feature-off test should also assert the explicit feature-disabled diagnostic.

### Test split and honest reporting

Always-run tests: generator determinism, SIR golden, symbol/order checks, exact literal emission, dump naming/content, feature-disabled error, injected driver/NVRTC/no-device errors. GPU tests under `tests/gpu_*.rs` should each be `#[test] #[ignore = "requires CUDA GPU; run via ..."]`: Philox vectors; SIR/sir_policy/canonical per-tick oracle hashes; same-run-twice Level A. Ignoring should be unconditional, not based on probing and returning early, so an explicitly requested remote run fails honestly if prerequisites are absent.

Provide a production adaptation of the Hyperstack runner (not reuse its spike result mutation): verify `nvidia-smi` and NVRTC/toolkit, run `cargo test --locked --release -p sembla-cuda --features cuda -- --ignored --nocapture`, capture commit/device/driver/toolkit and test log, hash artifacts, then destroy immediately. Correctness may use any CUDA GPU; label throughput claims full-rate only after runtime hardware verification. If no remote GPU was used, report criteria 2-4 individually as `unanswered (GPU not run)` with the exact runbook command—never “passed,” zero mismatches, or simulated output.

### Proposed file layout

```text
crates/sembla-cuda/
  Cargo.toml
  src/lib.rs                 # always-present request/status/error API
  src/error.rs               # typed FeatureDisabled/DriverMissing/NoDevice/NvrtcMissing/etc.
  src/codegen/mod.rs
  src/codegen/expr.rs        # AST-order CUDA expression emission
  src/codegen/template.cuh   # Philox/shared fixed-order helpers
  src/backend.rs             # cfg(cuda), context/module/device-resident state
  src/api.rs                 # injectable CUDA/NVRTC adapter
  src/reduction.rs           # launch plans/fixed shapes
  tests/codegen.rs
  tests/fixtures/sir.generated.cu
  tests/absence.rs
  tests/gpu_philox.rs
  tests/gpu_oracle.rs
scripts/run-cuda-tests-hyperstack.sh (or documented adaptation under spikes/precision/infra-hyperstack/)
```

## Findings / Risks

- **High:** copying the spike `build.rs` design into production would make CUDA toolkit discovery/build coupling unnecessary and cannot compile arbitrary model kernels; use runtime NVRTC/dynamic Driver loading.
- **High:** CUDA `log` and CPU `f64::ln` bit equality across libdevice/libm is not established merely by disabling FMA. PRD demands bitwise full-state equality, so generated transcendental semantics require real GPU differential evidence; report unanswered until run.
- **High:** “same fixed order as CPU oracle” must be derived from production runtime scatter/aggregate/effect order, not assumed from the spike's fixed two-half workload. The spike proves one shape only.
- **Medium:** workspace feature language can be misread as excluding the crate itself. Excluding it would prevent always-on codegen tests; include the crate, gate only CUDA dependencies/modules, and gate optional references from other members.
- **Medium:** dynamic loading permits compilation on non-CUDA hosts, but `--features cuda` runtime behavior depends on the selected binding truly avoiding link-time CUDA requirements; validate on clean CI before pinning.
- **Medium:** env-var dumps can leak model details and collide under concurrency; content-addressed names plus atomic write address determinism/collision, but permissions/data policy remain to document.
- **Low:** current worktree already has unrelated modified/untracked `.piprd` and `.pi-subagents` files. No staged files were observed; this scout changed only its required artifact.

## Start Here

Open `spikes/precision/src/cuda/f64_native.cu` first for the verified Philox/reduction/argmin arithmetic contract, then implement the pure generator around `ValidatedModel::transitions()` from `crates/sembla-ir/src/validate.rs` before introducing any CUDA binding.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings include exact source paths/symbols, severity-ranked risks, dependency/feature design, test strategy, and proposed layout."
    }
  ],
  "changedFiles": [
    ".pi-subagents/artifacts/outputs/9a9cb7f2/.pi-subagents/prd0008-cuda-analysis.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "git status --short; grep CUDA binding names in Cargo.lock",
      "result": "passed",
      "summary": "Found no existing CUDA/NVRTC binding lock entries; observed unrelated pre-existing worktree changes and no staged entries."
    },
    {
      "command": "targeted file discovery/read/grep under docs, crates, and spikes/precision",
      "result": "passed",
      "summary": "Mapped workspace, CUDA spike, Hyperstack runner, ValidatedModel, evaluator, and Philox sources."
    }
  ],
  "validationOutput": [
    "Read-only analysis completed; no build or GPU test was claimed.",
    "CUDA criteria 2-4 remain explicitly unanswered because no GPU run was performed."
  ],
  "residualRisks": [
    "Exact current cudarc version/features/MSRV and dynamic-loading behavior require verification before pinning.",
    "CPU libm versus CUDA libdevice transcendental bit equality remains unproven.",
    "Production fixed scatter/reduction order requires deeper executor integration analysis."
  ],
  "noStagedFiles": true,
  "diffSummary": "Added only the required scouting analysis artifact; repository source was not edited.",
  "reviewFindings": [
    "high: spikes/precision/build.rs:33-74 - nvcc/static cudart approach should not be copied to arbitrary-model NVRTC production backend",
    "high: spikes/precision/src/cuda/f64_native.cu:83-100 - CUDA log/libdevice bit equality with CPU evaluator remains unproved",
    "medium: Cargo.toml:1-7 - add sembla-cuda as a workspace member but gate CUDA dependencies/modules, not pure codegen tests"
  ],
  "manualNotes": "No GPU was reachable or invoked in this scout; remote acceptance must report unanswered until the ignored tests are genuinely run."
}
```

# CUDA aggregate semantic parity audit

## Blockers first

**No code blocker found in the inspected uncommitted CUDA aggregate changes.** The classification, activation, dependency order, phase-state selection, fixed two-part reductions, zero-row paths, and input-table ordered numeric conversion are internally consistent with the current CPU evaluator/executor implementation.

Real-GPU execution remains unanswered: this host has no `nvidia-smi`, and the eight CUDA semantic parity tests in `crates/sembla-cuda/tests/gpu_semantics.rs` remain ignored without a CUDA GPU/toolchain runtime.

## Concrete findings

1. **Low — aggregate and output overflow diagnostics report an error-buffer slot as though it were the aggregate/field index.**
   - `crates/sembla-cuda/src/codegen.rs:989` stores `status[1] = i`, where `i` is the shared aggregate error-buffer position (`0/1` for partials or `2 + group` for merge errors), not `aggregate_index`.
   - `crates/sembla-cuda/src/backend.rs:796-797` renders that value as `aggregate {status[1]}`.
   - Likewise, `crates/sembla-cuda/src/codegen.rs:1359,1399,1408` uses partial/merge error slots, while `crates/sembla-cuda/src/backend.rs:810` calls the raw slot a wire output field.
   - Impact: failure detection and rollback remain correct, but diagnostics can identify the wrong aggregate/output field. This is not a state-semantic blocker.

2. **Low — evaluator module documentation is stale after the fixed two-part schedule change.**
   - `crates/sembla-runtime/src/eval.rs:6-7` still says aggregate sums use one sequential ascending pass.
   - Actual input and group reductions use two contiguous halves and merge partial 0 then partial 1 at `crates/sembla-runtime/src/eval.rs:961-1019` and `:1543-1655`; outputs do the same at `crates/sembla-runtime/src/executor.rs:865-904`.
   - Impact: documentation only; implementation parity is consistent.

## Inherited decisions

- Audit is read-only with respect to repository source; no product files were edited.
- CPU `eval.rs`/`executor.rs` is the semantic oracle.
- Moore outputs observe prospective state and are fallible before commit (`crates/sembla-runtime/src/executor.rs:342-355`).
- The intended Level-A reduction schedule is now two fixed contiguous halves followed by partial 0 + partial 1.
- Wired outputs are observable; unwired outputs are not evaluated by the CPU executor (`crates/sembla-runtime/src/executor.rs:812-831`).

## Diagnosis

- **Classification and shared uses:** `collect_all` assigns schedule use to guards, hazards, claims, and claim keys; effect use to effect values; and output use only to wired output expressions (`crates/sembla-cuda/src/codegen.rs:213-263`). A structurally shared aggregate accumulates all uses (`:332-334`). Schedule-shared effect aggregates are intentionally omitted from the effect-only list and reuse tick-start results (`:799-817`).
- **Nested dependency order:** recursive collection visits an aggregate's filter/value before inserting the outer aggregate (`crates/sembla-cuda/src/codegen.rs:327-373`). Backend phase loops preserve generated index order (`crates/sembla-cuda/src/backend.rs:413-449`, `:559-596`, `:647-683`), so nested dependencies are ready before consumers.
- **Activation:** effect-only aggregates are marked active only when one of their owning rules has a winning row (`crates/sembla-cuda/src/codegen.rs:912-931`), matching CPU effect-column evaluation only after a transition has at least one winner (`crates/sembla-runtime/src/executor.rs:745-773`). Shared schedule/effect aggregates are already eagerly evaluated because the schedule occurrence itself is CPU-live.
- **Phase state:** schedule and effect aggregate launches read `self.state` (`crates/sembla-cuda/src/backend.rs:418`, `:565`); output aggregates and output fields read `self.next_state` (`:652`, `:712`), matching CPU prepared-snapshot output construction.
- **Error buffers:** aggregate errors are reset at tick start (`crates/sembla-cuda/src/backend.rs:404-410`), part errors use distinct slots, merge errors use `2 + group`, and phase checks preserve an earlier nonzero status (`crates/sembla-cuda/src/codegen.rs:957-989`). No stale-success counterexample was found. The only issue found is diagnostic identity, above.
- **Zero rows:** two aggregate partials still initialize zero results; a zero group count skips the finish launch (`crates/sembla-cuda/src/backend.rs:433-448`), while a wired zero-row output still emits one zero aggregate row through `next_input_counts = 1` (`crates/sembla-cuda/src/codegen.rs:1291-1303`) and zero-valued partials (`:1337-1366`), matching CPU `build_output`'s one-row table.
- **Input ordered comparison:** CPU converts both ordered numeric operands to `f64` (`crates/sembla-runtime/src/eval.rs:1101-1112`); CUDA now promotes both operands for `Rows::Input` ordered comparisons (`crates/sembla-cuda/src/codegen.rs:634-649`). Equality remains exact for Int/Int as on CPU.

## Drift / contradiction check

- No implementation drift was found in the requested semantic areas.
- The only quiet contradiction is the stale “one sequential pass” documentation in `eval.rs:6-7`; the code and updated tests consistently use two halves.
- Error reporting quietly conflates buffer coordinates with semantic aggregate/field identity, but error presence, rollback, and state/input preservation are not affected.

## Recommendation

- Treat the current change set as having **no code-blocking semantic parity defect** based on static audit and host-side tests.
- Before claiming end-to-end CUDA parity, run `crates/sembla-cuda/scripts/run-gpu-tests.sh` on real NVIDIA hardware so the eight ignored tests exercise NVRTC compilation, kernel launch ordering, device error buffers, nested prospective aggregates, and exact per-tick hashes.
- Separately correct the two low-severity diagnostic/documentation issues when convenient.

## Risks

- Real-GPU NVRTC compilation and kernel execution were not available on this host.
- Static/generated-source tests cannot prove device scheduling, CUDA ABI argument alignment, or hardware floating-point bit identity.
- Existing GPU semantic tests cover key requested cases, but zero-row behavior and shared schedule+effect+output use do not have an explicit real-GPU counterexample test in `gpu_semantics.rs`.

## Need from main agent

None. No blocking decision is required.

## Suggested execution prompt

No executor handoff is warranted for this read-only audit. A later GPU-capable validation run is warranted, not a source implementation handoff.

## Validation summary

- `cargo test -p sembla-runtime`: passed (all non-ignored runtime unit/integration/doc tests; one release-only performance test ignored).
- `cargo test -p sembla-cuda --features cuda --tests`: passed all host-runnable tests; 8 `gpu_semantics` tests, 4 `gpu_oracle` tests, and 1 `gpu_philox` test were ignored because they require a CUDA GPU.
- `cargo test -p sembla-cuda --features cuda --lib`: passed 14/14.
- Targeted no-device semantic fixture: passed 1/1.
- `git diff --check`: passed with no output.
- `git diff --cached --name-only`: empty; no staged files.
- `nvidia-smi`: unavailable; no real-GPU run performed.
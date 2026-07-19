# Implementation Plan

## Goal
Restore the frozen CPU oracle and make CUDA generation, reductions, and device-side validation match it for every validated v0.1 model without changing IR semantics or adding a fallback.

## Tasks
1. **Freeze the three review counterexamples as regressions before refactoring**: Add focused fixtures for hostile model names, sequential reduction order, and cross-transition error precedence.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: Add an always-running codegen test using a validated model name containing `\n#error`; require deterministic generation and require that the payload cannot start a generated-source line/directive.
   - File: `crates/sembla-runtime/tests/eval.rs`
   - Changes: Restore the cancellation-order regression for `[1e16, 1.0, -1e16, 1.0]` with the established sequential result `1.0`, not the current two-half result `0.0`.
   - File: `crates/sembla-cuda/tests/gpu_semantics.rs`
   - Changes: Add ignored GPU differentials for grouped, input-table, and wire-output real sums with the cancellation vector; add checked-Int order cases such as `[i64::MAX, 1, -1]`, where sequential evaluation must overflow even though two-half reassociation can hide the overflow. Add a two-transition model where transition 0 has an overflowing claim key and transition 1 has an overflowing guard, and require CUDA to preserve the transition-0 claim failure.
   - Acceptance: The new source-injection test fails on the current raw comment; the restored CPU test demonstrates the frozen oracle result; the GPU tests are checked in as `#[ignore]` and validate on CPU without requiring a device.

2. **Remove the generated-source trust-boundary violation without narrowing the IR**: Never place a raw validated string in CUDA source.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: Replace `// model: {raw_name}` with an ASCII-only digest such as `// model-name-sha256: {hex}` or omit the model-name line entirely. Prefer the digest because `sha2` and hex rendering already exist and arbitrary Unicode, CR/LF, directives, and trailing backslashes cannot affect preprocessing. Do not add name restrictions to `sembla-ir`; the PRD requires CUDA to accept every already-valid model.
   - File: `crates/sembla-cuda/tests/fixtures/sir.generated.cu`
   - Changes: Regenerate the golden after the safe label change.
   - Acceptance: A model named `ok\n#error injected` validates, generates deterministic source, and the generated source remains valid CUDA/C++ with no live injected directive.

3. **Restore the CPU oracle exactly rather than redefining it for CUDA**: Revert only the reduction-order hunks introduced by PRD 0008.
   - File: `crates/sembla-runtime/src/eval.rs`
   - Changes: Restore sequential ascending-row accumulation for input aggregates and grouped aggregates, including the original checked-Int overflow point and filter/value evaluation order. Restore the module documentation to the sequential canonical CPU order. Preserve unrelated evaluator fixes such as input-row ordered-comparison `f64` conversion.
   - File: `crates/sembla-runtime/src/executor.rs`
   - Changes: Restore sequential ascending-row wire-output Count/Sum behavior and original overflow diagnostics; do not revert unrelated prospective-state output logic.
   - File: `crates/sembla-runtime/tests/eval.rs`
   - Changes: Replace the two-half expectation with the frozen sequential expectation and retain repeatability coverage.
   - Acceptance: Default runtime hashes and semantics are unchanged from `HEAD`; the cancellation vector returns `1.0`; checked-Int overflow occurs at the same row as before; `git diff` contains no remaining CPU reduction reassociation.

4. **Use a correctness-first serial CUDA fold while retaining deterministic two-stage publication**: Adapt CUDA to the oracle instead of splitting the oracle into halves.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: For input aggregate helpers, replace two half accumulators and their merge with one accumulator folded from row 0 through `count - 1`. Preserve CPU evaluation order: validate/evaluate filters across all rows before Sum values, then fold selected values in ascending order. Because expressions are pure, the smallest buffer-free implementation may validate in one loop and deterministically recompute in the fold loop.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: For grouped aggregates, have only partial slot 0 initialize every group and scan the full target table in ascending row order. Evaluate the complete filter column before the complete Sum-value column, then perform the ordered group fold. Keep the finish kernel as a pure bitwise publication/copy from partial 0 to `aggs`; do not add `+ 0.0` or merge an identity partial, because an extra FP operation can change signed zero or NaN payload behavior. For Int, use checked addition at each sequential row and perform no final merge addition.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: For wire-output fields, launch one worker per field, scan that field’s rows sequentially, and have the finish kernel copy the stored bits directly. Preserve CPU field ordering in the serial error checker. Keep existing two-slot allocations initially if that minimizes layout churn, but slot 1 must be semantically unused and never arithmetically merged.
   - File: `crates/sembla-cuda/src/backend.rs`
   - Changes: Launch one aggregate partial worker instead of two and one output partial worker per field instead of two. Update error counts/signatures as needed. Keeping the existing `aggregate_partials` and `output_partials` allocation sizes is acceptable for this revision; correctness is more important than reclaiming the unused half.
   - Acceptance: Generated CUDA contains no midpoint split or partial-0/partial-1 arithmetic merge on result-bearing paths; grouped/input/output cancellation and checked-overflow GPU differentials match the restored CPU oracle; no result-bearing atomics are introduced.

5. **Interleave device-side transition and claim validation in CPU declaration order**: Stop batching all guard/hazard errors before all claims.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: Give candidate errors separate guard and hazard planes (or equivalent distinct slots) so a serial checker can scan every guard row before every hazard row, matching `eval_column(guard)` followed by `eval_column(hazard)`. Reset `local_error` between the two expressions. Extend the transition kernel with a read-only status argument so later work returns when an earlier transition already failed.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: Parameterize claim validation by transition/rule instead of validating all transitions in one monolithic pass. Emit claim-major loops: for each claim in declaration order, evaluate its resource for all rows, then its key for all rows. This matches the CPU’s column evaluation; the current row-major/claim-minor loop does not. Move incompatible-ordering candidate-pair validation into a separate serial kernel that runs only after every transition has successfully staged candidates.
   - File: `crates/sembla-cuda/src/backend.rs`
   - Changes: In the existing transition loop, enqueue on the same CUDA stream: (1) that transition’s guard/hazard/draw kernel, (2) a serial candidate-error check limited to that transition’s candidate range and ordered guard plane then hazard plane, and (3) claim validation for that transition. After the loop, enqueue the separate compatibility validator and then conflict resolution. Every checker must preserve an already-set status. This remains device-resident: do not download status or branch on the host between transitions.
   - File: `crates/sembla-cuda/src/backend.rs`
   - Changes: Allocate/reset two candidate-error planes with checked size arithmetic, including the zero-candidate sentinel case. Load any renamed/split validation kernels and update launch ABIs.
   - Acceptance: The transition-0-claim/transition-1-guard regression reports the claim failure; same-transition guard errors precede hazard errors regardless of row; claim 0 errors precede claim 1 errors according to CPU column order; disabled candidates still do not participate in compatibility resolution.

6. **Audit aggregate-error precedence before declaring parity complete**: Phase-batched aggregate precomputation can create the same class of fail-fast mismatch as phase-batched claims.
   - File: `crates/sembla-cuda/src/codegen.rs`
   - Changes: Add an adversarial model where an earlier transition has a scalar guard/claim error and a later transition’s scheduling aggregate overflows. Verify the CPU error remains first. If current eager aggregate status setting preempts it, defer per-aggregate errors and surface them at the aggregate’s first semantic use, or build scheduling aggregates according to an ordered per-transition use plan rather than setting global status for the entire aggregate list up front.
   - File: `crates/sembla-cuda/src/backend.rs`
   - Changes: If needed, replace the flat scheduling-aggregate launch/check block with generated ordered first-use metadata. Preserve aggregate caching/deduplication and do not rebuild a shared aggregate after its first successful evaluation.
   - Acceptance: Aggregate, guard, hazard, and claim failures are selected in the same transition/expression order as the CPU for the added adversarial cases. Do not treat the narrower cross-transition claim fix as sufficient until this audit passes.

7. **Update deterministic artifacts and documentation to describe the restored contract**: Remove claims that the CPU oracle was changed to two row halves.
   - File: `crates/sembla-cuda/README.md`
   - Changes: Document the correctness-first ordered CUDA fold/two-stage publication and its performance tradeoff; state that the existing CPU sequential order remains authoritative. Retain NVRTC, no-fallback, device-residency, and honest-reporting documentation.
   - File: `crates/sembla-cuda/tests/fixtures/sir.generated.cu`
   - Changes: Regenerate only through `examples/generate_sir_golden.rs` after codegen is final.
   - Acceptance: README, runtime module docs, source golden, and emitted kernels all describe/implement the same sequential arithmetic order.

8. **Run the full local and remote-capable verification matrix**: Validate feature gating, source syntax, parity fixtures, and honest GPU status.
   - File: `scripts/check.sh`
   - Changes: No change expected; run it.
   - Acceptance: Run `cargo fmt --all --check`, `cargo build --workspace --locked`, `cargo test --workspace --locked`, `cargo check -p sembla-cuda --features cuda --all-targets --locked`, `cargo test -p sembla-cuda --features cuda --locked`, workspace and CUDA-feature Clippy with `-D warnings`, `bash scripts/check.sh`, and `git diff --check`. Generate CUDA for SIR and every new adversarial fixture and run Clang CUDA-stub syntax checks; run actual NVRTC/GPU tests through `crates/sembla-cuda/scripts/run-gpu-tests.sh` only on a clean CUDA host. If no GPU is reachable, leave criteria 2–4 explicitly unanswered in `GPU-STATUS.md`.

## Files to Modify
- `crates/sembla-cuda/src/codegen.rs` - safe model labeling, sequential reduction emission, ordered error planes/claim validation, and codegen regressions.
- `crates/sembla-cuda/src/backend.rs` - one-part reduction launches, candidate-error storage, and per-transition device-side validation order.
- `crates/sembla-cuda/tests/gpu_semantics.rs` - cancellation, checked-overflow, and first-error differential fixtures.
- `crates/sembla-cuda/tests/fixtures/sir.generated.cu` - regenerated deterministic golden.
- `crates/sembla-cuda/README.md` - restored CPU-oracle reduction contract and CUDA implementation description.
- `crates/sembla-runtime/src/eval.rs` - targeted reversion to sequential grouped/input reductions and documentation.
- `crates/sembla-runtime/src/executor.rs` - targeted reversion to sequential wire-output reduction.
- `crates/sembla-runtime/tests/eval.rs` - restore the sequential cancellation regression.

## New Files
- None. Add regressions to the existing codegen and GPU semantic test modules.

## Dependencies
- Task 1 defines the failing behaviors and should precede implementation changes.
- Task 3 must establish the authoritative CPU results before Task 4 rewrites CUDA reductions.
- Task 2 is independent but changes the same generated golden as Tasks 4–5, so regenerate once in Task 7.
- Task 5 depends on final candidate-buffer and kernel ABI decisions in `codegen.rs` and `backend.rs`.
- Task 6 depends on Task 5’s ordered validation machinery and may require extending it with ordered aggregate first-use metadata.
- Task 7 follows all codegen/runtime changes; Task 8 is last.

## Risks
- The PRD’s phrase “fixed-shape two-pass trees” conflicts with the already-frozen sequential CPU oracle and the explicit instruction not to change CPU semantics. For this revision, oracle equality and acceptance criterion 1 should win: use a deterministic serial fold plus a non-arithmetic publication stage. If maintainers instead want a new two-half numeric contract, that requires a separate semantic-change PRD/ADR and rebaselining, not a PRD-0008 patch.
- Do not use `result + 0.0` as a nominal identity merge; signed-zero and NaN payload behavior can change. Publish/copy bits directly.
- Serial CUDA folding may reduce throughput, especially for 100k-row/200-tick tests, but performance is explicitly a non-goal. Optimize later only with an algorithm proven bitwise identical to the sequential oracle.
- Recomputing pure filter/value expressions avoids new row-staging buffers, but codegen must preserve eager CPU error order: all filters first, then all Sum values, then reduction. Add counterexamples with errors on different rows to verify this.
- Fixing only “all transitions then all claims” is insufficient if candidate errors still collapse guard and hazard into one per-row slot or if claim loops remain row-major. The two-plane and claim-major requirements are part of the safe fix.
- Flat eager scheduling-aggregate error checks are a likely residual first-error-order hazard. Task 6 must be resolved or explicitly proven safe before seeking approval.
- Kernel signature changes are not type-checked against NVRTC function loading; generated-source syntax checks and real NVRTC compilation are essential.
- Keep `.piprd/*` artifacts intact and do not claim criteria 2–4 without real GPU evidence.
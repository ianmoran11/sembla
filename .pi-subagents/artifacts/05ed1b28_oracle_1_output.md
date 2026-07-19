Inherited decisions:
- The CPU runtime is the semantics oracle; PRD 0005 freezes aggregate Sum as one ascending-row sequential fold, and DESIGN §5.2 says the CPU `f64` numeric contract is unchanged.
- PRD 0008 requires bitwise CUDA/CPU equality, no result-bearing atomics, no silent fallback, and device-resident decisions. Performance optimization is explicitly a non-goal.
- CUDA-only tests remain ignored and criteria 2–4 remain honestly unanswered without hardware. Criteria 5–6 already pass and should not be disturbed.
- The current revision intentionally stages schedule/effect/output aggregates by semantic liveness; preserve that phase/state work while correcting reduction order and error precedence.
- Do not narrow the IR merely to make code generation easier, and do not commit.

Diagnosis:
- Blocker 1 is an output-encoding bug, not an IR-validation bug. `codegen.rs` emits `ValidatedModel.model().name` verbatim at the end of a `//` line. Newlines can inject directives; a name ending in backslash can also trigger C/C++ backslash-newline splicing if a naive escaping fix leaves `\\` immediately before the newline.
- Blocker 2 is real semantic drift. The base CPU implementation and PRD 0005 use one left fold. The current diff changes grouped, input-table, and output reductions to two half-folds and a merge. Non-associative `f64` and order-sensitive checked `i64` make this observable. `[1e16, 1, -1e16, 1]` is 1.0 sequentially but 0.0 under the current halves; `[i64::MAX, 1, -1]` errors sequentially but can succeed when split.
- There is a literal wording tension: PRD 0008 says “fixed-shape two-pass trees,” while the same PRD says “same fixed order as the CPU oracle,” PRD 0005 freezes a sequential pass, DESIGN says the numeric contract is unchanged, and acceptance criterion 1 says unchanged. These cannot all describe two independent half sums. The semantic/oracle requirements must dominate the mechanism parenthetical unless a separate PRD/ADR explicitly revises CPU semantics.
- A CUDA pass-1 sequential fold followed by pass-2 bitwise publication is the smallest deterministic two-stage implementation. It preserves the existing oracle and avoids atomics/reassociation. It is not honestly a parallel reduction tree; documentation should call it a fixed two-stage ordered reduction, not claim two independent half-tree arithmetic. If a reviewer insists on the literal word “tree,” the PRD needs clarification rather than another CPU semantic change.
- Blocker 3 is broader than the reported cross-transition example. Current CUDA batching also differs from CPU in these ways:
  - all schedule aggregates are checked before any transition, so a later transition's aggregate error can preempt an earlier scalar guard error;
  - each transition kernel fuses guard and hazard per row into one error byte, while CPU evaluates the entire guard column before the entire hazard column;
  - claim validation loops rows outside claims, while CPU evaluates claim 0's whole resource/key columns before claim 1;
  - grouped/output aggregate kernels interleave filter and value per row, while CPU evaluates the complete filter column, then complete value column, then folds;
  - effect preparation itself is already serial in transition/effect/row order, but effect aggregates are prechecked before scalar effect expressions, creating the analogous aggregate/scalar precedence risk.
  A narrow “check claims after each transition” patch fixes the cited counterexample but is likely to fail the next adversarial review.

Drift / contradiction check:
- Changing `sembla-ir` validation to reject newlines would conflict with “any validated model” and alter IR semantics outside PRD 0008. Encode or omit the debug name in CUDA instead.
- Keeping the CPU two-half changes and updating PRD 0005/DESIGN would revise an inherited semantic decision solely to accommodate CUDA. That is the wrong direction and violates the unchanged-oracle contract.
- A publish stage must copy bits, not add an identity partial. An extra `+ 0.0` is unnecessary floating-point work and can be problematic for signed zero/NaN payload behavior; checked Int publication must likewise not introduce a new merge-overflow point.
- First-write-wins device status is incompatible with batched evaluation when launch order differs from CPU semantic order. Either execute checks in semantic order or record error facts and select the minimum semantic rank later; do not use atomics to race for status.

Recommendation:
1. Fix source injection first, independently.
   - In `crates/sembla-cuda/src/codegen.rs`, remove the model-name comment or replace it with a fixed-width SHA-256 hex digest. Removing it is smallest and safest.
   - Do not add name-character restrictions in `crates/sembla-ir/src/validate.rs`.
   - Add an always-running codegen regression for a validated name containing newline/`#error`, carriage return, non-ASCII, and a trailing backslash. Assert generated source contains no raw payload/directive and remains deterministic. Regenerate the SIR golden.

2. Restore the CPU oracle exactly, with targeted hunk reverts rather than whole-file checkout.
   - Restore sequential input Sum and grouped Count/Sum in `crates/sembla-runtime/src/eval.rs`.
   - Restore sequential wire-output Sum in `crates/sembla-runtime/src/executor.rs`.
   - Restore the original row-order regression in `crates/sembla-runtime/tests/eval.rs` (1.0 for the cancellation vector) and add the checked-Int order counterexample.
   - Update `crates/sembla-cuda/README.md` and the runtime module comment so they no longer claim the CPU oracle uses two halves.
   - Preserve unrelated runtime changes, especially input ordered-comparison `f64` semantics and prospective output behavior.

3. Make CUDA reductions correctness-first and serial in arithmetic order.
   - In `crates/sembla-cuda/src/codegen.rs`, change input helpers, grouped aggregate partials, and wire-output partials to one ascending-row fold.
   - Smallest layout-preserving option: only partial 0 performs the full fold; launch one partial worker; the finish kernel publishes partial 0 by typed/bitwise copy and performs no `+ partial1` operation. Existing two-part storage can remain temporarily to minimize layout churn, but unused partial 1 must not participate.
   - In `crates/sembla-cuda/src/backend.rs`, launch one reduction worker rather than two where applicable. Keep the second kernel stage as deterministic publication and preserve current state/effect/output phase selection.
   - This is acceptable as the next correctness revision because performance is a non-goal. A future parallel tree requires a separately approved CPU-oracle change or an exact-summation design, not reassociation by fiat.

4. Replace first-write schedule error handling with a bounded semantic-order selector; do not merely reorder one host call.
   - Keep computation kernels parallel and device-resident, but record error facts in unique slots without committing final `status` immediately.
   - At minimum record separate guard and hazard error planes per candidate; record claim errors by transition/claim/row; retain one persistent error record per aggregate rather than clearing/reusing it before semantic selection.
   - Generate stable evaluation-order metadata in `GeneratedCuda`: transition declaration order; guard then hazard; claim resource then key in claim declaration order; aggregate first-use location/order. A single-thread device selector scans those facts in the CPU order and writes the final status. Because it is serial and runs on the stream, no result-bearing atomic is needed.
   - Separate static incompatible-order validation from expression-error selection: compatibility belongs after all transition expression evaluation, matching CPU `resolve_claims`.
   - For a smaller first implementation, per-transition serial validation launches are acceptable, but only if they also preserve guard-column-before-hazard-column and claim-before-row nesting and prevent later aggregate checks from preempting earlier transition errors. The semantic-rank/error-fact approach is more review-resistant than many special-case launches.
   - Audit effect/output phases under the same rule. Current effect scalar loop order is close to CPU; ensure active effect aggregate errors are assigned to their first effect use, and output errors follow wire/field/filter/value/fold order.

5. Add counterexamples before or alongside implementation.
   - Source: model name `ok\n#error injected_model_name` and a trailing-backslash name.
   - Reduction: grouped real `[1e16, 1, -1e16, 1]` => bitwise 1.0; input-table and wired-output variants of the same; checked Int `[MAX, 1, -1]` must fail at the same stage/row as CPU.
   - Cross-transition: transition 0 claim-key overflow plus transition 1 guard overflow; transition 0 claim must win precedence.
   - Intra-transition: guard overflow on a later row plus hazard overflow on an earlier row; guard column must win.
   - Multiple claims: claim 0 overflow on a later row plus claim 1 overflow on an earlier row; claim 0 must win.
   - Aggregate batching: earlier transition scalar guard overflow plus later transition aggregate overflow; earlier guard must win.
   - Aggregate internals: filter overflow on a later row plus value overflow on an earlier row; CPU filter-column precedence must win.
   Always-running tests should validate codegen metadata/source and CPU oracle expectations; real CPU/CUDA differential/error tests remain `#[ignore]`.

6. Verification order:
   - `cargo fmt --all -- --check`
   - `cargo build --workspace --locked`
   - `cargo test --workspace --locked`
   - `cargo check -p sembla-cuda --features cuda --all-targets --locked`
   - `cargo test -p sembla-cuda --features cuda --locked` (GPU cases remain ignored)
   - workspace and CUDA-feature clippy with `-D warnings`
   - `bash scripts/check.sh`; `git diff --check`
   - regenerate `crates/sembla-cuda/tests/fixtures/sir.generated.cu`
   - Clang/CUDA-stub syntax checks for all new generated fixtures and, when hardware is available, actual NVRTC plus ignored GPU tests. Otherwise criteria 2–4 remain unanswered.

Risks:
- The fixed-tree wording remains ambiguous. The recommended serial-fold/publish path honors every observable inherited contract but is not a parallel tree. If literal parallel-tree compliance is mandatory, the main agent must request an explicit PRD/ADR decision rather than silently changing the oracle again.
- Exact fail-fast parity is an architectural surface, not just one kernel order. Missing aggregate first-use or filter/value ordering can produce another blocker even after the cited claim example passes.
- Serial reductions may be slow, especially with many aggregates, but correctness and bounded scope outrank performance in this PRD. Do not optimize until GPU criteria pass.
- Retaining unused second-part storage is safe but can confuse future reviewers; comment it as compatibility/layout staging or simplify after correctness is established.
- Model-name hashing/removal changes the generated golden and source SHA. That is expected; ensure dump determinism tests are updated.

Need from main agent:
- No product decision is required to proceed with the correctness-first interpretation: restore the established CPU oracle and make CUDA conform.
- Only if the project intends “fixed-shape two-pass tree” to override the frozen sequential CPU contract is an explicit owner/PRD decision required. That would be a semantic pivot, not a PRD 0008 bug fix.

Suggested execution prompt:
- “Revise PRD 0008 only. Do not change IR validation or CPU semantics. Remove/hash the untrusted model-name CUDA comment and add newline/trailing-backslash regressions. Revert only the CPU two-half reduction hunks and restore sequential grouped/input/output folds. Change CUDA input/grouped/output reductions to one ascending-row arithmetic fold followed by a no-arithmetic bitwise publish stage; preserve phase-liveness metadata and no atomics. Replace first-write schedule error status with persistent error facts and a single-thread semantic-order selector matching CPU transition order (guard column, hazard column, claims claim-major/row-minor, aggregate first use), and audit effects/outputs similarly. Add the listed cancellation, checked-overflow, cross-transition, intra-transition, multi-claim, aggregate-batching, and filter/value regressions. Regenerate the golden and run the full default/CUDA-feature/clippy/script/Clang verification suite. Do not commit; leave GPU criteria unanswered without hardware.”
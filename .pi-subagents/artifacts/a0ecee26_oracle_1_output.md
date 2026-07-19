Inherited decisions:
- CPU runtime semantics are authoritative; PRD 0008 must not change the CPU/IR contract merely to accommodate CUDA.
- CUDA must execute every validated v0.1 model without silent fallback, result-bearing atomics, or host-side semantic decisions.
- GPU-only criteria remain honestly unanswered when no device is available.
- The current revision fixed raw model-name source injection, restored sequential CPU reductions, made CUDA arithmetic folds sequential, and fixed the previously cited earlier-claim/later-guard case.

Diagnosis:
- The cited fixes are present, but the backend still commits errors in CUDA implementation-phase order rather than the CPU evaluator's semantic order. This is observable for valid models that contain more than one failing expression.
- Static host/generated-kernel ABI inspection found no new mismatch: `sembla_check_candidate_errors` has four arguments on both sides; per-rule claim validation and claim-compatibility argument lists align; feature-enabled Rust checks compile. This does not substitute for NVRTC/device execution.

Blocking findings:
1. Same-transition aggregate errors still preempt earlier scalar guard errors.
   - CUDA builds and checks every aggregate first used by a rule before launching that rule's transition kernel (`crates/sembla-cuda/src/backend.rs:419-465` versus transition evaluation at `:466-533`).
   - CPU evaluates a guard expression recursively left-to-right (`crates/sembla-runtime/src/eval.rs:856-859`) when `stage_box` evaluates the guard (`crates/sembla-runtime/src/executor.rs:649-664`).
   - Concrete validated counterexample: one transition over two rows with `x=[i64::MAX,1]`, and guard `(x * 2 > 0) And (Agg Sum(x) >= 0)`. The left scalar multiplication is the CPU's first error; CUDA first builds the aggregate, whose sequential `MAX + 1` overflows, and reports aggregate failure before launching the guard. `/tmp/same_rule_order.json` was accepted by `sembla validate`.

2. Effect aggregate batching still preempts earlier effect-expression errors.
   - CPU evaluates winning transitions and effects in transition/effect declaration order (`crates/sembla-runtime/src/executor.rs:718-757`).
   - CUDA marks/builds/checks all live effect-only aggregates before `sembla_prepare_effects` evaluates any ordinary effect expression (`crates/sembla-cuda/src/backend.rs:579-635` before `:639-658`; generator effect loop at `crates/sembla-cuda/src/codegen.rs:1233-1269`).
   - Concrete valid model: an always-winning transition has effect 0 `y = x * 2` and effect 1 `z = Agg Sum(x)`, with `x=[MAX,1]`. CPU fails effect 0; CUDA reports the effect-aggregate overflow first.

3. Global transition batching violates CPU box order for effect-versus-later-box errors.
   - CPU completes `stage_box` for box 0, including its winning effects, before beginning box 1 (`crates/sembla-runtime/src/executor.rs:306-309`, with effects inside `stage_box` at `:718-757`).
   - CUDA evaluates transitions for every box first (`crates/sembla-cuda/src/backend.rs:419-534`), then globally resolves and evaluates effects (`:535-658`).
   - Concrete valid model: box 0 has an always-winning transition whose effect computes `MAX * 2`; box 1 has a guard computing `MAX * 2`. CPU reports box 0's effect error; CUDA reports box 1's candidate/guard error and suppresses effects.

4. Wired-output aggregate batching violates output field order.
   - CPU visits wires and fields in declaration order, evaluating each field filter/value before the next field (`crates/sembla-runtime/src/executor.rs:812-831,835-889`).
   - CUDA builds/checks all aggregates reachable from every output before launching any output field reduction (`crates/sembla-cuda/src/backend.rs:677-723` before `:724-765`).
   - Concrete valid output: field 0 is `Sum(x * 2)` and field 1 is `Sum(Agg Sum(x))`, with `x=[MAX,1]`. CPU fails field 0's scalar value; CUDA reports field 1's nested aggregate first.

Drift / contradiction check:
- The implementation no longer changes CPU reduction behavior; that earlier contradiction is resolved.
- Raw model names are represented by an ASCII digest, resolving source injection.
- The remaining drift is architectural: `status` is first-writer-wins in phase order, while the CPU oracle is fail-fast in box/transition/expression/effect/wire order. Per-rule aggregate first-use metadata fixes only cross-rule scheduling, not first use inside an expression or later phases.

Acceptance criteria:
1. PASS locally: `cargo build --workspace --locked` and `cargo test --workspace --locked` both succeeded; default codegen/golden tests run.
2. UNANSWERED as permitted: ignored device Philox test exists (`gpu_philox.rs:27-57`); no GPU/toolkit was reachable.
3. UNANSWERED as permitted: ignored 100k SIR, `sir_policy`, and canonical 200-tick tests exist (`gpu_oracle.rs:47-89`).
4. UNANSWERED as permitted: ignored repeatability test exists (`gpu_oracle.rs:92-98`).
5. PASS: the nonignored no-device test freezes `cuda backend unavailable: no CUDA device found` and feature-off construction cannot substitute CPU (`tests/absence.rs:3-32`).
6. PASS: `GPU-STATUS.md:3-16` explicitly records 2-4 as unanswered and points to the remote evidence runbook.

Recommendation:
- Assessment: REVISE.
- Replace first-writer phase status with semantic-order error facts/ranks, or execute a device-resident validation schedule that exactly follows box -> transition -> guard -> hazard -> claims -> active effects -> wire/field order, including aggregate first-use positions inside expressions. Do not patch only the four examples; they share one scheduling root cause.
- Add always-running model-validation/codegen regressions and ignored GPU differentials for each counterexample above.

Risks:
- GPU criteria remain legitimately unanswered, so successful NVRTC loading and ABI execution are still unverified on hardware.
- A narrow fix for same-rule guards will leave effects, boxes, and outputs divergent.

Need from main agent:
- No product decision is required. Additional implementation is required before approval.

Suggested execution prompt:
- No worker handoff proposed by this oracle; the parent owns implementation orchestration.
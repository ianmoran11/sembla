Inherited decisions:
- PRD 0008 requires the native-f64 CUDA path to execute any validated v0.1 model, with no CPU fallback, deterministic generated CUDA, fixed two-part CPU/CUDA reductions, device-resident state, and honest unanswered reporting when no GPU is reachable.
- Default workspace builds must remain CUDA-toolkit-free. GPU tests stay ignored. Criteria 2-4 cannot be claimed without hardware.
- The current shared two-part reduction contract, dense owner-cell scatter, serial claim compatibility validation, phased aggregate scheduling, and input-row f64 ordering specialization are intentional inherited decisions and should be preserved.

Diagnosis:
- Assessment: REVISE.
- Concrete blocker: generated CUDA source is injectable through the validated model name. `crates/sembla-cuda/src/codegen.rs:783` emits `self.model.model().name` verbatim into a `// model: ...` comment. `crates/sembla-ir/src/validate.rs:120-143` validates `dt` and uniqueness of parameters/boxes/summaries but imposes no lexical/control-character constraint on `model.name`.
- Reproduction performed read-only using `examples/two_state.json` with the name changed to `ok\n#error injected_model_name`: `cargo run -q -p sembla-cli -- validate /tmp/prd0008-injected-name.json` exited 0, while `sembla_cuda::generate()` began with:
  `// model: ok`
  `#error injected_model_name`
  This necessarily makes NVRTC compilation fail in `CudaBackend::new`, so a valid model in the declared fragment cannot execute. It directly contradicts the PRD goal of executing any validated model and the deterministic model-load compilation contract.
- No other blocking semantic race/order issue was established. Result-bearing paths are atomic-free and uniquely written; aggregate phases use start state for scheduling/winning effects and prospective state for wired outputs; input-row ordered numeric comparisons use f64 conversion.

Acceptance criteria:
1. PASS locally: `cargo build --workspace --locked` and `cargo test --workspace --locked` succeeded. Non-GPU codegen/golden tests run in the default suite.
2. UNANSWERED as required: ignored `device_philox_is_bit_identical_to_checked_cpu_vectors` exists and compares device output with shared CPU vectors; no GPU result is claimed.
3. UNANSWERED as required: ignored 100k/200-tick SIR, two-box `sir_policy`, and canonical `two_state` per-tick oracle tests exist; no GPU result is claimed.
4. UNANSWERED as required: ignored same-run-twice hash test exists; no GPU result is claimed.
5. PASS locally: nonignored frozen no-device diagnostic tests exist, plus feature-enabled production `CUDA_ERROR_NO_DEVICE` classification coverage. The CUDA-feature test suite passed.
6. PASS: `GPU-STATUS.md`, README, and `scripts/run-gpu-tests.sh` explicitly retain criteria 2-4 as unanswered and reference the Hyperstack runbook/provenance workflow.
- Additional verification: `cargo test -p sembla-cuda --features cuda --locked` passed with 13 GPU tests ignored; `git diff --check` passed.

Drift / contradiction check:
- The implementation trajectory otherwise preserves the inherited decisions. The blocker is not a request to narrow the IR or alter CPU semantics; doing so would conflict with “any validated model.” The generated diagnostic comment quietly assumes a single-line safe name, an assumption not present in validation or the PRD.

Recommendation:
- Sanitize/escape the model name before embedding it in generated CUDA, or omit the name comment entirely. Preserve deterministic bytes for ordinary names if desired.
- Add an always-running regression that validates a model whose name contains newline/directive text, generates CUDA, and proves the payload remains inside a harmless single-line representation and cannot create a preprocessing directive. Regenerate the SIR golden only if ordinary-name output changes.
- After this narrow fix, rerun default workspace tests, CUDA-feature tests/clippy, generated-CUDA syntax checks, and `git diff --check`. Criteria 2-4 should remain unanswered until real hardware runs.

Risks:
- Actual NVRTC compilation and all device semantic differentials remain unverified on hardware, but this is explicitly permitted as unanswered by criterion 6 rather than an implementation blocker.
- Exact multi-failure diagnostic precedence has less coverage than state/hash parity, though no current state-bearing divergence was demonstrated.

Need from main agent:
- No product decision is needed; the PRD already resolves the scope in favor of supporting every validated model.

Suggested execution prompt:
- “In `crates/sembla-cuda/src/codegen.rs`, make the generated model-name comment safe for arbitrary validated strings (especially CR/LF and preprocessor payloads) without changing model semantics. Add an always-running regression using a validated model named `ok\n#error injected` that asserts the generated source has no injected directive; preserve deterministic output and update the golden only if required. Run fmt, workspace/default and CUDA-feature tests/clippy, generated CUDA syntax checks, and diff-check. Do not commit or modify managed `.piprd` artifacts.”
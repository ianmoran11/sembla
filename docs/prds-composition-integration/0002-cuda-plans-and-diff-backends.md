# PRD 0002: Plan envelopes on the CUDA backend; `diff-backends` over plans

## Context

Read `docs/prds-composition-integration/README.md` first; its constraints
bind — especially decision 1 (CUDA keying split) and decision 2 (local vs
hardware acceptance).

State of the code this PRD changes:

- `crates/sembla-cli/src/main.rs:486` rejects plan envelopes on CUDA:
  `"plan envelopes run on the cpu backend only for now"`. `run` already
  executes plans on CPU via `ValidatedPlan::to_validated_model()`
  (`crates/sembla-ir/src/plan.rs:131`), which stamps per-transition
  `rule_word` values onto the `ValidatedModel` (`validate.rs:60-99`).
- The CPU runtime already keys Philox and tie-breaks by
  `validated.rule_word` (`crates/sembla-runtime/src/executor.rs:698,719,
  975-982,1081`) and uses the dense id only for indexing/diagnostics.
- The CUDA backend (`crates/sembla-cuda/src/backend.rs`, `codegen.rs`) still
  uses `transition.rule_id` everywhere — correct for legacy models only,
  because legacy sets `rule_word == rule_id` (`validate.rs:129`). The
  `ValidatedModel` the backend receives **already carries `rule_word`**; no
  new plumbing from `sembla-ir` is needed, only correct keying inside the
  CUDA crate.
- `diff-backends` is legacy-only (`main.rs:2699` uses `read_model`), and
  `--all-examples` walks `examples/` (`main.rs:2634`).

## Goal

Plan envelopes execute on CUDA with the rule-word/ordinal split mirroring
the CPU runtime; legacy CUDA behavior is provably byte-identical;
`diff-backends` accepts plan files and a new `--all-plan-fixtures` flag; the
differential-corpus runbook covers the composition fixtures; all local
criteria pass without a GPU.

## Specification

### 1. Audit and split rule identity in `sembla-cuda`

Repeat, for the CUDA crate, exactly the audit the composition track's
PRD 0004 performed for the CPU runtime. Enumerate every use of
`transition.rule_id` / baked rule ids in `backend.rs` and `codegen.rs`
(start from `grep -n rule_id crates/sembla-cuda/src/*.rs`) and classify
each as:

- **Philox key material** — anywhere a rule identity becomes counter word 1
  of a Philox invocation (device-side `sembla_philox` counters for race
  times and effect draws; host-side helpers such as `philox_vectors`,
  `backend.rs:439-483`, if rule-keyed). These switch to `rule_word`.
- **Ordering/tie-break keys** — device or host comparisons mirroring the
  CPU's `(time, rule_word, entity_id)` lexicographic winner selection
  (see `backend.rs:713` area and any codegen'd comparator). These switch to
  `rule_word`.
- **Indexing/layout/diagnostics** — candidate-buffer offsets, fired-count
  slots, `AggUse::Schedule/Effect(rule_id)` codegen specialization
  (`codegen.rs:146-155`), kernel-selection arguments, and error messages
  (`backend.rs:659,713` row-count errors). These keep the dense `rule_id`.

Where a kernel currently receives one `rule_id` argument that serves both
purposes, pass both values (add a `rule_word` argument or bake it as a
codegen constant alongside the id — prefer whichever the surrounding code
already does for per-rule constants). Record the complete classified list in
the implementation notes; review checks it against the grep.

Two invariants to preserve exactly:

1. **Legacy bit-identity.** For legacy models `rule_word == rule_id`, so
   every existing CUDA golden, the differential corpus expectations, and
   `docs/cuda-differential-harness.md` evidence remain valid unchanged. Any
   needed change to a checked-in CUDA expectation means the split leaked;
   fix the code.
2. **CPU/CUDA agreement is the contract.** The CPU oracle is ground truth
   (DESIGN.md §5.2). The CUDA path must consume the same
   `ValidatedModel.rule_word` values — never recompute words from
   identities inside the CUDA crate.

### 2. Lift the CLI restriction

In `main.rs`, remove the `plan envelopes run on the cpu backend only for
now` rejection: `run <plan.json> --backend cuda` executes through the same
`ValidatedPlan::to_validated_model()` path as CPU. Keep the `--dt` override
rejection for plans (README decision 5) exactly as is. Update `USAGE` and
any help text.

### 3. `diff-backends` over plans

- Accept plan envelopes: route through `parse_input`; for plans, apply
  validation + canonicality (same as `run`), reject `--dt` with the same
  message `run` uses, and execute both backends from the derived
  words-carrying model. Legacy paths byte-unchanged.
- Add `--all-plan-fixtures`: walks `fixtures/plans/*.plan.json` and
  `fixtures/plans/linked/*.plan.json` (sorted paths), running the same
  CPU-vs-CUDA comparison per file. It composes with the existing flags
  (`--population`, `--seed`, `--ticks`) the way `--all-examples` does, and
  is mutually exclusive with a positional model path and with
  `--all-examples` (deterministic usage error if combined). Note: fixture
  plans embed small populations (Person 1000 scale) — `--population` for
  plan fixtures must follow whatever convention `run` already uses for plan
  fixtures in tests (mirror the existing plan-run golden tests' invocation
  exactly; do not invent a new population story).
- The comparison contract (which fields must match bitwise vs by tolerance
  at the configured determinism level) is **unchanged** — reuse
  `execute_backend_output` and the existing field comparison
  (`compare_field`, `main.rs:2422`) as-is.

### 4. Differential corpus extension

- Extend `crates/sembla-cuda/scripts/run-differential-corpus.sh` (and its
  corpus listing, wherever the current model list lives) with the
  composition plan fixtures: at minimum `two_box.plan.json`,
  `linked/epidemic_policy.plan.json`, `linked/two_regions.plan.json`,
  `linked/regional_response.plan.json`, `linked/wrapped_ping_pong.plan.json`,
  invoked via `diff-backends --all-plan-fixtures` or per-file, matching the
  script's existing style.
- Extend `crates/sembla-cli/tests/gpu_differential.rs` with the plan
  fixtures using the **same GPU-availability gating convention the file
  already uses** (tests skip cleanly, never fail, without CUDA — read the
  file first and copy its pattern).
- Update `docs/cuda-differential-harness.md`: add the composition corpus
  section, state the rule-word keying decision (pointer to DECISIONS §J14),
  and state explicitly that existing legacy evidence remains valid because
  legacy words equal dense ids.

### 5. Local vs hardware acceptance (README decision 2)

Local (must pass in this managed run, no GPU): everything in
`./scripts/check.sh` including `cargo build -p sembla-cuda` and the crate's
host-side/codegen tests; codegen snapshot/unit tests updated for the new
`rule_word` parameter (if codegen has string-snapshot tests, update them —
these are *not* frozen goldens; say so in the notes); GPU-gated tests skip
cleanly; `diff-backends --all-plan-fixtures` fails gracefully with the
existing no-CUDA diagnostic.

Hardware (listed as pending in the implementation notes, run later via the
runbook): the full corpus including plan fixtures passes CPU-vs-CUDA on
qualified hardware; legacy corpus results unchanged. Do not fabricate or
simulate these results.

## Allowed files

- `crates/sembla-cuda/src/**`, `crates/sembla-cuda/scripts/**`,
  `crates/sembla-cuda/tests/**` (if present)
- `crates/sembla-cli/src/main.rs`, `crates/sembla-cli/tests/**`
- `crates/sembla-ir/src/plan.rs` (only if a small accessor is missing;
  no schema or validation change)
- `docs/cuda-differential-harness.md`
- `Cargo.lock` (should be unchanged; no new dependencies)
- implementation notes/artifacts created by the managed run

## Non-goals

- Any CUDA algorithm/precision/reduction change; ADR 0001's contract is
  untouched.
- Sweep/compare plan support (PRDs 0003–0004).
- Plan-schema or identity changes; mailbox identities do not enter CUDA.
- Running or claiming GPU evidence in hosted CI (docs/ci.md discipline).

## Acceptance criteria

1. `./scripts/check.sh` passes; **zero** diffs to `examples/**`, CSV/hash
   goldens, plan fixtures, or recorded differential evidence.
2. Implementation notes contain the complete classified rule-identity list
   for `sembla-cuda` (word vs ordinal per use site), checkable against
   `grep -n rule_id`.
3. `run <plan> --backend cuda` is accepted (code path exists; on a no-GPU
   machine it fails with the existing CUDA-unavailable diagnostic, not the
   removed "cpu backend only" message). `--dt` with a plan still rejects.
4. `diff-backends` accepts a plan file; `--all-plan-fixtures` enumerates
   exactly the sorted plan fixtures; combining it with a path or
   `--all-examples` is a usage error — each covered by a CLI test.
5. `gpu_differential.rs` covers the plan fixtures with clean skips sans
   GPU; the corpus runbook and harness doc list the composition corpus and
   the §J14 keying decision.
6. Implementation notes list the pending hardware criteria explicitly.
7. `git diff --check` passes; no new dependencies.

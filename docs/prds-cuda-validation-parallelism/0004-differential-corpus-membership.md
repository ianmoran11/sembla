# PRD 0004: Admit the demographic model to the CUDA differential corpus

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind.

The model that exposed this defect is not in the differential corpus. Until it
is, nothing prevents the regression returning: the corpus is what turns
"CPU and CUDA agree" from a claim into a checked property, and every model in it
today is one that generates no per-row validation loops.

## Goal

`demographic_slots` in its no-grouped configuration is a first-class member of
the CUDA differential corpus, and the existing corpus is provably unchanged.

## Specification

### 1. Add the model to the corpus

Admit the no-grouped configuration only (§L5 — grouped observations stay
CPU-only and are rejected deterministically). Use a reduced scale that runs in
CI-appropriate time; the frozen 10M case in §L is for the performance gate, not
for the corpus.

### 2. Assert the existing corpus is untouched

The tracked CUDA differential evidence and every legacy golden must be
byte-identical. The corpus gains a member; nothing already in it changes.

### 3. Document the coverage gap this closes

Record in `docs/cuda-differential-harness.md` that the corpus previously
contained no model exercising contests or `Ref` dereferences, which is why a
12.3× regression on that path went undetected while every differential test
passed. Correctness testing and usability testing are different things, and the
corpus only ever asserted the first.

### 4. Keep `--all-plan-fixtures` semantics intact

Per §J14.6, `diff-backends --all-examples` keeps its exact current behaviour.
If the model enters through a plan fixture, it enters via `--all-plan-fixtures`.

## Allowed files

- `crates/sembla-cuda/tests/**`
- `crates/sembla-cuda/scripts/run-differential-corpus.sh`
- `crates/sembla-cli/tests/gpu_differential.rs` — grouped-rejection and supported-plan corpus tests only
- `crates/sembla-cli/src/main.rs` — only if corpus enumeration needs it
- `fixtures/plans/**` — new fixtures only
- `docs/cuda-differential-harness.md`
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No semantic change, no grouped-observation support, no performance work, no
change to `--all-examples` behaviour, no edits to existing fixtures or evidence.

## Acceptance criteria

**Local:**

1. Corpus listing includes the demographic no-grouped model; GPU-less runs skip
   gracefully with a named reason.
2. `git diff --stat` shows no change to `examples/**`, existing goldens, existing
   plan fixtures, or tracked CUDA differential evidence.
3. A test asserts the grouped configuration is still rejected deterministically
   on CUDA with a diagnostic naming the flag.
4. `cargo test --locked`, `scripts/check-rust.sh`, and
   `python3 scripts/check-markdown-links.py` green.

**Hardware (pending per §J14.2):**

5. The demographic no-grouped model passes CPU/CUDA differential equality at
   the corpus scale under the declared numeric contract.

## Revision note

Written before PRD 0002 lands. If 0002 changes how models are admitted to the
corpus, revise this PRD explicitly rather than reinterpreting it.

Attempt-1 review amendment: `crates/sembla-cli/tests/gpu_differential.rs` is
narrowly allowed so the existing stale all-plan success test can be split into
an executable GPU-less grouped-rejection contract and an ignored hardware test
for every CUDA-supported plan fixture. CLI enumeration and production behavior
remain unchanged.

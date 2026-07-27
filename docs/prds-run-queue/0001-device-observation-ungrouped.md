# PRD 0001: Observe on the device, and stop downloading the state

## Context

Read `docs/prds-device-observation/README.md` first; its constraints bind,
including the eligibility rules and the §J14.2 local/hardware split.

The CUDA tick loop downloads the whole device state every tick and rebuilds a
host `StateStore` so `executor::observe_views` can read it. Measured at 5M rows
over 2 ticks, that machinery — `state_transfer` 239.6 ms, `state_reconstruct`
220.7 ms, `readback_control` 206.3 ms — is **71% of CUDA wall time**, against
9.3 ms of kernels.

Every reduction in the benchmark model is `count` or `max` over `int`, so a
device-side reduction produces identical values by construction. See the README
for the general cases that do not.

## Goal

Where every view in a model is device-eligible, observation happens on the
device and the per-tick state download does not occur. Results are unchanged,
bit for bit.

## Specification

### 1. An eligibility predicate over the IR

Decide from the IR whether a view can be observed on the device. Eligible:

- `count`, with a row-local filter;
- `min` / `max` over `Int`;
- a row-local value expression, by the same predicate PRD 0006 of
  `prds-evaluator-throughput` established — reuse it rather than writing a
  second one.

Ineligible, and these must fall back:

- `Sum` over `Real` — ascending row order is canonical (`eval.rs` module doc);
- `Sum` over `Int` — a different association order can overflow where the
  sequential pass does not;
- `min` / `max` over `Real` — argue NaN asymmetry explicitly or exclude;
- anything containing `Expr::Agg` or `Expr::Input`;
- grouped views — a later PRD.

**Conservative by default**: anything the predicate does not positively
recognise is ineligible.

### 2. Model-level eligibility gates the download

Skipping the per-tick download is only permissible when **every** view in the
model is eligible. One host-bound view means the state must come back anyway,
and the win is gone.

Compute this once per run, record it, and make it visible — a run that expected
the fast path and silently took the slow one would look like the optimisation
failed.

### 3. Emit device reductions inside the closed fragment

`DESIGN.md` §4.2 defines the closed kernel fragment as map, filter,
join-on-keys, commutative-monoid group-by, segmented argmin and Philox. Counts
and integer min/max are commutative monoids and sit inside it. Do not extend the
fragment; if a view needs something outside it, that view is ineligible.

Return only the reduced values — one scalar per view per tick — not per-row
data. Returning arrays would reintroduce the transfer this PRD exists to remove.

### 4. What still has to come back

Skipping the state download must not break anything else that reads state:

- **Per-tick hashes** in `HashMode::EveryTick`. `download_hash` works from the
  raw device bytes rather than the `StateStore`, so it can stay — but it still
  transfers. State whether the differential path therefore keeps a transfer, and
  measure it separately, because it changes what a differential run costs.
- **The final state export**, once at the end, not per tick.
- **`readback_control`** — the `wins` and `deferred` arrays. These are *not*
  observation; they feed the tick report's fired counts. Reducing them on device
  is a real and separate opportunity worth 22%, and is **out of scope here**.
  Say in the notes whether this PRD leaves them untouched.

### 5. The differential harness is the correctness argument

Do not weaken, skip, or special-case the CPU-versus-CUDA comparison. It is the
reason this change is safe to make: a device observation that disagrees with the
oracle surfaces as a mismatch on the existing corpus.

Add a differential case that exercises the device path specifically — a model
whose views are all eligible — and one whose views are not, proving the fallback
engages.

### 6. Amend the decision record in the same commit

Ungrouped device observation does not contradict §K6, which concerns grouped
views. But the eligibility rule is a semantic commitment and belongs in
`DECISIONS.md`. Record it as a new §K or §L entry: what is eligible, what is
not, and that ineligibility is decided conservatively from the IR.

Do **not** amend §K6 or §K9 here — those govern grouped views, which this PRD
does not touch.

### 7. Measure

Re-run the `cuda-l4-20260726` case with `--timing-json` and report the full
phase table against the README's. `state_transfer`, `state_reconstruct` and
`readback_control` are the headline.

Report the eligibility decision for the benchmark model, and the per-view
breakdown, so a partial result can be diagnosed.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`, `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-runtime/src/executor.rs`, `crates/sembla-runtime/src/eval.rs`
- `crates/sembla-cli/src/main.rs` — the CUDA tick loop only, as authorised for
  `prds-cuda-host-path/0001`
- `crates/**/tests/**` (tests only)
- `DECISIONS.md` — §6's new entry only
- `docs/evidence/**` (new evidence only)
- `docs/prds-device-observation/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2. If the list turns out to make the PRD unachievable, say so
and stop; the operator will amend it before the run continues, per §M2's
carve-out.

## Non-goals

**No grouped views** — later PRD, and §K6/§K9 govern them. **No device-side
reduction of `wins`/`deferred`** — separate, worth 22%, and mixing them would
make both measurements uninterpretable. No change to the closed kernel fragment.
No change to `Sum` semantics or reduction order. No RNG change. No IR or Lean
changes. No new dependencies.

## Acceptance criteria

**Local (required for approval):**

1. An IR-level eligibility predicate exists, is conservative, and has tests
   covering each ineligible case in §1.
2. Model-level eligibility is computed once, recorded, and visible in output.
3. A test proves a host-ineligible view forces the fallback, including the state
   download.
4. Differential cases per §5 exist, one eligible and one not.
5. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
6. `cargo test --locked` and `scripts/check-rust.sh` green.
7. §6's `DECISIONS.md` entry is present and states the eligibility rule.
8. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2, listed in the implementation notes):**

9. `cargo build --release --features cuda` compiles on the GPU host.
10. CPU/CUDA differential equality holds on the corpus, including the
    demographic no-grouped model.
11. The `cuda-l4-20260726` case re-run with `--timing-json`, full phase table
    before and after, with the eligibility decision reported.
12. `kernels` reported before and after — device observation adds work there,
    and if it adds much, that is worth knowing.

## Note on expectations

The three targeted phases total 71% of CUDA wall time, so the ceiling is large —
but it is a ceiling, and three things will eat into it.

`report` at 12.8% and `other` at 5% are untouched. The differential path may
still transfer for hashing, per §4. And device observation adds kernel work
where there was almost none: 9.3 ms is 1.0% today, and a few passes over 5M rows
will add to it. That is the trade, and it is a good one, but the result will not
be 71%.

**A run that measures no improvement most likely took the fallback.** Check the
eligibility decision before concluding anything else — that is why §2 requires it
to be visible.

If the benchmark model turns out to be ineligible for a reason this PRD did not
anticipate, **stop and report it**. That finding is worth more than a partial
implementation, because it would mean the eligibility rule needs rethinking
before any of this is worth building.

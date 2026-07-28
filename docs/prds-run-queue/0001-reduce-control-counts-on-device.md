# PRD 0002: Count fired and deferred candidates on the device

## Context

Read `docs/prds-cuda-host-path/README.md` first; its constraints bind, including
the §J14.2 local/hardware split.

This folder reserved this work: "Later PRDs are deliberately unwritten and
re-scoped from the phase split measured after 0001 lands." That split has now
been measured twice on hardware, and the two candidates it named have become
almost the whole cost.

Measured 2026-07-28 on an H100 PCIe, 5M rows over 2 ticks
(`docs/evidence/demographic-bench/hyperstack-l4-20260728T072119Z/profile/`, and
reproduced within noise in the session of the same day):

| phase | grouped ms | share |
|---|---:|---:|
| **`readback_control`** | **203.3** | **55.9%** |
| **`report`** | **119.9** | **32.9%** |
| `other` | 21.8 | 6.0% |
| `kernels` | 19.0 | 5.2% |
| `state_transfer` | 0.0 | 0% |
| `state_reconstruct` | 0.0 | 0% |
| `observe_views` | 0.0 | 0% |

**Together they are 88.8% of CUDA wall time.** Everything else this folder and
`prds-device-observation` removed is now at zero.

## What the two phases actually do

`readback_control` (`backend.rs:1019`) is two `memcpy_dtov` calls and nothing
else:

- **`wins`** — one `u8` per candidate, where a candidate is a (transition, row)
  pair. Did this rule fire on this row?
- **`deferred`** — one `u8` per *(candidate × table)*. Did this candidate lose a
  claim on that resource table?

`report` is `control_reports` (`backend.rs:1029`), a host scan over those same
buffers.

On the benchmark model — 10 transitions over `person_slot`, 3 tables — at 5M
rows:

```
candidates = 10 × 5,000,000       = 50,000,000
wins       = 50,000,000 × 1 byte  =  50 MB
deferred   = 50,000,000 × 3 bytes = 150 MB
                                    ------
                          per tick  200 MB
```

The output of moving and scanning that is **at most 13 integers**: one fired
count per transition, and one deferred count per resource table that has any.

**Nothing in the simulation reads them.** They populate the fired/deferred lines
of the report. This is a diagnostic that costs 200 MB per tick.

## Goal

The counts are computed on the device. The per-tick transfer is a handful of
integers rather than hundreds of megabytes, and the host scan disappears.

## Specification

### 1. Reduce on the device

Two counts, both pure and both integer:

- **fired per transition**: the number of non-zero entries in
  `wins[candidate_offsets[rule] .. candidate_offsets[rule + 1]]`. A segmented
  count over contiguous per-rule ranges.
- **deferred per table**: for each global table index `t`, the number of
  candidates where `deferred[candidate * table_count + t] != 0`. A strided
  count.

Both are commutative integer reductions, so they are **bit-identical by
construction** regardless of thread scheduling — no ordering argument is needed,
and none should be offered.

**`candidate_offsets` is already resident on the device** (`backend.rs:219`,
uploaded once at construction, line 434). The segmented count needs no new
upload and no per-tick host-to-device copy; do not add one.

The only host consumers of `wins` and `deferred` are the two `memcpy_dtov` calls
in `readback_control` (`backend.rs:1130` and `1133`). Every other reference is a
kernel argument. So removing the transfer removes the last host dependency —
verify that before relying on it, but it is the expected finding.

`sembla_observe_view` already performs segmented counts on device, and PRD 0008
of `prds-cuda-validation-parallelism` established the multi-pass lock-free
reduction idiom. **Prefer those patterns over inventing a third.** No spin lock,
no critical section — a generated-source test already forbids them and must
continue to pass.

### 2. Do not download `wins` and `deferred` at all

Reducing on device and *still* copying the buffers would leave `readback_control`
where it is. The buffers must stay resident; only the counts come back.

If some path genuinely still needs the raw bytes, say which and why before
weakening this.

### 3. The reported values are unchanged, exactly

`control_reports` produces:

- `fired_per_box`: per box, a `(rule_id, count)` per transition, in the
  declaration order the current loop produces;
- `deferred_per_resource_table`: `(name, count)` pairs, walking boxes and tables
  in declaration order, **omitting tables whose count is zero**, and qualifying
  the name as `box.table` only when the model has more than one box.

All of that is observable — it reaches stdout, and the frozen gate hashes
stdout across every replicate and both backends. Reproduce it exactly, including
the omission of zeros and the name qualification rule.

### 4. Keep both timing phases, with their current meanings

`readback_control` stays the transfer (now tiny) and `report` stays the
construction of the report structures. Do not merge them, and do not renumber
the phase array.

Two reasons. The before/after comparison this PRD exists to produce needs the
same phases on both sides. And `main.rs:3420` maps the backend phase array
positionally; preserving it keeps the CLI unchanged.

### 5. Both call sites

`readback_control()` is called at `backend.rs:760` and `backend.rs:804` — the
timed and untimed tick paths. They must not diverge; a change applied to one
produces a backend whose timing instrumentation measures something the normal
path does not do.

### 6. Measure under the established protocol — on hardware, later

**Everything in this section is §J14.2 hardware-pending and is not required for
approval.** It is written here so the criteria are unambiguous when the GPU
session happens, not so that an implementer without a GPU attempts it.

Re-run the 5M/2-tick case with `--timing-json` and report the full phase table
against the one above, for **both** the no-grouped and grouped configurations.
`readback_control` and `report` are the headline; `kernels` should rise slightly
and that rise is part of the result, not a regression.

Also report whole-sweep numbers at 1M × 24 ticks × 20 draws. The session of
2026-07-28 measured a median later draw of **497 ms**, of which ~90% is these
two phases; that is the number this PRD should move, and it is what batch
workflows actually wait on.

## Allowed files

- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cli/src/main.rs` — **only** if §4's phase array or the
  observation types must change; nothing else in the CLI is in scope
- `crates/sembla-cuda/tests/**`, `crates/sembla-runtime/tests/**`,
  `crates/sembla-cli/tests/**` (tests only)
- `crates/sembla-cuda/scripts/run-differential-corpus.sh`
- `spikes/precision/infra-hyperstack/run-demographic-benchmark.sh` — only if the
  measurement in §6 needs a collector change
- `docs/evidence/**` (new evidence only)
- `docs/prds-cuda-host-path/README.md` (status notes only)

`main.rs` and the collector are listed because §4 and §6 could require them, not
because they are expected to. **If they turn out to be unnecessary, do not touch
them.**

**`DECISIONS.md` is deliberately excluded.** The verdict here is a hardware
result that will not exist at approval time, so the operator records it after
the GPU session. This also avoids the §M2 failure of 2026-07-28, where an
operator commit and an implementer's uncommitted edits collided in that one
file and made a criterion unsatisfiable.

**If a required gate fails on files outside this list, stop and report it
immediately** — `DECISIONS.md` §M2, and report it on attempt one, not attempt
five. Five managed runs have stalled on an allowed-file list that made a PRD
unachievable; in every case the runner was right and the list was wrong.

## Non-goals

No change to which candidates fire, to conflict resolution, or to §E3. No change
to `download_hash`, to state transfer, or to observation — those are done. No
kernel optimisation beyond the new reduction. No CPU backend changes: the CPU
path builds these counts from data it already holds and pays nothing. No sweep
or draw-lifecycle changes — that is `prds-sweep-throughput`. No new dependencies.

## Acceptance criteria

**Local (required for approval):**

1. The per-tick `memcpy_dtov` of `wins` and `deferred` is **removed from the
   code**, asserted structurally — a test over the generated source, or over the
   tick path, that fails if either transfer returns. It must be assertable
   **without a GPU**; the runtime confirmation is criterion 10.
2. The reduction's counts are turned into `fired_per_box` and
   `deferred_per_resource_table` by a host function that is tested directly from
   synthetic counts, with no GPU: covering an all-zero table (omitted), a table
   with a non-zero count (kept, in declaration order), and a multi-box model
   (names qualified `box.table`).
3. The generated source contains no spin lock or critical section; PRD 0008's
   assertion still passes.
4. **Every golden byte-identical**, including the manifest and
   `final_state_sha256`. `git diff --stat` shows none of them.
5. `cargo test --locked` and `scripts/check-rust.sh` green; CUDA-feature
   `cargo check`/`clippy` green without claiming GPU execution.
6. `python3 scripts/check-markdown-links.py` passes.
7. `python3 scripts/check-prd-allowlist.py docs/prds-cuda-host-path/0002-*.md`
   prints `OK`.

**Criteria 1–7 are sufficient for approval.** Everything below is §J14.2
hardware-pending, executed in a later GPU session. Do not block on lacking a
GPU, and do not present a local result as GPU evidence.

**Hardware (pending — but unlike previous folders, every command below exists
and is known green, per §M3):**

8. `cargo build --release --features cuda` on a GPU host.
9. **`BENCH_CORPUS=1` completes with `differential-corpus/exit-code.txt` = 0.**
   A *requirement*, not a deferral: the corpus ran green on hardware on
   2026-07-28, so this is the first PRD able to demand equality rather than
   promise it later.
10. Full `--timing-json` phase tables before and after, both configurations,
    confirming at runtime that `readback_control` has collapsed.
11. `BENCH_SWEEP=1` whole-sweep and median-later-draw numbers at 1M.

## Note on expectations

These two phases are 88.8% of CUDA wall time at 5M/2 ticks, so the ceiling is
large and unusually well established — measured three times on two hardware
sessions.

But a floor remains. The counts still have to be computed, which means reading
the same 200 MB **on the device**, where bandwidth is roughly 2 TB/s rather than
PCIe. That is around 0.1 ms per tick against the current ~160 ms, so the
residual should be small — but `kernels` will rise and the honest result is the
new total, not the disappearance of two rows.

A per-draw figure near 50–100 ms at 1M would be a good outcome. If the total
barely moves, the buffers are still being transferred somewhere this PRD did not
look, and §2 is the place to start.

**Do not report a §L4 ratio as this PRD's result.** §L9 retired it, and §L11
reaffirmed that even when it reads favourably.

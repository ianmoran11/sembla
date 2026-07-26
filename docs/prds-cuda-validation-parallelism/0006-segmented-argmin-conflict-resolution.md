# PRD 0006: Replace quadratic conflict resolution with a segmented argmin

## Context

Read `docs/prds-cuda-validation-parallelism/README.md` first; its constraints
bind. `DECISIONS.md` §L1–L6 are normative (§L6 supersedes §L1), and §E3 defines
the conflict semantics this PRD must preserve exactly.

Profiling on 2026-07-26 (`docs/evidence/cuda-profile-20260726/`) established:

| Rows | `resolve_conflicts` ns/instance | share of GPU time |
|---:|---:|---:|
| 500,000 | 483,282,045 | 28.5% |
| 2,000,000 | 6,432,974,613 | 56.7% |
| 5,000,000 | 43,979,638,562 | **77.9%** |

Fitted exponent **1.96** — quadratic. Extrapolated to 10M it accounts for ~176
of the 235.3 s/tick measured for the whole run. **This kernel is the CUDA cost.**

The cause is in `crates/sembla-cuda/src/codegen.rs`:

```c
for (unsigned long long other_row = 0; other_row < row_counts[...]; ++other_row)
```

an inner scan over every row of the resource table, inside a kernel already
running one thread per row — n threads × n iterations.

**The CPU oracle already implements this correctly.** `resolve_claims` in
`crates/sembla-runtime/src/executor.rs` builds a flat list of (candidate, claim)
instances, groups it, and takes a minimum within each `[start..end]` range using
`compare_instances`. That is the **segmented argmin** `DESIGN.md` §4.2 names as a
member of the closed kernel fragment. The CUDA backend simply does not use it.

## Goal

CUDA conflict resolution uses a segmented argmin with the same asymptotic
behaviour as the CPU oracle, selects **bit-identical winners**, and preserves
Level A determinism.

## Specification

### 1. Port the CPU algorithm, do not invent one

Mirror `resolve_claims`: build the (candidate, claim) instance list, group by
resource identity, and reduce to the minimum within each group under the
existing ordering. Structural similarity to the oracle is a feature — it makes
divergence easier to reason about and is the strongest available argument that
winners match.

Sorting or grouping must be deterministic: a total order on (resource, then the
existing `compare_instances` key, then a tie-break that cannot depend on thread
scheduling). A stable-by-construction key is preferable to relying on a stable
sort primitive.

### 2. Winners are results, not diagnostics

§E3's argmin with lexicographic tie-break **is** the conflict semantics. A
different winner is a different simulation, not a different message. This is a
stronger constraint than PRD 0002's diagnostic equality: existing CUDA
differential goldens must remain **byte-identical**, which is only possible if
every winner is unchanged.

### 3. Determinism must not depend on launch geometry

As §L3: no result may vary with block count, grid size, or scheduling order.
Where a reduction is used, it must be order-independent or ordered by an explicit
key, never by arrival.

### 4. Re-profile at the target scale

After the change, re-run the profile at 500k, 2M, and 5M and record the new
exponent. The acceptance is not "faster" but **"no longer quadratic"**: the
fitted exponent must be at or near linear. Cost at one scale can mislead — that
is exactly how this PRD's two previous scopings went wrong.

### 5. Then re-run the frozen §L4 gate

Unchanged protocol. Record the verdict as measured.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`
- `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-cuda/tests/**`
- `DECISIONS.md` (§L addition only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-cuda-validation-parallelism/README.md` (status notes only)

## Non-goals

No change to the conflict *semantics* — only how they are computed. No change to
`compare_instances` ordering or §E3. No CPU-side changes. No work on
`check_candidate_errors` or `prepare_effects` — those are PRD 0007. No
grouped-observation support (§L5).

## Acceptance criteria

**Local:**

1. The emitted `sembla_resolve_conflicts` contains no scan over all rows of
   another table; the §5 guard test from PRD 0007 (or an equivalent here)
   asserts it.
2. Every existing CUDA differential golden is **byte-identical**. Any diff is a
   failed PRD, because it means a winner changed.
3. `examples/**` and all CSV/hash goldens unchanged; `cargo test --locked` and
   `scripts/check-rust.sh` green.
4. A test asserts winner selection is independent of launch geometry, mirroring
   PRD 0003's approach.

**Hardware (pending per §J14.2):**

5. Profiles at 500k, 2M, 5M show `resolve_conflicts` scaling at or near linear;
   the fitted exponent is recorded.
6. CPU/CUDA differential equality holds on the corpus including the demographic
   no-grouped model.
7. The frozen §L protocol is re-run and the §L4 verdict recorded as measured.

## Risk note

This is the first PRD in the folder that can change results rather than
diagnostics or timing. Criterion 2 is the guard: if any golden moves, stop. A
faster kernel that picks different winners is not a partial success — it is a
semantic regression, and the correct response is to revert and understand why,
not to regenerate goldens.

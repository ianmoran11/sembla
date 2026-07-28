# PRD 0002: Grouped views on the device

## Context

Read `docs/prds-device-observation/README.md` first; its constraints bind,
including the eligibility rules and the §J14.2 local/hardware split. Read
`0001-device-observation-ungrouped` too: this PRD extends its eligibility
predicate rather than building a second mechanism.

**This is the PRD that makes the folder matter.** The demographic model's
calibration and validation outputs *are* grouped views, and
`docs/demographic-model.md` runs the workflow with
`--enable grouped-observations` throughout:

| view | table | keys |
|---|---|---|
| `population_cells` | `person_slot` | `sex`, `area`, `age_months` banded by 60 |
| `deaths_cells` | `person_slot` | `sex`, `event_age_months` banded by 60 |
| `vacancy_cells` | `person_slot` | `entry_stream`, `area` |

Because eligibility is all-or-nothing per run, 0001 alone leaves the driver
model downloading the full state every tick. Only with grouped views on the
device does the 71% become available to real work.

`crates/sembla-cli/src/main.rs:2660` and `:2709` currently reject grouped views
on CUDA outright. Removing that rejection is the visible part of this change;
the decision-record amendment is the substantive part.

## Why this is easier than it looks

`observe_grouped_views` (`executor.rs:601`) reduces by **count only** —
`*buckets.entry(tuple).or_default() += 1`. There is no float accumulation
anywhere in the grouped path, so a device reduction is bit-identical by
construction rather than by argument.

The key spaces are small and dense:

- `sex`, `entry_stream` — enums, cardinality known from the schema
- `area` — a `Ref`, bounded by the target table's row count
- `age_months` / 60 — banded `Int`, the only unbounded axis

`population_cells` is roughly 2 × 4 × 20 ≈ **160 groups**. This is a dense
histogram, not the dynamic-discovery problem it resembles on the host, where
`BTreeMap` is used because it needs no bound.

## Goal

Grouped views are observed on the device, with identical values and identical
output order. A model whose views are all eligible — grouped or not — skips the
per-tick state download.

## Specification

### 1. Bound the key space, exactly

Derive each key axis's cardinality:

- **enum** — variant count from the schema;
- **`Ref`** — target table row count;
- **banded `Int`** — not statically known. Take one min/max reduction over the
  column on the device and derive the band range from it. This is exact, costs
  one pass, and avoids both a guessed bound and a declared one.

If the product of cardinalities exceeds a configured limit, **fail
deterministically** with a diagnostic naming the view and the computed size. Do
not silently fall back to the host: a model that quietly runs 40× slower than
the operator expects is worse than one that refuses.

### 2. Reduce as a dense histogram inside the closed fragment

One counter per group, incremented per selected row. `DESIGN.md` §4.2 admits
commutative-monoid group-by, and integer counting is one. Do not extend the
fragment.

Return only the counters — at most a few hundred integers per view per tick —
never per-row data.

### 3. Output order must match the host exactly

The host iterates a `BTreeMap<Vec<i128>, usize>`, so `GroupedViewValue`s emerge
**sorted by key tuple**, ascending, lexicographically by axis in declaration
order. The device path must reproduce that order exactly.

**Empty groups are the trap.** The host only emits groups that occur in the
data, because `BTreeMap` has no entry for a group nothing landed in. A dense
histogram has a counter for every possible group, most of them zero.
**Zero-count groups must not be emitted**, or every golden moves.

A test must cover a model where some groups are empty.

### 4. Keys are `i128` on the host — preserve the values

`GroupedViewValue.keys` is `Vec<i128>`. The device works with packed small
integers, so unpacking must reproduce the same `i128` values the host produces
— including the banded value, which is the *band index*, not the raw attribute.
Check `observe_grouped_views` for exactly what is stored before assuming.

### 5. Remove the CUDA rejection, and amend the decision record

Delete the rejections at `main.rs:2660` and `:2709`, and amend:

- **§K6**, which states "V1 is CPU-only and rejects grouped views
  deterministically on CUDA";
- **§K9**, which lists CUDA grouped-observation support as a deferred construct
  with a trigger.

State the new position: grouped observation is supported on CUDA where the key
space is boundable, with the §1 limit and its deterministic failure. Leaving
either section contradicting the code is not acceptable.

**Amended 2026-07-28.** This originally required both in *one commit*. That is
no longer satisfiable and the requirement is dropped: the operator's commit
`696f439` accidentally included the §K6/§K9 amendments, which were sitting
uncommitted in the working tree from an earlier attempt, alongside an unrelated
`DECISIONS.md` change. The rejection removals then landed separately in
`aeb3c28`.

Atomicity was a means, not the end. What matters is that the decision record
never contradicts the code, and at HEAD it does not. Rewriting pushed history to
satisfy the packaging would be worse than recording why the packaging differs.

Note that `grouped-observations` remains a §E8 runtime flag recorded in the
manifest. This PRD changes which backends honour it, not whether it is recorded.

### 6. The differential harness is the correctness argument

Add the demographic model **in its grouped configuration** to the differential
corpus — the configuration `DECISIONS.md` §L5 excluded precisely because CUDA
rejected it. That is the strongest available evidence that device grouping
matches the oracle, and it closes a long-standing gap: the differential corpus
has never covered the configuration the driver model actually uses.

### 7. Measure

Re-run the `cuda-l4-20260726` case with `--timing-json`, and **additionally** a
grouped-configuration run, since the frozen case is no-grouped and cannot show
this PRD's benefit. Report both phase tables.

Report per view: the computed key-space size, the number of groups actually
occupied, and the number emitted.

## Allowed files

- `crates/sembla-cuda/src/codegen.rs`, `crates/sembla-cuda/src/backend.rs`
- `crates/sembla-runtime/src/executor.rs`
- `crates/sembla-cli/src/main.rs` — the CUDA path and the rejections at 2660/2709
- `crates/**/tests/**` (tests only)
- `crates/sembla-cuda/scripts/run-differential-corpus.sh` — **added 2026-07-28
  by operator authorisation**, see below
- `DECISIONS.md` — §K6 and §K9 amendments per §5
- `fixtures/**` — only if the differential corpus needs a grouped entry
- `docs/evidence/**` (new evidence only)
- `docs/prds-device-observation/README.md` (status notes only)

**If a required gate fails on files outside this list, stop and report it** —
`DECISIONS.md` §M2, whose carve-out also covers amending this list if it proves
unachievable.

### Why the corpus runner is in scope (authorised 2026-07-28)

The original list omitted `run-differential-corpus.sh`, which made this PRD
**self-contradictory**: §5 requires removing the CUDA rejection of grouped
views, §6 requires adding the grouped demographic configuration to the
differential corpus, and that script is where both live. It currently asserts
that grouped observations *are* rejected — an assertion §5 necessarily breaks —
so the corpus run cannot pass without editing it.

The exception is limited to the corpus entry and the removal of the
now-obsolete rejection assertion. It does not extend to changing how the corpus
runs, what else it covers, or its failure semantics.

## Non-goals

No change to grouped-view *semantics* — same groups, same counts, same order,
same `--enable` flag. No `Sum` support in grouped views; they are count-only
today and this PRD does not widen them. No device-side reduction of
`wins`/`deferred` — separate, worth 22%. No change to the closed kernel
fragment. No IR or Lean changes. No new dependencies.

## Acceptance criteria

**Local (required for approval):**

1. Key-space bounding per §1, including the min/max derivation for banded `Int`
   and deterministic failure past the limit, with tests for both.
2. Zero-count groups are not emitted; a test covers a model with empty groups.
3. Output order matches the host `BTreeMap` order exactly; a test compares the
   full `GroupedViewValue` sequence, not just the counts.
4. The CUDA rejections are removed and §K6 and §K9 are amended, both present at
   `HEAD`. They need not share a commit — see §5's 2026-07-28 amendment.
5. The grouped demographic configuration is in the differential corpus.
6. **Every golden is byte-identical**, including the manifest and
   `final_state_sha256`.
7. `cargo test --locked` and `scripts/check-rust.sh` green.
8. `python3 scripts/check-markdown-links.py` passes.

**Hardware (pending per §J14.2, listed in the implementation notes):**

9. `cargo build --release --features cuda` compiles on the GPU host.
10. CPU/CUDA differential equality holds on the corpus **including the grouped
    demographic configuration**.
11. Phase tables for both the frozen no-grouped case and a grouped run.
12. Per-view key-space size, occupied groups, and emitted groups.

## Note on expectations

For the frozen no-grouped case this PRD changes nothing — that configuration has
no grouped views. **Its benefit only appears in a grouped run**, which is why §7
requires one.

In a grouped run the prize is the same 71% that 0001 unlocks for no-grouped
models, since it is the same download being skipped. Expect less: `report` and
`other` are untouched, the differential path may still transfer for hashing, and
the histogram adds kernel work.

The most likely surprise is §3. Emitting zero-count groups is the natural way to
write a dense histogram and it would silently change every grouped golden — a
diff nobody would read as a semantics change until they looked closely. Treat
that test as the one that matters.

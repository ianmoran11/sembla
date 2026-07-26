# PRD 0003: Compute per-tick state hashes only when something consumes them

## Context

Read `docs/prds-host-evaluator-performance/README.md` first; its constraints
bind. PRDs 0001 and 0002 took the fixed case from 49.7 s to 10.9 s with
byte-identical output. This PRD addresses what the resulting full-duration
profile shows is now the single largest cost.

In `docs/evidence/host-evaluator-reference-resolve-once-20260726/post-change-full-duration.sample.txt`,
of 8390 main-execution samples:

| Branch | Samples | Share |
|---|---:|---:|
| `execute_backend_output_with_features` → `StateStore::state_hash` | 2637 | **31.4%** |
| `execution_hashes` → `state_hash` (final, once) | 109 | 1.3% |

The 2637 is `main.rs:2528`, at the bottom of the tick loop:

```rust
per_tick_hashes.push(state.state_hash());
```

a full SHA-256 over every column of every table — 1M rows — once per tick.

**Nothing in a plain `sembla run` consumes the result.** `per_tick_hashes` is
carried on `RunOutput` and `BackendRunOutput` and read in exactly one place: the
CPU-vs-CUDA differential comparison at `main.rs:3768`, which reports the first
tick where the two sequences diverge and quotes the hashes in its diagnostics.
It is never written to a manifest and never emitted to a file. The run manifest
records `final_state_sha256` only, produced separately by `execution_hashes`
(`main.rs:2250`), which this PRD does not touch.

Neither `DECISIONS.md` nor `DESIGN.md` requires per-tick hashes of a plain run.

The precedent already exists on the other side of the codebase:
`sembla_cuda::HashMode` is `EveryTick | FinalOnly`, and the CUDA backend is
constructed with `HashMode::EveryTick` at `main.rs:2554`.

## Goal

Per-tick state hashing happens when a consumer needs it and not otherwise. No
recorded digest changes, and no comparison that runs today becomes weaker.

## Specification

### 1. Thread a hash mode through the run path

Introduce a per-tick hash mode for the host run path, reaching
`run_results_output_with_features` and `run_results_output_cuda`. Match
`sembla_cuda::HashMode`'s naming; reusing that type or defining a CLI-local
equivalent are both acceptable, but do not move the CUDA type between crates —
that is a refactor with its own risks and is not in scope.

The differential command requests every-tick hashing. Plain `run` does not.

### 2. Make absence type-enforced, not silently empty

**This is the criterion the PRD turns on.** If per-tick hashes become an empty
`Vec` when disabled, the differential comparison at `main.rs:3768` still
compiles, still runs, finds no divergence across zero elements, sees two equal
lengths, and **passes**. That converts the CUDA equality check into a no-op that
reports success. A slow run is a much smaller problem than a green differential
that checked nothing.

So the disabled state must be unrepresentable as "no divergence":

- Model per-tick hashes as `Option<Vec<[u8; 32]>>` (or an equivalent that
  cannot be confused with an empty sequence) so the comparison must handle the
  absent case explicitly.
- The differential path must **fail loudly** if it is handed absent hashes —
  this is an internal invariant violation, not a user error.
- Add a test asserting the differential command still detects a divergence it
  detects today. A mutation-style check is ideal: with a deliberately
  divergent hash sequence, the comparison must still report the first differing
  tick.

While here, fix the ordering at `main.rs:3768`: the sequences are `zip`ped
before their lengths are compared, so a length mismatch is currently reported
only if no earlier element differs. Compare lengths first.

### 3. Apply it to the CUDA path too

`run_results_output_cuda` hashes the host state mirror per tick at its own tick
loop, *and* constructs `CudaBackend` with `HashMode::EveryTick`. Both should
follow the requested mode. State in the implementation notes whether these are
two separate per-tick hashes of the same state, because if so, that is worth
recording even if this PRD only gates them.

Do not re-run the §L4 gate or amend its recorded verdict. §L4 is frozen and its
verdict stands as measured; a note that the protocol's cost profile has changed
is appropriate, a re-measurement is a separate decision.

### 4. Keep the final hash and everything recorded

`execution_hashes`, `final_state_sha256`, the initial-state hash record, and
every manifest field stay exactly as they are. This PRD removes computations
whose results are discarded; it changes nothing that is written down.

### 5. Measure, locally

Run the README's fixed case three times before and after on the same machine in
one session, take the median, and capture a full-duration `sample` profile of
the post-change build. Commit both under `docs/evidence/`.

Report **user time alongside wall time.** The 0002 baseline had a contended run
(23.92 s wall against 14.20 s user) that inflated its median and made the
headline 1.62× overstate a real ~1.4×. If a run's wall time exceeds user+sys by
a wide margin, note it and say which figure is load-bearing.

## Allowed files

- `crates/sembla-cli/src/main.rs`
- `crates/sembla-cli/tests/**`, `crates/sembla-runtime/tests/**` (tests only)
- `docs/evidence/**` (new profile evidence only)
- `docs/prds-host-evaluator-performance/README.md` (status notes only)

## Non-goals

**No change to what is hashed, how it is hashed, or the digest algorithm.** Not
the field order, not the framing, not SHA-256 itself. Those digests are the
reproducibility contract — `DECISIONS.md` §E2 defines determinism levels
against them and §J14 uses them as GPU evidence — and any change to them fails
acceptance criterion 1 by construction. This PRD is only about *when* the
existing hash is computed.

No moving `HashMode` between crates. No evaluator, allocation, or write-path
work. No IR, Lean, or CUDA kernel changes. No new dependencies.

## Acceptance criteria

1. **Every golden is byte-identical**: `examples/**`, all CSV and hash goldens,
   the frozen demographic state fixture, and the tracked CUDA differential
   evidence. `git diff --stat` shows none of them. Every manifest field,
   including `final_state_sha256`, is unchanged.
2. A plain `sembla run` performs no per-tick `state_hash` call. A test or
   profile-based assertion covers this.
3. The differential command still requests and compares per-tick hashes, and a
   test proves it still detects a divergence — absent hashes cause a loud
   failure, never a pass.
4. Length mismatch is reported as a length mismatch, checked before the
   element-wise comparison.
5. `cargo test --locked` and `scripts/check-rust.sh` green, with unchanged
   negative-suite expectations.
6. Before/after medians over three runs each, with user time reported and any
   contended run flagged, plus a full-duration post-change `sample` profile.
7. `python3 scripts/check-markdown-links.py` passes.

## Note on expectations

31.4% of samples is a full-duration measurement, so unlike the figures 0001 and
0002 were scoped from, it is a sound share of total runtime. Removing it should
be visible — but the profile also shows real compute underneath (`log` at 545
samples, `draw_u32x4` at 538) that sets a floor, and Amdahl applies.

As with the previous two, the case does not rest on a predicted number: this is
work whose output is discarded, and not doing it cannot change a result. If the
gain is smaller than the share suggests, that is a finding, and the next PRD is
scoped from the new profile.

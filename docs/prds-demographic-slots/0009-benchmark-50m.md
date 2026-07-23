# PRD 0009: Scale benchmark — memory, throughput, and the `Expr::Tick` trigger

## Context

Read `docs/prds-demographic-slots/README.md` first; DECISIONS §K2's
trigger binds this PRD's purpose: **measure** whether the monthly ageing
write (one `Int` write per present slot per tick — ~300M/year at
25M people) is a material cost, producing the evidence that either fires
or dismisses the deferred `Expr::Tick` design. Secondary purpose: honest
capacity numbers for the 50M-slot architecture — the use case's storage
arithmetic (~64–80 B/slot raw; double-buffered `StateStore` ⇒ ≥6.4–8.0 GB
steady state before overheads) is a lower-bound estimate, not a
measurement.

This folder's convention for hardware-dependent work (from the
integration track): **local criteria** must pass in the managed run on a
no-GPU, moderate-memory machine; **hardware criteria** are scripted,
documented, and listed as pending — never fabricated.

## Goal

A deterministic synthetic-state generator and benchmark script measure
state-artifact I/O, memory, tick throughput, per-transition-group cost
shares, and export cost for the demographic model across scales; local
runs at 1M slots produce checked-in evidence; 50M-scale and CUDA runs are
scripted with a filled-in evidence template awaiting hardware; the §K2
trigger determination is recorded.

## Specification

### 1. `sembla synth-state` — deterministic benchmark-state synthesis

New CLI command mirroring `synth-pop`'s role (explicitly labelled
benchmark/test tooling in USAGE and docs — DESIGN §10.5's descope of
scientific population generation stands):

```text
sembla synth-state --model <model-or-plan.json> --slots N --areas K \
  --present-fraction F --streams birth:B,overseas:O,internal:I \
  --seed S --out state.artifact
```

Generates a `sembla.state/v1` artifact for the `demographic_slots` schema
shape *generically* (driven by the model's declared tables/attrs — no
hard-coded attribute lists beyond a documented mapping of the demographic
columns; if full genericity is awkward, scope it honestly to models with
the demographic column roles and reject others deterministically, noting
the scope in USAGE). Deterministic: same flags ⇒ identical bytes (Philox
coordinate draws or plain arithmetic; no OS RNG). `--slots` overriding
the model's declared `rows` conflicts with PRD 0002's exact-match rule —
resolve it the honest way: `synth-state` **also emits a companion model
file** with `rows` rewritten to match (legacy-model JSON manipulation is
fine here; it is tooling, not semantics), or requires a model whose
declared rows already equal `--slots`. Pick one, document it, test it.

### 2. `scripts/bench-demographic.sh`

A parameterized script (env/flags for scale list, seed, ticks, output
dir) that, per scale in its list, runs and records:

1. `synth-state` wall time and artifact size on disk;
2. state-artifact **load** time (a `run` with `--ticks 0` or the closest
   supported no-op run — verify what the CLI supports and use it);
3. peak RSS and wall time for a fixed-tick run (24 ticks) of: (a) the
   full model, (b) the model minus the ageing transition, (c) the model
   minus grouped views (flag off, grouped views absent via a variant
   model) — three variant model files maintained beside the canonical
   one, generated or checked in with a comment tying them to this
   benchmark (they are benchmark fixtures, not canonical models);
   measure via `/usr/bin/time -l` on darwin and `/usr/bin/time -v` on
   linux (detect and branch);
4. `--export-state` wall time and artifact size;
5. derived quantities: ticks/sec, ageing cost share
   ((a−b)/a wall-time), grouped-observation cost share ((a−c)/a).

Output: one machine-readable `bench-results.json` per invocation
(scale, machine fingerprint: OS, CPU, RAM — no hostnames/paths per the
§5.3 identity rules' spirit) plus a rendered markdown table. The script
must be re-runnable and idempotent into a fresh directory.

### 3. Local evidence (managed-run acceptance)

Run the script locally at **10k, 100k, and 1M slots** (24 ticks, fixed
seed) and check the results into
`docs/evidence/demographic-bench/local-<date>/` (JSON + table), following
the precedent of the precision-spike evidence conventions (tracked
directory, README naming the machine class). CI is **not** extended to
run benchmarks (they are manual, like the differential corpus).

Sanity assertions in a CLI test (fast, small scale only — 10k): the
synth-state artifact validates and loads; two synth-state invocations are
byte-identical; the three model variants produce the expected
presence/absence of transitions/views (guarding against the variants
drifting from the canonical model — compare their IR against the
canonical model's IR minus the named pieces).

### 4. 50M and CUDA — hardware criteria (pending)

- The script's scale list accepts `50000000`; nothing in the code may
  assume it fits — `synth-state` and the loader must stream or
  chunk-write enough to build the artifact without holding 2× copies
  (verify; fix `write` with a streaming path in `state_artifact.rs` if
  needed, byte-identical output, round-trip test at small scale).
- `docs/demographic-benchmark.md`: the evidence template — machine
  requirements (≥ 32 GB RAM for 50M CPU; H100-class for CUDA), the exact
  commands, the table skeleton with **pending** entries for 10M/50M CPU
  and CUDA runs, and the interpretation section (below). The CUDA row
  notes grouped observations are CPU-only (§K6) so CUDA rows run the
  no-grouped variant.
- Implementation notes list the pending hardware runs explicitly.

### 5. The §K2 trigger determination

`docs/demographic-benchmark.md` ends with a **Determination** section:
from the 1M local numbers (and extrapolation clearly labelled as such),
state whether the ageing-write share is material (suggested line: >10%
of tick wall time at 1M, or a stated reason the number does not
extrapolate), and therefore whether the `Expr::Tick` design should be
opened. This is a *recommendation with evidence*; the decision itself is
a future §K amendment — say exactly that in the section. Mirror the
recommendation in the implementation notes.

## Allowed files

- `crates/sembla-cli/src/main.rs` (synth-state), `crates/sembla-cli/tests/**`
- `crates/sembla-runtime/src/state_artifact.rs` (streaming write only, if
  needed)
- `scripts/bench-demographic.sh` (new), benchmark variant model fixtures
  under `fixtures/demographic/**`
- `docs/demographic-benchmark.md` (new),
  `docs/evidence/demographic-bench/**` (new), `docs/demographic-model.md`
  (link only)
- implementation notes/artifacts created by the managed run

## Non-goals

- Implementing `Expr::Tick` (the determination gates a future design,
  nothing more).
- Performance optimization of `StateStore`, cloning, or kernels — measure
  first; optimization PRDs come from the numbers, not with them.
- CI benchmark jobs, dashboards, or regression gates.
- Scientific realism in synthesized states; hostname/path-bearing
  evidence.

## Acceptance criteria

1. Full check battery passes; `synth-state` determinism, validation, and
   the variant-model IR-diff assertions pass at small scale.
2. `scripts/bench-demographic.sh` runs locally at 10k/100k/1M and the
   evidence directory contains the JSON + rendered table for all three
   scales with machine class recorded.
3. Ageing and grouped-observation cost shares are computed and present in
   the evidence; the Determination section states the §K2 recommendation
   with its threshold reasoning.
4. `docs/demographic-benchmark.md` carries the 50M/CUDA template with
   explicit pending entries and exact commands; the implementation notes
   list them as pending.
5. The 50M path is structurally safe (streaming/chunked artifact write
   verified or landed with byte-identical small-scale round-trips).
6. `git diff --check` passes; no new dependencies; no canonical model or
   frozen artifact changed.

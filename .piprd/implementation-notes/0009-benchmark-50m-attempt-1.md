# PRD 0009 implementation notes — attempt 1

## Benchmark-state tooling

Added `sembla synth-state` as explicitly scoped benchmark/test tooling. It
accepts either a legacy model or validated plan, requires the documented
`demographic_slots` table/column roles, rejects other schemas deterministically,
and uses fixed arithmetic coordinate mixing rather than OS randomness. It
emits `<out>.model.json`, a canonical legacy companion whose `area`,
`person_slot`, and `slot_resource` row declarations match the requested scale;
this preserves the exact-row-count state-artifact contract without mutating a
plan identity.

The state writer now validates the caller-owned columns without cloning them,
then streams the canonical header and little-endian column payloads with a
bounded 64 KiB encoding buffer. Existing frozen state-artifact bytes remain
unchanged. The 50M synthesis path therefore holds the typed columns once rather
than adding an artifact-sized byte-vector copy.

## Variants and tests

The benchmark fixtures under `fixtures/demographic/benchmark/` are the
canonical model, canonical-minus-`age_monthly`, and canonical-minus-grouped-
views. `crates/sembla-cli/tests/synth_state.rs` compares their IR directly to
those exact canonical subtractions, synthesizes and loads 10k rows, checks
model and plan inputs, validates the emitted artifact/companion, proves two
invocations byte-identical, and checks deterministic rejection outside the
documented demographic roles.

## Script and local evidence

`scripts/bench-demographic.sh` accepts scale, seed, ticks, output directory,
backend, and machine-class flags/environment. It uses `/usr/bin/time -l` on
Darwin or `-v` on Linux and records synthesis, real `--ticks 0` load through a
summary-free schema-equivalent working model, full/no-ageing/no-grouped 24-tick
runs, export, peak RSS, artifact bytes, ticks/second, and the two required cost
shares. It removes only its output directory's `work/` subtree, is rerunnable,
and records no hostname or workspace path.

Managed-run CPU evidence is checked in at
`docs/evidence/demographic-bench/local-2026-07-24/` for 10k, 100k, and 1M
slots, seed 9009, 24 ticks. At 1M the full run was 54.68 s, no-ageing was
51.50 s, measured ageing share was 5.8%, throughput was 0.439 ticks/s, and peak
RSS was 551.8 MiB. The grouped single-run share was -3.8% and is retained as
noise rather than adjusted or promoted to a performance claim.

## §K2 recommendation

The measured 1M ageing-write share is below the stated 10% materiality
threshold. The recommendation is **not to open `Expr::Tick` now**. Linear
extrapolation preserves the percentage, but 10M/50M memory/cache behavior is
unknown. This is evidence for a future decision, not a DECISIONS amendment;
opening the design still requires a future §K amendment.

## Pending hardware criteria

The following runs are explicitly pending and no values were fabricated:

- 10M CPU full/no-ageing/no-grouped on a dedicated ≥32 GiB host;
- 50M CPU full/no-ageing/no-grouped on a dedicated ≥32 GiB host;
- 10M CUDA no-grouped on an H100-class system;
- 50M CUDA no-grouped on an H100-class system.

`docs/demographic-benchmark.md` contains the exact CPU/CUDA commands and a
pending table. CUDA uses only the no-grouped variant because grouped
observations remain CPU-only under DECISIONS §K6.

## Validation

Passed:

- `cargo test --locked -p sembla-cli --test synth_state` (4 tests,
  including 10k deterministic synthesis/load and exact variant IR diffs);
- runtime and CLI state-artifact test targets (26 + 8 passing, with only the
  explicit runtime regeneration test ignored);
- the existing demographic-slot target (14 passing, four explicit
  regeneration tests ignored), preserving canonical model/state goldens;
- `cargo clippy --locked --workspace --all-targets -- -D warnings`;
- `./scripts/check.sh`, including the full Rust, Lean, negative, parity,
  documentation, dependency, and lock checks;
- a post-change benchmark-script smoke run and `bash -n`;
- temporary-index `git diff --cached --check` and Markdown-link validation
  across all 14 PRD candidate files.

The real Git index remained empty. `Cargo.lock`, `DESIGN.md`, `examples/**`,
the canonical demographic model/plan/state, and all prior demographic goldens
remain unchanged.
